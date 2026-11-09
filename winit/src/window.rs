mod mode_flags;
mod state;

use mode_flags::ModeFlags;
use state::State;

pub use crate::core::window::{CursorGrab, Event, Id, RedrawRequest, Settings, cursor};

use crate::Proxy;
use crate::conversion;
use crate::core;
use crate::core::alignment;
use crate::core::input_method;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::shell;
use crate::core::text;
use crate::core::theme;
use crate::core::time::{Duration, Instant};
use crate::core::{Color, InputMethod, Padding, Point, Rectangle, Renderer, Size, Text, Vector};
use crate::graphics::Compositor;
use crate::program::{self, Program};

use winit::dpi::{LogicalPosition, LogicalSize};
use winit::monitor::MonitorHandle;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub struct Manager<P, C>
where
    P: Program,
    C: Compositor<Renderer = P::Renderer>,
    P::Theme: theme::Base,
{
    aliases: BTreeMap<winit::window::WindowId, Id>,
    entries: BTreeMap<Id, Window<P, C>>,
    family: Family,
}

/// The owner relation between windows.
#[derive(Debug, Default)]
struct Family {
    parents: BTreeMap<Id, Id>,
    /// Windows whose creation was requested but whose owner closed before
    /// they were created.
    cancelled: BTreeSet<Id>,
}

impl Family {
    fn set_parent(&mut self, id: Id, parent: Id) {
        let _ = self.parents.insert(id, parent);
    }

    fn cancel(&mut self, id: Id) {
        let _ = self.cancelled.insert(id);
        self.forget(id);
    }

    /// Cancels the ids in `ids` that have a parent and are not `is_open`.
    fn cancel_pending(&mut self, ids: &[Id], is_open: impl Fn(Id) -> bool) -> Vec<Id> {
        let pending: Vec<Id> = ids
            .iter()
            .copied()
            .filter(|id| self.parents.contains_key(id) && !is_open(*id))
            .collect();

        for &id in &pending {
            self.cancel(id);
        }

        pending
    }

    /// Returns whether the creation of `id` was cancelled, and clears it.
    fn take_cancelled(&mut self, id: Id) -> bool {
        self.cancelled.remove(&id)
    }

    fn forget(&mut self, id: Id) {
        let _ = self.parents.remove(&id);
    }

    /// Returns every window owned by `id`, directly or not, children
    /// before their own parents.
    fn descendants(&self, id: Id) -> Vec<Id> {
        let mut result = Vec::new();

        for (&child, &parent) in &self.parents {
            if parent == id {
                result.extend(self.descendants(child));
                result.push(child);
            }
        }

        result
    }
}

