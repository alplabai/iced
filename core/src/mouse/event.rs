use crate::{Point, Vector};

use super::Button;

/// A mouse event.
///
/// _**Note:** This type is largely incomplete! If you need to track
/// additional events, feel free to [open an issue] and share your use case!_
///
/// [open an issue]: https://github.com/iced-rs/iced/issues
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// The mouse cursor entered the window.
    CursorEntered,

    /// The mouse cursor left the window.
    CursorLeft,

    /// The mouse cursor was moved
    CursorMoved {
        /// The new position of the mouse cursor
        position: Point,
    },

    /// The mouse moved by a raw, unaccelerated-by-the-cursor amount.
    ///
    /// This is relative motion straight from the device, and keeps arriving
    /// while the cursor is held in place by [`window::CursorGrab::Locked`],
    /// when [`CursorMoved`] stops reporting movement. The unit of `delta` is
    /// device-defined, not logical pixels.
    ///
    /// Only emitted on platforms where the system reports raw device motion,
    /// and only to the focused window.
    ///
    /// [`window::CursorGrab::Locked`]: crate::window::CursorGrab::Locked
    /// [`CursorMoved`]: Self::CursorMoved
    RawMotion {
        /// The change in position since the previous raw motion event.
        delta: Vector,
    },

    /// A mouse button was pressed.
    ButtonPressed(Button),

    /// A mouse button was released.
    ButtonReleased(Button),

    /// The mouse wheel was scrolled.
    WheelScrolled {
        /// The scroll movement.
        delta: ScrollDelta,
    },

    /// A pinch ("magnify") gesture was performed on a trackpad.
    ///
    /// `delta` is the incremental change in magnification since the previous
    /// event: a positive value means the user is zooming in (fingers moving
    /// apart), a negative value zooming out. Useful for pinch-to-zoom.
    PinchGesture {
        /// The incremental magnification change.
        delta: f32,
    },
}

/// A scroll movement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollDelta {
    /// A line-based scroll movement
    Lines {
        /// The number of horizontal lines scrolled
        x: f32,

        /// The number of vertical lines scrolled
        y: f32,
    },
    /// A pixel-based scroll movement
    Pixels {
        /// The number of horizontal pixels scrolled
        x: f32,
        /// The number of vertical pixels scrolled
        y: f32,
    },
}
