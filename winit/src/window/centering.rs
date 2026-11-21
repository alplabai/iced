//! Centres a new window on its parent or on its monitor.
//!
//! Limits: the window is clamped to the monitor's full rectangle, because
//! winit does not expose the work area (taskbar, dock, menu bar), and the
//! window's inner size is centred, because its decorations are unknown before
//! it exists, so it may sit about half a title bar low.
use crate::core::{Point, Rectangle, Size};

/// The logical size a window is created with: `size` times the program's
/// scale factor.
pub fn logical_size(size: Size, program_scale: f32) -> Size {
    Size::new(size.width * program_scale, size.height * program_scale)
}

/// The physical size of a window of logical `size` on a monitor.
pub fn physical_size(size: Size, monitor_scale: f64) -> Size<f64> {
    Size::new(
        f64::from(size.width) * monitor_scale,
        f64::from(size.height) * monitor_scale,
    )
}

/// Returns the top-left corner that centres a window of physical `size` on
/// the `monitor` rectangle.
pub fn centered_on_monitor(monitor: Rectangle<f64>, size: Size<f64>) -> Point<f64> {
    Point::new(
        monitor.x + (monitor.width - size.width) / 2.0,
        monitor.y + (monitor.height - size.height) / 2.0,
    )
}

/// Returns the top-left corner that centres a window of `size` on the parent
/// frame at `origin` with size `extent`, then keeps it inside `monitor` (a
/// window larger than the monitor is aligned to the monitor's top-left). All
/// values are physical pixels.
///
/// Returns `None` if the platform cannot tell where the parent is (Wayland),
/// so the caller falls back to centring on the monitor.
pub fn centered_on_parent(
    origin: Option<Point<f64>>,
    extent: Size<f64>,
    monitor: Option<Rectangle<f64>>,
    size: Size<f64>,
) -> Option<Point<f64>> {
    let origin = origin?;

    let x = origin.x + (extent.width - size.width) / 2.0;
    let y = origin.y + (extent.height - size.height) / 2.0;

    Some(match monitor {
        Some(monitor) => Point::new(
            clamp(x, monitor.x, monitor.x + monitor.width - size.width),
            clamp(y, monitor.y, monitor.y + monitor.height - size.height),
        ),
        None => Point::new(x, y),
    })
}

fn clamp(value: f64, low: f64, high: f64) -> f64 {
    if high < low {
        low
    } else {
        value.clamp(low, high)
    }
}

/// Returns the position that centres a window of logical `size` (already
/// multiplied by the program's scale factor) on `parent`, or `None` to fall
/// back to centring on the monitor.
pub fn on_parent(parent: &winit::window::Window, size: Size) -> Option<winit::dpi::Position> {
    let origin = parent.outer_position().ok()?;
    let extent = parent.outer_size();

    let monitor = parent.current_monitor();
    let scale = monitor.as_ref().map_or_else(
        || parent.scale_factor(),
        winit::monitor::MonitorHandle::scale_factor,
    );
    let size = physical_size(size, scale);

    let monitor = monitor.map(|monitor| {
        let (position, size) = (monitor.position(), monitor.size());

        Rectangle {
            x: f64::from(position.x),
            y: f64::from(position.y),
            width: f64::from(size.width),
            height: f64::from(size.height),
        }
    });

    let position = centered_on_parent(
        Some(Point::new(f64::from(origin.x), f64::from(origin.y))),
        Size::new(f64::from(extent.width), f64::from(extent.height)),
        monitor,
        size,
    )?;

    Some(winit::dpi::Position::Physical(
        winit::dpi::PhysicalPosition::new(position.x.round() as i32, position.y.round() as i32),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rectangle<f64> = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };

    fn centre(origin: Point<f64>, extent: Size<f64>, size: Size<f64>) -> Option<Point<f64>> {
        centered_on_parent(Some(origin), extent, Some(SCREEN), size)
    }

    #[test]
    fn size_scaling_combines_program_and_monitor_scale() {
        let size = Size::new(800.0, 600.0);

        for (program, monitor, expected) in [
            (1.0, 1.0, Size::new(800.0, 600.0)),
            (1.5, 1.0, Size::new(1200.0, 900.0)),
            (1.0, 2.0, Size::new(1600.0, 1200.0)),
            (1.5, 2.0, Size::new(2400.0, 1800.0)),
        ] {
            assert_eq!(
                physical_size(logical_size(size, program), monitor),
                expected
            );
        }
    }

    #[test]
    fn monitor_centring_uses_the_scaled_size() {
        // A 3840x2160 monitor at scale 2 and an 800x600 window at program
        // scale 1.5 (2400x1800 physical), on a monitor that is not at the origin.
        let monitor = Rectangle {
            x: 100.0,
            y: 50.0,
            width: 3840.0,
            height: 2160.0,
        };

        let size = physical_size(logical_size(Size::new(800.0, 600.0), 1.5), 2.0);

        assert_eq!(centered_on_monitor(monitor, size), Point::new(820.0, 230.0));
        assert_eq!(
            centered_on_monitor(monitor, physical_size(Size::new(800.0, 600.0), 1.0)),
            Point::new(1620.0, 830.0)
        );
    }

    #[test]
    fn parent_centring_uses_the_scaled_size() {
        // Parent at (0, 0) of 2000x1000 physical; a 400x200 dialog at program
        // scale 1.5 and monitor scale 2 is 1200x600 physical.
        let size = physical_size(logical_size(Size::new(400.0, 200.0), 1.5), 2.0);

        let at = centered_on_parent(
            Some(Point::new(0.0, 0.0)),
            Size::new(2000.0, 1000.0),
            Some(Rectangle {
                x: 0.0,
                y: 0.0,
                width: 3840.0,
                height: 2160.0,
            }),
            size,
        );

        assert_eq!(at, Some(Point::new(400.0, 200.0)));
    }

    #[test]
    fn centred_on_the_parent_outer_frame() {
        let at = centre(
            Point::new(100.0, 200.0),
            Size::new(800.0, 600.0),
            Size::new(400.0, 200.0),
        );

        assert_eq!(at, Some(Point::new(300.0, 400.0)));
    }

    #[test]
    fn parent_partly_off_screen_clamps_to_the_monitor() {
        let at = centre(
            Point::new(-500.0, 900.0),
            Size::new(800.0, 600.0),
            Size::new(400.0, 200.0),
        );

        assert_eq!(at, Some(Point::new(0.0, 880.0)));
    }

    #[test]
    fn dialog_bigger_than_parent_is_still_centred_and_clamped() {
        let inside = centre(
            Point::new(500.0, 400.0),
            Size::new(200.0, 100.0),
            Size::new(400.0, 300.0),
        );
        let at_the_edge = centre(
            Point::new(10.0, 10.0),
            Size::new(200.0, 100.0),
            Size::new(400.0, 300.0),
        );

        assert_eq!(inside, Some(Point::new(400.0, 300.0)));
        assert_eq!(at_the_edge, Some(Point::new(0.0, 0.0)));
    }

    #[test]
    fn parent_position_unavailable_falls_back_to_monitor_centring() {
        let at = centered_on_parent(
            None,
            Size::new(800.0, 600.0),
            Some(SCREEN),
            Size::new(400.0, 200.0),
        );

        assert_eq!(at, None);
    }
}
