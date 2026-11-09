//! Lists that only build the rows you can see.
//!
//! A [`VirtualList`] displays `len` rows of a fixed height, but only builds,
//! lays out and draws the rows that intersect its viewport (plus a few rows
//! of overscan). Its cost is therefore proportional to the size of the
//! viewport and not to the number of rows, which makes lists of hundreds of
//! thousands of rows practical.
//!
//! Rows are built on demand by a closure that receives the index of the row.
//! Widget state follows a row key (see [`VirtualList::key`]), so a focused
//! `text_input` keeps its focus and cursor while the window of visible rows
//! slides or while the rows are reordered. By default, the key of a row is
//! its index.
//!
//! The list scrolls with the mouse wheel and exposes itself to
//! [`scrollable`](crate::core::widget::operation::scrollable) operations when
//! given an [`Id`]. Use [`reveal_row`] to bring a row into view.
//!
//! A [`VirtualList`] does not draw a scrollbar. If you need to know its
//! size, wrap it in a [`Sensor`](crate::Sensor).
//!
//! # Sizing
//! A [`VirtualList`] needs a bounded height. Do not put it inside a vertical
//! [`scrollable`](crate::scrollable) (or in a column that is): the viewport
//! would be unbounded and every row would have to be built. If it is laid
//! out with an unbounded height anyway (an infinite maximum, or the
//! `infinite` limits flag a vertical scrollable sets), it falls back to a
//! viewport of 4096 logical pixels instead of building all of its rows.
//!
//! # Precision
//! The scroll offset is an `f32`, like in [`scrollable`](crate::scrollable).
//! Beyond roughly 2^23 pixels of content (about 350 thousand rows of 24
//! pixels) it cannot represent sub-pixel offsets, so slow scrolling deep in
//! such a list may stall. Row positions are computed in `f64` and snapped to
//! the pixel grid of the renderer.
//!
//! # Example
//! ```no_run
//! # mod iced { pub mod widget { pub use iced_widget::*; } pub use iced_widget::Renderer; pub use iced_widget::core::*; }
//! # use iced::widget::Widget;
//! # pub type State = ();
//! use iced::widget::{list, text};
//!
//! enum Message {
//!     // ...
//! }
//!
//! fn view(state: &State) -> impl Widget<Message> {
//!     list::virtual_list(100_000, 24.0, |i| text!("Row {i}"))
//! }
//! ```
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::operation::{self, Animation, scrollable};
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::{Id, Meta, Operation};
use crate::core::{self, Event, Length, Rectangle, Shell, Size, Vector, Widget};

use std::ops::Range;

/// The number of extra rows built on each side of the viewport by default.
const DEFAULT_OVERSCAN: usize = 2;

/// The viewport height used when the list is laid out with an unbounded
/// height, so that it never builds all of its rows.
const FALLBACK_HEIGHT: f32 = 4096.0;

/// Creates a new [`VirtualList`] of `len` rows of height `row_height`, built
/// with the given closure.
pub fn virtual_list<'a, W>(
    len: usize,
    row_height: f32,
    view: impl Fn(usize) -> W + 'a,
) -> VirtualList<'a, W> {
    VirtualList::new(len, row_height, view)
}

/// Produces an [`Operation`] that scrolls the [`VirtualList`] with the given
/// [`Id`] by the minimum amount needed to bring the row at `index` into view.
pub fn reveal_row<T>(id: Id, index: usize, row_height: f32) -> impl Operation<T> {
    // Only the vertical axis is revealed, so that the horizontal scroll of
    // any other scrollable with the same `Id` is left untouched.
    scrollable::reveal_vertical(
        id,
        (index as f64 * row_height as f64) as f32,
        row_height,
        Animation::Instant,
    )
}

/// A list of fixed-height rows that only builds the visible ones.
///
/// See the [module documentation](self) for more details.
pub struct VirtualList<'a, W> {
    id: Option<Id>,
    len: usize,
    row_height: f32,
    overscan: usize,
    width: Length,
    height: Length,
    view: Box<dyn Fn(usize) -> W + 'a>,
    key: Option<Box<dyn Fn(usize) -> u64 + 'a>>,
    rows: Vec<W>,
}

