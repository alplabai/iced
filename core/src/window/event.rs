use crate::time::Instant;
use crate::{Point, Size};

use std::path::PathBuf;

/// A window-related event.
#[derive(PartialEq, Clone, Debug)]
pub enum Event {
    /// A window was opened.
    Opened {
        /// The position of the opened window. This is relative to the top-left corner of the desktop
        /// the window is on, including virtual desktops. Refers to window's "outer" position,
        /// or the window area, in logical pixels.
        ///
        /// **Note**: Not available in Wayland.
        position: Option<Point>,
        /// The size of the created window. This is its "inner" size, or the size of the
        /// client area, in logical pixels.
        size: Size,
        /// The scale factor of the created window.
        scale_factor: f32,
    },

    /// A window was closed.
    Closed,

    /// A window was moved.
    Moved(Point),

    /// A window was resized.
    Resized(Size),

    /// A window changed its scale factor.
    Rescaled(f32),

    /// A window changed its maximized, minimized or fullscreen state.
    ///
    /// It is emitted once per change, whether the change was requested by the
    /// application, by the user or by the window manager, and carries the
    /// state after the change. Plain resizes and moves do not emit it. The
    /// flags are read at most once per event loop pass, after the window was
    /// resized, occluded or its focus changed, or after a maximize, minimize,
    /// toggle-maximize or mode action.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS / X11:** `fullscreen` reports the requested state, so it
    ///   flips when the transition starts, not when it ends.
    /// - **macOS:** for undecorated or non-resizable windows `maximized`
    ///   follows only the window actions, not changes made by the user or
    ///   window manager.
    /// - **X11:** minimizing may not produce a window event of its own, so
    ///   it is detected through the focus change that usually accompanies it.
    /// - **Wayland:** `minimized` is always `None`, and `maximized` changes
    ///   once the compositor acknowledges it.
    /// - **Windows:** maximize requests are applied asynchronously, so
    ///   `maximized` changes with the `Resized` that follows the request.
    /// - **Web:** `minimized` is `None`.
    ModeChanged {
        /// Whether the window is maximized.
        maximized: bool,
        /// Whether the window is minimized, or `None` if the platform
        /// cannot tell.
        minimized: Option<bool>,
        /// Whether the window is fullscreen.
        fullscreen: bool,
    },

    /// A window redraw was requested.
    ///
    /// The [`Instant`] contains the current time.
    RedrawRequested(Instant),

    /// The user has requested for the window to close.
    CloseRequested,

    /// A window was focused.
    Focused,

    /// A window was unfocused.
    Unfocused,

    /// A file is being hovered over the window.
    ///
    /// When the user hovers multiple files at once, this event will be emitted
    /// for each file separately.
    ///
    /// ## Platform-specific
    ///
    /// - **Wayland:** Not implemented.
    FileHovered(PathBuf),

    /// A file has been dropped into the window.
    ///
    /// When the user drops multiple files at once, this event will be emitted
    /// for each file separately.
    ///
    /// ## Platform-specific
    ///
    /// - **Wayland:** Not implemented.
    FileDropped(PathBuf),

    /// A file was hovered, but has exited the window.
    ///
    /// There will be a single `FilesHoveredLeft` event triggered even if
    /// multiple files were hovered.
    ///
    /// ## Platform-specific
    ///
    /// - **Wayland:** Not implemented.
    FilesHoveredLeft,
}
