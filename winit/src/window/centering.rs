//! Centres a new window on its parent.
use crate::core::{Point, Rectangle, Size};

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

/// Returns the position that centres a window of physical-pixel `size` on
/// `parent`, or `None` to fall back to centring on the monitor.
pub fn on_parent(parent: &winit::window::Window, size: Size<f64>) -> Option<winit::dpi::Position> {
    let origin = parent.outer_position().ok()?;
    let extent = parent.outer_size();

    let monitor = parent.current_monitor().map(|monitor| {
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
