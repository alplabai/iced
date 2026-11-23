//! Mark a part of an application-drawn title bar, such as the empty space of a
//! title strip, as a native title bar region.
//!
//! This only has an effect on Windows, in a window created with
//! `extend_content_into_title_bar` in its Windows
//! [`PlatformSpecific`](crate::core::window::settings::PlatformSpecific)
//! settings. There a [`Caption`](window::TitleBarRegion::Caption) region
//! behaves like the native caption: dragging it moves the window and snaps it
//! to screen edges, double-clicking it maximizes the window and right-clicking
//! it opens the system menu. The wrapped content still draws, but receives no
//! mouse input while the regions are active.
//!
//! Wrap the application's maximize button in a
//! [`Maximize`](window::TitleBarRegion::Maximize) region to get the Windows 11
//! Snap Layouts flyout when hovering it. Unlike a caption, the button keeps
//! its mouse input and toggles maximization itself. Report at most one.
//!
//! The regions are dropped while any overlay is open (a menu, a pick list, a
//! tooltip anywhere in the window), so its clicks are not taken by the window.
//! A layer of a [`Stack`](crate::Stack) is not an overlay: while a stacked
//! modal or drawer covers a region, leave the region out of the view, or the
//! clicks there move the window. Do not place a region inside a
//! [`Scrollable`](crate::Scrollable): its bounds ignore the scroll offset.
//!
//! # Example
//! ```no_run
//! # mod iced { pub mod widget { pub use iced_widget::*; } pub use iced_widget::Renderer; pub use iced_widget::core::*; }
//! # use iced::widget::Widget;
//! use iced::widget::{button, row, space, text, title_bar_region};
//! use iced::window::TitleBarRegion;
//!
//! #[derive(Clone)]
//! enum Message {
//!     ToggleMaximize,
//!     Close,
//! }
//!
//! fn title_strip() -> impl Widget<Message> {
//!     row![
//!         text("My app"),
//!         title_bar_region(TitleBarRegion::Caption, space::horizontal()),
//!         title_bar_region(
//!             TitleBarRegion::Maximize,
//!             button("☐").on_press(Message::ToggleMaximize),
//!         ),
//!         button("×").on_press(Message::Close),
//!     ]
//! }
//! ```
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::{self, Operation, Widget};
use crate::core::window;
use crate::core::{Element, Event, Layout, Length, Rectangle, Shell, Size, Vector};

/// Marks its content as a native title bar region. See the
/// [module documentation](self).
pub struct TitleBarRegion<W> {
    region: window::TitleBarRegion,
    content: W,
}

impl<W> TitleBarRegion<W> {
    /// Creates a new [`TitleBarRegion`] of the given kind around the content.
    pub fn new(region: window::TitleBarRegion, content: W) -> Self {
        Self { region, content }
    }
}

impl<W> widget::Meta for TitleBarRegion<W> {}

impl<W, Message, Theme, Renderer> Widget<Message, Theme, Renderer> for TitleBarRegion<W>
where
    W: Widget<Message, Theme, Renderer>,
{
    fn tag(&self) -> tree::Tag {
        self.content.tag()
    }

    fn state(&self) -> tree::State {
        self.content.state()
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        self.content.layout(&mut tree.children[0], renderer, limits);

        tree.size = tree.children[0].size;
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
        let (layout, tree) = layout.iter(&tree.children).next().unwrap();

        self.content
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let (layout, tree) = layout.iter_mut(&mut tree.children).next().unwrap();

        self.content
            .operate(tree, layout, viewport, renderer, operation);
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
        let (layout, tree) = layout.iter_mut(&mut tree.children).next().unwrap();

        if let Event::Window(window::Event::RedrawRequested(_)) = event {
            shell.request_title_bar_region(self.region, layout.bounds());
        }

        self.content
            .update(tree, event, layout, cursor, renderer, shell, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let (layout, tree) = layout.iter(&tree.children).next().unwrap();

        self.content
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
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
        let (layout, tree) = layout.iter_mut(&mut tree.children).next().unwrap();

        self.content
            .overlay(tree, layout, renderer, viewport, translation, window)
    }
}

impl<'a, W, Message, Theme, Renderer> From<TitleBarRegion<W>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a,
    W: Widget<Message, Theme, Renderer> + 'a,
{
    fn from(region: TitleBarRegion<W>) -> Element<'a, Message, Theme, Renderer> {
        region._boxed()
    }
}