impl<P, C> Manager<P, C>
where
    P: Program,
    C: Compositor<Renderer = P::Renderer>,
    P::Theme: theme::Base,
{
    pub fn new() -> Self {
        Self {
            aliases: BTreeMap::new(),
            entries: BTreeMap::new(),
            family: Family::default(),
        }
    }

    /// Records that `parent` owns the window `id`.
    pub fn set_parent(&mut self, id: Id, parent: Id) {
        self.family.set_parent(id, parent);
    }

    /// Returns the windows owned by `id`, directly or not, children first.
    pub fn descendants(&self, id: Id) -> Vec<Id> {
        self.family.descendants(id)
    }

    /// Cancels the creation of every window in `ids` that is owned by a
    /// window but is not open yet, and returns the cancelled ids.
    ///
    /// Only ids with a recorded parent are considered, so closing an
    /// unrelated or already closed id records nothing.
    pub fn cancel_pending(&mut self, ids: &[Id]) -> Vec<Id> {
        self.family
            .cancel_pending(ids, |id| self.entries.contains_key(&id))
    }

    /// Returns whether the creation of `id` was cancelled by
    /// [`Manager::cancel_pending`]. A cancelled id is reported once.
    pub fn take_cancelled(&mut self, id: Id) -> bool {
        self.family.take_cancelled(id)
    }

    pub fn insert(
        &mut self,
        id: Id,
        window: Arc<winit::window::Window>,
        program: &program::Instance<P>,
        compositor: &mut C,
        proxy: Proxy<P::Message>,
        renderer_settings: renderer::Settings,
        exit_on_close_request: bool,
        system_theme: theme::Mode,
    ) -> &mut Window<P, C> {
        let state = State::new(program, id, &window, system_theme);
        let mut surface = compositor.create_surface(window.clone());
        let surface_size = state.physical_size();
        let surface_version = state.surface_version();

        compositor.configure_surface(&mut surface, surface_size.width, surface_size.height);

        let renderer = compositor.create_renderer(renderer_settings);

        let waker = shell::Waker::new(move || {
            proxy.send_action(iced_runtime::Action::Event {
                window: id,
                event: core::Event::Waken,
            });
        });

        let _ = self.aliases.insert(window.id(), id);

        let _ = self.entries.insert(
            id,
            Window {
                raw: window,
                waker,
                state,
                exit_on_close_request,
                surface,
                surface_version,
                surface_error_at: None,
                renderer,
                mouse_interaction: mouse::Interaction::None,
                cursor_grab: CursorGrab::None,
                release_grab_on_escape: false,
                redraw_at: None,
                frame_clock: FrameClock {
                    anchor: Instant::now(),
                    period: None,
                },
                preedit: None,
                ime_state: None,
                last_frame: None,
                frame_burst: 0,
            },
        );

        self.entries
            .get_mut(&id)
            .expect("Get window that was just inserted")
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_idle(&self) -> bool {
        self.entries
            .values()
            .all(|window| window.redraw_at.is_none())
    }

    pub fn redraw_at(&self) -> Option<Instant> {
        self.entries
            .values()
            .filter_map(|window| window.redraw_at)
            .min()
    }

    pub fn first(&self) -> Option<&Window<P, C>> {
        self.entries.first_key_value().map(|(_id, window)| window)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id, &mut Window<P, C>)> {
        self.entries.iter_mut().map(|(k, v)| (*k, v))
    }

    pub fn get(&self, id: Id) -> Option<&Window<P, C>> {
        self.entries.get(&id)
    }

    pub fn get_mut(&mut self, id: Id) -> Option<&mut Window<P, C>> {
        self.entries.get_mut(&id)
    }

    pub fn get_mut_alias(
        &mut self,
        id: winit::window::WindowId,
    ) -> Option<(Id, &mut Window<P, C>)> {
        let id = self.aliases.get(&id).copied()?;

        Some((id, self.get_mut(id)?))
    }

    pub fn last_monitor(&self) -> Option<MonitorHandle> {
        self.entries.values().last()?.raw.current_monitor()
    }

    pub fn remove(&mut self, id: Id) -> Option<Window<P, C>> {
        // Forget the parent link only for a window that existed: a pending owned
        // window keeps it, so closing its owner later still closes it.
        let window = self.entries.remove(&id)?;
        self.family.forget(id);
        let _ = self.aliases.remove(&window.raw.id());

        Some(window)
    }

    pub fn replace_with(&mut self, mut f: impl FnMut(Window<P, C>) -> Window<P, C>) {
        self.entries = std::mem::take(&mut self.entries)
            .into_iter()
            .map(|(id, window)| (id, f(window)))
            .collect();
    }
}

impl<P, C> Default for Manager<P, C>
where
    P: Program,
    C: Compositor<Renderer = P::Renderer>,
    P::Theme: theme::Base,
{
    fn default() -> Self {
        Self::new()
    }
}

pub struct Window<P, C>
where
    P: Program,
    C: Compositor<Renderer = P::Renderer>,
    P::Theme: theme::Base,
{
    pub raw: Arc<winit::window::Window>,
    pub waker: shell::Waker,
    pub state: State<P>,
    pub exit_on_close_request: bool,
    pub mouse_interaction: mouse::Interaction,
    cursor_grab: CursorGrab,
    release_grab_on_escape: bool,
    pub surface: C::Surface,
    pub surface_version: u64,
    pub surface_error_at: Option<Instant>,
    pub renderer: P::Renderer,
    pub redraw_at: Option<Instant>,
    frame_clock: FrameClock,
    preedit: Option<Preedit<P::Renderer>>,
    ime_state: Option<(Rectangle, input_method::Purpose)>,
    last_frame: Option<Instant>,
    frame_burst: usize,
}

struct FrameClock {
    anchor: Instant,
    period: Option<Duration>,
}

impl FrameClock {
    fn reanchor(&mut self, now: Instant, period: Duration) {
        if self.period != Some(period) {
            log::info!("Frame clock period: {:?} -> {period:?}", self.period);
        }

        self.period = Some(period);
        self.anchor = now - period / 2;
    }

    fn quantize(&self, now: Instant) -> Instant {
        let Some(period) = self.period else {
            return now;
        };

        let elapsed = now.saturating_duration_since(self.anchor);
        let steps = elapsed.as_nanos() / period.as_nanos();
        let snapped = Duration::from_nanos_u128(steps * period.as_nanos());

        self.anchor + snapped
    }
}

impl<P, C> Window<P, C>
where
    P: Program,
    C: Compositor<Renderer = P::Renderer>,
    P::Theme: theme::Base,
{
    pub fn tick(&mut self, now: Instant) -> Instant {
        let mut next = self.frame_clock.quantize(now);

        if let Some(period) = self.frame_clock.period
            && let Some(last) = self.last_frame
        {
            let frame_time = next.saturating_duration_since(last);

            if frame_time > period && self.frame_burst > 1 {
                log::debug!("Frame dropped! (target={period:?}, took={frame_time:?})");

                self.frame_clock.reanchor(now, period);
                next = self.frame_clock.quantize(now);
            }
        }

        self.last_frame = Some(next);
        next
    }

    pub fn reanchor(&mut self, now: Instant) {
        let period = self
            .raw
            .current_monitor()
            .and_then(|monitor| monitor.refresh_rate_millihertz())
            .map(|millihertz| {
                Duration::from_nanos(1_000_000_000_000 / u64::from(millihertz.max(1)))
            });

        if let Some(period) = period {
            self.frame_clock.reanchor(now, period);
        }
    }

    pub fn position(&self) -> Option<Point> {
        self.raw
            .outer_position()
            .ok()
            .map(|position| position.to_logical(self.raw.scale_factor()))
            .map(|position| Point {
                x: position.x,
                y: position.y,
            })
    }

    pub fn configure_surface(&mut self, compositor: &mut C) {
        let physical_size = self.state.physical_size();

        if physical_size.width == 0 || physical_size.height == 0 {
            return;
        }

        compositor.configure_surface(&mut self.surface, physical_size.width, physical_size.height);

        self.renderer.hint(self.state.scale());
        self.surface_version = self.state.surface_version();
    }

    pub fn request_redraw(&mut self, redraw_request: RedrawRequest) {
        match redraw_request {
            RedrawRequest::NextFrame => {
                self.raw.request_redraw();
                self.redraw_at = None;
                self.frame_burst += 1;
            }
            RedrawRequest::At(at) => {
                self.redraw_at = Some(at);
                self.frame_burst = 0;
            }
            RedrawRequest::Wait => {
                self.frame_burst = 0;
            }
        }
    }

    pub fn request_input_method(&mut self, input_method: InputMethod) {
        match input_method {
            InputMethod::Disabled => {
                self.disable_ime();
            }
            InputMethod::Enabled {
                cursor,
                purpose,
                preedit,
            } => {
                self.enable_ime(cursor, purpose);

                if let Some(preedit) = preedit {
                    if preedit.content.is_empty() {
                        self.preedit = None;
                    } else {
                        let mut overlay = self.preedit.take().unwrap_or_else(Preedit::new);

                        overlay.update(
                            cursor,
                            &preedit,
                            self.state.background_color(),
                            &self.renderer,
                        );

                        self.preedit = Some(overlay);
                    }
                } else {
                    self.preedit = None;
                }
            }
        }
    }

    /// Returns the cursor grab currently in effect.
    pub fn cursor_grab(&self) -> CursorGrab {
        self.cursor_grab
    }

    /// Chooses whether Escape releases the cursor grab. Off by default.
    pub fn set_release_grab_on_escape(&mut self, enabled: bool) {
        self.release_grab_on_escape = enabled;
    }

    /// Returns `true` if the runtime should release the cursor grab because
    /// of the given window event.
    pub fn releases_grab(&self, event: &winit::event::WindowEvent) -> bool {
        self.cursor_grab.is_grabbed()
            && releases_grab(
                event,
                ReleasePolicy {
                    on_blur: RELEASE_GRAB_ON_BLUR,
                    on_escape: self.release_grab_on_escape,
                },
            )
    }

    /// Requests a cursor grab, falling back to weaker modes the platform
    /// accepts, and returns the mode that was actually applied. A locked
    /// cursor is hidden (except on the web, where the browser hides it);
    /// releasing it restores visibility unless the current interaction hides
    /// it.
    pub fn set_cursor_grab(&mut self, grab: CursorGrab) -> CursorGrab {
        let applied = apply_cursor_grab(grab, |candidate| {
            self.raw.set_cursor_grab(conversion::cursor_grab(candidate))
        });

        let was_locked = self.cursor_grab == CursorGrab::Locked;
        self.cursor_grab = applied;

        if let Some(visible) = cursor_visibility_after_grab(
            was_locked,
            applied,
            self.mouse_interaction == mouse::Interaction::Hidden,
            HIDE_CURSOR_ON_LOCK,
        ) {
            self.raw.set_cursor_visible(visible);
        }

        applied
    }

    pub fn update_mouse(&mut self, cursors: &Cursors, interaction: mouse::Interaction) {
        if interaction != self.mouse_interaction {
            if let Some(cursor) = cursor_for(cursors, interaction) {
                self.raw.set_cursor(cursor);

                if shows_cursor_on_interaction_change(
                    self.mouse_interaction == mouse::Interaction::Hidden,
                    self.cursor_grab,
                    HIDE_CURSOR_ON_LOCK,
                ) {
                    self.raw.set_cursor_visible(true);
                }
            } else {
                self.raw.set_cursor_visible(false);
            }

            self.mouse_interaction = interaction;
        }
    }

    pub fn draw_preedit(&mut self) {
        if let Some(preedit) = &self.preedit {
            preedit.draw(
                &mut self.renderer,
                self.state.text_color(),
                self.state.background_color(),
                &Rectangle::new(Point::ORIGIN, self.state.viewport().logical_size()),
            );
        }
    }

    fn enable_ime(&mut self, cursor: Rectangle, purpose: input_method::Purpose) {
        if self.ime_state.is_none() {
            self.raw.set_ime_allowed(true);
        }

        if self.ime_state != Some((cursor, purpose)) {
            self.raw.set_ime_cursor_area(
                LogicalPosition::new(cursor.x, cursor.y),
                LogicalSize::new(cursor.width, cursor.height),
            );
            self.raw.set_ime_purpose(conversion::ime_purpose(purpose));

            self.ime_state = Some((cursor, purpose));
        }
    }

    fn disable_ime(&mut self) {
        if self.ime_state.is_some() {
            self.raw.set_ime_allowed(false);
            self.ime_state = None;
        }

        self.preedit = None;
    }
}

struct Preedit<Renderer>
where
    Renderer: text::Renderer,
{
    position: Point,
    content: Renderer::Paragraph,
    spans: Vec<text::Span<'static, ()>>,
}

impl<Renderer> Preedit<Renderer>
where
    Renderer: text::Renderer,
{
    fn new() -> Self {
        Self {
            position: Point::ORIGIN,
            spans: Vec::new(),
            content: Renderer::Paragraph::default(),
        }
    }

    fn update(
        &mut self,
        cursor: Rectangle,
        preedit: &input_method::Preedit,
        background: Color,
        renderer: &Renderer,
    ) {
        self.position = cursor.position() + Vector::new(0.0, cursor.height);

        let background = Color {
            a: 1.0,
            ..background
        };

        let spans = match &preedit.selection {
            Some(selection) => match (
                preedit.content.get(..selection.start),
                preedit.content.get(selection.start..selection.end),
                preedit.content.get(selection.end..),
            ) {
                (Some(prefix), Some(selected), Some(suffix)) => vec![
                    text::Span::new(prefix),
                    text::Span::new(if selection.start == selection.end {
                        "\u{200A}"
                    } else {
                        selected
                    })
                    .color(background),
                    text::Span::new(suffix),
                ],
                _ => vec![text::Span::new(&preedit.content)],
            },
            None => vec![text::Span::new(&preedit.content)],
        };

        if spans != self.spans.as_slice() {
            use text::Paragraph as _;

            self.content = Renderer::Paragraph::with_spans(Text {
                content: &spans,
                bounds: Size::INFINITE,
                size: preedit.text_size.unwrap_or_else(|| renderer.text_size()),
                line_height: renderer.line_height(),
                font: renderer.font(),
                align_x: text::Alignment::Default,
                align_y: alignment::Vertical::Top,
                shaping: text::Shaping::Advanced,
                wrapping: text::Wrapping::None,
                ellipsis: text::Ellipsis::None,
                hint_factor: renderer.hint_factor(),
            });

            self.spans.clear();
            self.spans
                .extend(spans.into_iter().map(text::Span::to_static));
        }
    }

    fn draw(&self, renderer: &mut Renderer, color: Color, background: Color, viewport: &Rectangle) {
        use text::Paragraph as _;

        if self.content.min_width() < 1.0 {
            return;
        }

        let mut bounds = Rectangle::new(
            self.position - Vector::new(0.0, self.content.min_height()),
            self.content.min_bounds(),
        );

        bounds.x = bounds
            .x
            .max(viewport.x)
            .min(viewport.x + viewport.width - bounds.width);

        bounds.y = bounds
            .y
            .max(viewport.y)
            .min(viewport.y + viewport.height - bounds.height);

        renderer.with_layer(bounds, |renderer| {
            let background = Color {
                a: 1.0,
                ..background
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    ..Default::default()
                },
                background,
            );

            renderer.fill_paragraph(&self.content, bounds.position(), color, bounds);

            const UNDERLINE: f32 = 2.0;

            renderer.fill_quad(
                renderer::Quad {
                    bounds: bounds.shrink(Padding {
                        top: bounds.height - UNDERLINE,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                color,
            );

            for span_bounds in self.content.span_bounds(1) {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: span_bounds + (bounds.position() - Point::ORIGIN),
                        ..Default::default()
                    },
                    color,
                );
            }
        });
    }
}

/// Whether the runtime releases the grab when the window loses focus.
///
/// Only macOS: there the lock is global to the system
/// (`CGAssociateMouseAndMouseCursorPosition`) and would outlive the focus.
/// Windows and Wayland re-apply the grab when focus returns, so winit's own
/// handling is left alone.
const RELEASE_GRAB_ON_BLUR: bool = cfg!(target_os = "macos");

/// Whether the runtime hides the cursor itself while it is locked. On the
/// web the browser hides it during pointer lock, and the lock may be refused.
const HIDE_CURSOR_ON_LOCK: bool = !cfg!(target_arch = "wasm32");

/// Whether a change away from `Hidden` makes the cursor visible again. While
/// the runtime itself keeps the cursor hidden for a lock it must stay hidden;
/// where it does not hide on lock (web) nothing else keeps it hidden.
fn shows_cursor_on_interaction_change(
    was_hidden: bool,
    grab: CursorGrab,
    hide_on_lock: bool,
) -> bool {
    was_hidden && !(hide_on_lock && grab == CursorGrab::Locked)
}

/// The custom cursors created with `window::create_cursor`, shared by every
/// window of the application.
pub type Cursors = std::collections::BTreeMap<cursor::Id, winit::window::CustomCursor>;

/// The winit cursor for `interaction`, or `None` to hide the cursor.
///
/// A custom cursor that is not (or no longer) known falls back to the default
/// arrow instead of hiding the cursor.
fn cursor_for(cursors: &Cursors, interaction: mouse::Interaction) -> Option<winit::window::Cursor> {
    match interaction {
        mouse::Interaction::Custom(id) => Some(cursors.get(&id).map_or_else(
            || winit::window::CursorIcon::Default.into(),
            |cursor| cursor.clone().into(),
        )),
        _ => conversion::mouse_interaction(interaction).map(Into::into),
    }
}

/// Whether raw device motion is forwarded to a window. The cheap grab check
/// comes first so windows without a grab never query focus.
pub fn forwards_raw_motion(grab: CursorGrab, has_focus: impl FnOnce() -> bool) -> bool {
    grab.is_grabbed() && has_focus()
}

/// Which window events make the runtime release a cursor grab.
#[derive(Debug, Clone, Copy)]
struct ReleasePolicy {
    on_blur: bool,
    on_escape: bool,
}

fn releases_grab(event: &winit::event::WindowEvent, policy: ReleasePolicy) -> bool {
    use winit::event::WindowEvent;

    match event {
        WindowEvent::Focused(false) => policy.on_blur,
        WindowEvent::KeyboardInput {
            event,
            is_synthetic,
            ..
        } => {
            policy.on_escape
                && is_escape_press(event.state.is_pressed(), &event.logical_key, *is_synthetic)
        }
        _ => false,
    }
}

fn is_escape_press(pressed: bool, key: &winit::keyboard::Key, is_synthetic: bool) -> bool {
    pressed
        && !is_synthetic
        && *key == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape)
}