impl<'a, W> VirtualList<'a, W> {
    /// Creates a new [`VirtualList`] of `len` rows of height `row_height`.
    ///
    /// The `view` closure builds the row at the given index. It is only
    /// called for rows that are (nearly) visible.
    ///
    /// # Panics
    /// Panics if `row_height` is not a positive, finite number.
    pub fn new(len: usize, row_height: f32, view: impl Fn(usize) -> W + 'a) -> Self {
        assert!(
            row_height.is_finite() && row_height > 0.0,
            "VirtualList requires a positive, finite row height"
        );

        Self {
            id: None,
            len,
            row_height,
            overscan: DEFAULT_OVERSCAN,
            width: Length::Fill,
            height: Length::Fill,
            view: Box::new(view),
            key: None,
            rows: Vec::new(),
        }
    }

    /// Sets the [`Id`] of the [`VirtualList`], so it can be targeted by
    /// scrolling [`Operation`]s.
    pub fn id(mut self, id: impl Into<Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the width of the [`VirtualList`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`VirtualList`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the number of rows built beyond each edge of the viewport.
    ///
    /// The default is 2.
    pub fn overscan(mut self, rows: usize) -> Self {
        self.overscan = rows;
        self
    }

    /// Sets a function that returns a stable key for the row at the given
    /// index.
    ///
    /// Widget state (focus, cursor position, ...) follows the key of a row,
    /// even if the row changes index. Keys should be unique; if two visible
    /// rows share a key, only one of them keeps its state.
    ///
    /// By default, the key of a row is its index.
    pub fn key(mut self, key: impl Fn(usize) -> u64 + 'a) -> Self {
        self.key = Some(Box::new(key));
        self
    }

    fn key_of(&self, index: usize) -> u64 {
        self.key.as_ref().map_or(index as u64, |key| key(index))
    }

    fn content_height(&self) -> f32 {
        self.len as f32 * self.row_height
    }
}

/// Returns the range of rows to build for the given scroll `offset` and
/// `viewport` height.
fn visible_range(
    offset: f32,
    viewport: f32,
    row_height: f32,
    len: usize,
    overscan: usize,
) -> Range<usize> {
    let first = (offset / row_height).floor().max(0.0) as usize;
    let last = ((offset + viewport) / row_height).ceil().max(0.0) as usize;

    let start = first.saturating_sub(overscan).min(len);
    let end = last.saturating_add(overscan).min(len);

    start..end
}

/// Returns the translation of the row at `index` relative to the list,
/// snapped to the pixel grid given by the `hint` factor.
fn row_translation(index: usize, row_height: f32, offset: f32, hint: f32) -> Vector {
    let y = (index as f64 * row_height as f64 - offset as f64) as f32;

    Vector::new(0.0, y).hint(hint)
}

#[derive(Debug, Default)]
struct State {
    /// The requested scroll offset.
    offset: f32,
    /// The scroll offset the current rows were laid out with.
    laid_out: f32,
    /// The keys of the rows the children of the tree belong to.
    keys: Vec<u64>,
    /// The maximum scroll offset at the last layout.
    max_offset: f32,
    /// The range of rows the children of the tree were built for.
    range: Range<usize>,
}

impl State {
    /// Moves the already built rows to the requested offset without a
    /// relayout. Returns `false` if the offset needs rows that are not built,
    /// in which case nothing changes and the layout must be invalidated.
    #[allow(clippy::too_many_arguments)]
    fn shift_rows(
        &mut self,
        children: &mut [Tree],
        viewport: f32,
        row_height: f32,
        len: usize,
        overscan: usize,
        hint: f32,
    ) -> bool {
        let range = visible_range(self.offset, viewport, row_height, len, overscan);

        if range != self.range || range.len() != children.len() {
            return false;
        }

        for (index, child) in range.zip(children) {
            child.translation = row_translation(index, row_height, self.offset, hint);
        }

        self.laid_out = self.offset;

        true
    }
}

impl operation::Scrollable for State {
    fn snap_to(
        &mut self,
        offset: scrollable::RelativeOffset<Option<f32>>,
        _animation: Animation,
        _bounds: Rectangle,
        _content: Size,
    ) {
        if let Some(y) = offset.y {
            self.offset = (y * self.max_offset).clamp(0.0, self.max_offset);
        }
    }

    fn scroll_to(
        &mut self,
        offset: scrollable::AbsoluteOffset<Option<f32>>,
        _animation: Animation,
        _bounds: Rectangle,
        _content: Size,
    ) {
        if let Some(y) = offset.y {
            self.offset = y.clamp(0.0, self.max_offset);
        }
    }

    fn scroll_by(
        &mut self,
        offset: scrollable::AbsoluteOffset,
        _animation: Animation,
        _bounds: Rectangle,
        _content: Size,
    ) {
        self.offset = (self.offset + offset.y).clamp(0.0, self.max_offset);
    }
}

impl<W> Meta for VirtualList<'_, W> {}

impl<W, Message, Theme, Renderer> Widget<Message, Theme, Renderer> for VirtualList<'_, W>
where
    Renderer: core::Renderer,
    W: Widget<Message, Theme, Renderer>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn diff(&mut self, _tree: &mut Tree) {
        // Rows are built and diffed during layout, once the viewport is known.
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        let limits = limits.width(self.width).height(self.height);
        let total = self.content_height();

        let mut size = limits.resolve(self.width, self.height, Size::new(0.0, total));

        // A vertical `scrollable` does not hand out an infinite maximum, it
        // sets the `infinite` flag on a finite viewport height, so the flag
        // has to be checked as well as the resolved size.
        if limits.infinite.height || !size.height.is_finite() {
            // An unbounded height would build every row.
            size.height = total.min(FALLBACK_HEIGHT);
        }

        let Tree {
            state,
            children,
            size: tree_size,
            ..
        } = tree;

        let state = state.downcast_mut::<State>();

        state.max_offset = (total - size.height).max(0.0);
        state.offset = state.offset.clamp(0.0, state.max_offset);
        state.laid_out = state.offset;

        let range = visible_range(
            state.offset,
            size.height,
            self.row_height,
            self.len,
            self.overscan,
        );

        let keys: Vec<u64> = range.clone().map(|index| self.key_of(index)).collect();
        self.rows = range.clone().map(|index| (self.view)(index)).collect();

        // Reconcile the children by key, so that widget state follows the row
        // while the window slides or the rows are reordered.
        let mut old: Vec<Option<Tree>> = std::mem::take(children).into_iter().map(Some).collect();

        *children = keys
            .iter()
            .zip(&self.rows)
            .map(|(key, row)| {
                state
                    .keys
                    .iter()
                    .zip(old.iter())
                    .position(|(old_key, tree)| old_key == key && tree.is_some())
                    .and_then(|index| old[index].take())
                    .unwrap_or_else(|| Tree::new(row))
            })
            .collect();

        state.keys = keys;
        state.range = range.clone();

        let hint = renderer.hint_factor().unwrap_or(1.0);

        let row_limits = layout::Limits::new(
            Size::new(size.width, self.row_height),
            Size::new(size.width, self.row_height),
        );

        for (index, (row, child)) in range.zip(self.rows.iter_mut().zip(children.iter_mut())) {
            Tree::diff(child, row);

            row.layout(child, renderer, &row_limits);

            child.translation = row_translation(index, self.row_height, state.offset, hint);
        }

        *tree_size = size;
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let bounds = layout.bounds();
        let content = Size::new(bounds.width, self.content_height());

        let state = tree.state.downcast_mut::<State>();
        let translation = Vector::new(0.0, state.offset);

        operation.scrollable(self.id.as_ref(), bounds, content, translation, state);

        let viewport = viewport.intersection(&bounds).unwrap_or_default();

        operation.traverse(&mut |operation| {
            self.rows
                .iter_mut()
                .zip(layout.iter_mut(&mut tree.children))
                .for_each(|(row, (layout, tree))| {
                    row.operate(tree, layout, &viewport, renderer, operation);
                });
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let is_over = cursor.is_over(bounds);

        let hint = renderer.hint_factor().unwrap_or(1.0);

        {
            let Tree {
                state, children, ..
            } = &mut *tree;
            let state = state.downcast_mut::<State>();

            if (state.offset - state.laid_out).abs() > f32::EPSILON {
                // The offset was changed by an operation. Only a change of
                // the visible rows needs a relayout.
                if !state.shift_rows(
                    children,
                    bounds.height,
                    self.row_height,
                    self.len,
                    self.overscan,
                    hint,
                ) {
                    shell.invalidate_layout();
                }

                shell.request_redraw();
            }
        }

        let cursor = if is_over {
            cursor
        } else {
            mouse::Cursor::Unavailable
        };

        let viewport = viewport.intersection(&bounds).unwrap_or_default();

        for (row, (layout, tree)) in self
            .rows
            .iter_mut()
            .zip(layout.iter_mut(&mut tree.children))
        {
            row.update(tree, event, layout, cursor, renderer, shell, &viewport);
        }

        if shell.is_event_captured() || !is_over {
            return;
        }

        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
            let Tree {
                state, children, ..
            } = &mut *tree;
            let state = state.downcast_mut::<State>();

            let dy = match *delta {
                mouse::ScrollDelta::Lines { y, .. } => y * self.row_height,
                mouse::ScrollDelta::Pixels { y, .. } => y,
            };

            let offset = (state.offset - dy).clamp(0.0, state.max_offset);

            if (offset - state.offset).abs() > f32::EPSILON {
                state.offset = offset;

                // The rows are positioned by their translation, so a layout
                // is only needed when a new row becomes visible.
                if !state.shift_rows(
                    children,
                    bounds.height,
                    self.row_height,
                    self.len,
                    self.overscan,
                    hint,
                ) {
                    shell.invalidate_layout();
                }

                shell.request_redraw();
                shell.capture_event();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();

        if !cursor.is_over(bounds) {
            return mouse::Interaction::None;
        }

        let viewport = viewport.intersection(&bounds).unwrap_or_default();

        self.rows
            .iter()
            .zip(layout.iter(&tree.children))
            .map(|(row, (layout, tree))| {
                row.mouse_interaction(tree, layout, cursor, &viewport, renderer)
            })
            .max()
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        let Some(viewport) = viewport.intersection(&bounds) else {
            return;
        };

        let cursor = if cursor.is_over(bounds) {
            cursor
        } else {
            mouse::Cursor::Unavailable
        };

        // Clip to the visible part of the list, not to its full bounds, so that
        // rows never paint over what clips the list.
        renderer.with_layer(viewport, |renderer| {
            for (row, (layout, tree)) in self.rows.iter().zip(layout.iter(&tree.children)) {
                if layout.bounds().intersects(&viewport) {
                    row.draw(tree, renderer, theme, style, layout, cursor, &viewport);
                }
            }
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        let bounds = layout.bounds();

        // Rows that were built as overscan are off screen: their overlays
        // must not outlive them.
        self.rows
            .iter_mut()
            .zip(layout.iter_mut(&mut tree.children))
            .filter(|(_, (layout, _))| layout.bounds().intersects(&bounds))
            .flat_map(|(row, (layout, state))| {
                row.overlay(state, layout, renderer, viewport, translation, window)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_covers_viewport_plus_overscan() {
        // Rows 10..15 are visible; two extra on each side.
        assert_eq!(visible_range(100.0, 50.0, 10.0, 1000, 2), 8..17);
    }

    #[test]
    fn range_includes_partially_visible_rows() {
        assert_eq!(visible_range(105.0, 50.0, 10.0, 1000, 0), 10..16);
    }

    #[test]
    fn range_is_clamped_to_the_list() {
        assert_eq!(visible_range(0.0, 50.0, 10.0, 1000, 2), 0..7);
        assert_eq!(visible_range(9950.0, 50.0, 10.0, 1000, 2), 993..1000);
        assert_eq!(visible_range(0.0, 50.0, 10.0, 3, 2), 0..3);
        assert_eq!(visible_range(0.0, 50.0, 10.0, 0, 2), 0..0);
    }
}
