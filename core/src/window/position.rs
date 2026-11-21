use crate::{Point, window::MonitorIndex};

/// Details how a new window should be positioned.
#[derive(Debug, Clone, Copy, Default)]
pub enum Position {
    /// The platform-specific default position for a new window on the default screen.
    #[default]
    Default,
    /// The window is completely centered on the default screen.
    ///
    /// If the window has a [`Settings::parent`](crate::window::Settings::parent),
    /// it is centered on the parent's outer frame instead, kept inside the
    /// parent's monitor. If the platform cannot tell where the parent is
    /// (Wayland), it is centered on the screen.
    Centered,
    /// The windows is positioned with specific coordinates on a given monitor
    Specific(PositionOnMonitor),
}

/// The Position and Monitor of a window.
#[derive(Debug, Clone, Copy, Default)]
pub struct PositionOnMonitor {
    /// The index of the monitor on which the window should be positioned.
    /// If `None`, the window will be positioned on the default screen.
    pub monitor_index: Option<MonitorIndex>,
    /// The window is positioned with specific coordinates: `(X, Y)`.
    ///
    /// When the decorations of the window are enabled, Windows 10 will add some
    /// invisible padding to the window. This padding gets included in the
    /// position. So if you have decorations enabled and want the window to be
    /// at (0, 0) you would have to set the position to
    /// `(PADDING_X, PADDING_Y)`.
    pub position: Point,
}

impl From<Point> for PositionOnMonitor {
    fn from(position: Point) -> Self {
        PositionOnMonitor {
            monitor_index: None,
            position,
        }
    }
}

impl From<PositionOnMonitor> for Position {
    fn from(position_on_monitor: PositionOnMonitor) -> Self {
        Position::Specific(position_on_monitor)
    }
}

impl From<Point> for Position {
    fn from(position: Point) -> Self {
        Position::Specific(position.into())
    }
}