/// Tries `requested` and then each weaker mode until `set` accepts one, and
/// returns the mode that was applied. If even [`CursorGrab::None`] is
/// refused, nothing was applied and [`CursorGrab::None`] is returned.
fn apply_cursor_grab<E>(
    requested: CursorGrab,
    mut set: impl FnMut(CursorGrab) -> Result<(), E>,
) -> CursorGrab {
    let mut mode = Some(requested);

    while let Some(candidate) = mode {
        if set(candidate).is_ok() {
            return candidate;
        }

        mode = candidate.fallback();
    }

    CursorGrab::None
}

/// Returns the cursor visibility to set after a grab change, if any.
fn cursor_visibility_after_grab(
    was_locked: bool,
    applied: CursorGrab,
    interaction_hidden: bool,
    hide_on_lock: bool,
) -> Option<bool> {
    if applied == CursorGrab::Locked {
        hide_on_lock.then_some(false)
    } else if was_locked && !interaction_hidden {
        Some(true)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use winit::keyboard::{Key, NamedKey};

    #[test]
    fn an_unknown_custom_cursor_falls_back_to_the_default_arrow() {
        let cursors = Cursors::new();

        assert_eq!(
            cursor_for(&cursors, mouse::Interaction::Custom(cursor::Id::unique())),
            Some(winit::window::CursorIcon::Default.into())
        );
        assert_eq!(
            cursor_for(&cursors, mouse::Interaction::Pointer),
            Some(winit::window::CursorIcon::Pointer.into())
        );
        assert_eq!(cursor_for(&cursors, mouse::Interaction::Hidden), None);
    }

    fn policy(on_blur: bool, on_escape: bool) -> ReleasePolicy {
        ReleasePolicy { on_blur, on_escape }
    }

    #[test]
    fn raw_motion_needs_a_grab_and_skips_focus_query_without_one() {
        let none = forwards_raw_motion(CursorGrab::None, || {
            panic!("focus must not be queried without a grab")
        });
        assert!(!none);
        assert!(forwards_raw_motion(CursorGrab::Locked, || true));
        assert!(forwards_raw_motion(CursorGrab::Confined, || true));
        assert!(!forwards_raw_motion(CursorGrab::Locked, || false));
    }

    #[test]
    fn interaction_change_shows_cursor_unless_lock_hides_it() {
        // Native: the lock hides the cursor, keep it hidden.
        assert!(!shows_cursor_on_interaction_change(
            true,
            CursorGrab::Locked,
            true
        ));
        assert!(shows_cursor_on_interaction_change(
            true,
            CursorGrab::Confined,
            true
        ));
        // Web: the runtime did not hide it, so it must be shown again.
        assert!(shows_cursor_on_interaction_change(
            true,
            CursorGrab::Locked,
            false
        ));
        assert!(!shows_cursor_on_interaction_change(
            false,
            CursorGrab::None,
            true
        ));
    }

    #[test]
    fn blur_releases_only_when_enabled() {
        let blur = winit::event::WindowEvent::Focused(false);
        let focus = winit::event::WindowEvent::Focused(true);

        assert!(releases_grab(&blur, policy(true, false)));
        assert!(!releases_grab(&blur, policy(false, true)));
        assert!(!releases_grab(&focus, policy(true, true)));
        assert!(!releases_grab(
            &winit::event::WindowEvent::CursorEntered {
                device_id: winit::event::DeviceId::dummy()
            },
            policy(true, true)
        ));
    }

    #[test]
    fn escape_press_is_a_release_unless_synthetic_or_a_release_event() {
        let escape = Key::Named(NamedKey::Escape);

        assert!(is_escape_press(true, &escape, false));
        assert!(!is_escape_press(true, &escape, true));
        assert!(!is_escape_press(false, &escape, false));
        assert!(!is_escape_press(true, &Key::Named(NamedKey::Enter), false));
    }

    #[test]
    fn fallback_loop_applies_first_accepted_mode() {
        let mut tried = Vec::new();

        let applied = apply_cursor_grab(CursorGrab::Locked, |mode| {
            tried.push(mode);

            if mode == CursorGrab::Locked {
                Err(())
            } else {
                Ok(())
            }
        });

        assert_eq!(applied, CursorGrab::Confined);
        assert_eq!(tried, [CursorGrab::Locked, CursorGrab::Confined]);
    }

    #[test]
    fn fallback_loop_chain_ends_at_none() {
        let mut tried = Vec::new();

        let applied = apply_cursor_grab(CursorGrab::Locked, |mode| {
            tried.push(mode);

            if mode == CursorGrab::None {
                Ok(())
            } else {
                Err(())
            }
        });

        assert_eq!(applied, CursorGrab::None);
        assert_eq!(
            tried,
            [CursorGrab::Locked, CursorGrab::Confined, CursorGrab::None]
        );
    }

    #[test]
    fn fallback_loop_reports_none_when_nothing_applies() {
        let mut calls = 0;

        let applied = apply_cursor_grab(CursorGrab::Confined, |_| {
            calls += 1;
            Err::<(), _>(())
        });

        assert_eq!(applied, CursorGrab::None);
        assert_eq!(calls, 2);
    }

    #[test]
    fn fallback_loop_stops_at_requested_mode_when_accepted() {
        let mut calls = 0;

        let applied = apply_cursor_grab(CursorGrab::Confined, |_| {
            calls += 1;
            Ok::<(), ()>(())
        });

        assert_eq!(applied, CursorGrab::Confined);
        assert_eq!(calls, 1);
    }

    #[test]
    fn locking_hides_the_cursor_unless_the_platform_does() {
        assert_eq!(
            cursor_visibility_after_grab(false, CursorGrab::Locked, false, true),
            Some(false)
        );
        assert_eq!(
            cursor_visibility_after_grab(false, CursorGrab::Locked, false, false),
            None
        );
    }

    #[test]
    fn leaving_a_lock_shows_the_cursor_unless_the_interaction_hides_it() {
        // Locked then degraded to Confined, or released.
        for applied in [CursorGrab::Confined, CursorGrab::None] {
            assert_eq!(
                cursor_visibility_after_grab(true, applied, false, true),
                Some(true)
            );
            assert_eq!(
                cursor_visibility_after_grab(true, applied, true, true),
                None
            );
        }
    }

    #[test]
    fn grab_changes_that_never_locked_leave_visibility_alone() {
        assert_eq!(
            cursor_visibility_after_grab(false, CursorGrab::Confined, false, true),
            None
        );
        assert_eq!(
            cursor_visibility_after_grab(false, CursorGrab::None, false, true),
            None
        );
    }

    #[test]
    fn descendants_are_children_first_and_exclude_unrelated_windows() {
        let (root, child, grandchild, other) =
            (Id::unique(), Id::unique(), Id::unique(), Id::unique());

        let mut family = Family::default();
        family.set_parent(child, root);
        family.set_parent(grandchild, child);

        assert_eq!(family.descendants(root), [grandchild, child]);
        assert_eq!(family.descendants(child), [grandchild]);
        assert!(family.descendants(other).is_empty());
    }

    #[test]
    fn cancelled_windows_are_reported_once_and_forgotten() {
        let (root, child) = (Id::unique(), Id::unique());

        let mut family = Family::default();
        family.set_parent(child, root);
        family.cancel(child);

        assert!(family.descendants(root).is_empty());
        assert!(family.take_cancelled(child));
        assert!(!family.take_cancelled(child));
        assert!(!family.take_cancelled(root));
    }

    #[test]
    fn only_pending_owned_windows_are_cancelled() {
        let (root, open, pending, stray) = (Id::unique(), Id::unique(), Id::unique(), Id::unique());

        let mut family = Family::default();
        family.set_parent(open, root);
        family.set_parent(pending, root);

        let cancelled = family.cancel_pending(&[open, pending, stray, root], |id| id == open);

        assert_eq!(cancelled, [pending]);
        assert!(family.take_cancelled(pending));
        assert!(!family.take_cancelled(stray));
        assert!(!family.take_cancelled(root));
        assert!(!family.take_cancelled(open));
    }

    #[test]
    fn pending_owned_window_is_found_and_cancelled_by_its_owner() {
        let (root, pending) = (Id::unique(), Id::unique());

        let mut family = Family::default();
        family.set_parent(pending, root);

        // While no `forget` ran, the owner still lists the pending window.
        // (`Manager::remove` is not covered: it needs a live window.)
        let descendants = family.descendants(root);
        assert_eq!(descendants, [pending]);

        let cancelled = family.cancel_pending(&descendants, |_| false);

        assert_eq!(cancelled, [pending]);
        assert!(family.take_cancelled(pending));
    }

    #[test]
    fn forgotten_windows_are_no_longer_descendants() {
        let (root, child) = (Id::unique(), Id::unique());

        let mut family = Family::default();
        family.set_parent(child, root);
        family.forget(child);

        assert!(family.descendants(root).is_empty());
    }
}
