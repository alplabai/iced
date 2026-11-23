use crate::Rectangle;

/// The parts of a window's title bar that the application draws itself, in
/// logical window coordinates.
///
/// Only windows that extend their content into the title bar use it; see
/// `extend_content_into_title_bar` in the Windows
/// [`PlatformSpecific`](super::settings::PlatformSpecific) settings. Widgets
/// report the regions through [`Shell::request_title_bar_region`] while
/// handling [`Event::RedrawRequested`](super::Event::RedrawRequested).
///
/// [`Shell::request_title_bar_region`]: crate::Shell::request_title_bar_region
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TitleBar {
    /// The regions that act as the native caption: dragging moves the
    /// window, double-clicking maximizes it and right-clicking opens the
    /// system menu. They receive no mouse input.
    pub caption: Vec<Rectangle>,
}

impl TitleBar {
    /// Adds the regions of `other`.
    pub fn merge(&mut self, other: &Self) {
        self.caption.extend_from_slice(&other.caption);
    }
}

/// The kind of a title bar region reported by a widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TitleBarRegion {
    /// An area that acts as the native caption; see [`TitleBar::caption`].
    Caption,
}
