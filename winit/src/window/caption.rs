//! Caption regions on platforms without native caption hit testing.
//!
//! On Windows the winit fork answers `WM_NCHITTEST` for the regions. Elsewhere
//! (Linux, macOS) the press inside a caption region is turned into the window
//! request the native caption would make. On Linux and the BSDs an undecorated
//! window that reports title bar regions also gets resize bands along its
//! edges, since nothing else draws resize borders for it.
use crate::core::mouse;
use crate::core::window::Direction;
use crate::core::{Point, Rectangle, Size};

/// What a press inside a caption region does to the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Move the window with the pointer (`drag_window`).
    Drag,
    /// Maximize the window, or restore it when it is maximized.
    ToggleMaximize,
    /// Open the window menu at the pointer (`show_window_menu`).
    ShowMenu,
}

/// The action for a `button` press of the given click `kind` at `position`,
/// or `None` when the press is outside every caption region, inside the
/// `maximize` region (the application's button wins over a caption, as on
/// Windows) or uses another button. A double click toggles maximization; any
/// other left press drags.
pub fn action(
    regions: &[Rectangle],
    maximize: Option<Rectangle>,
    position: Point,
    button: mouse::Button,
    kind: mouse::click::Kind,
) -> Option<Action> {
    if maximize.is_some_and(|maximize| maximize.contains(position))
        || !regions.iter().any(|region| region.contains(position))
    {
        return None;
    }

    match button {
        mouse::Button::Left if kind == mouse::click::Kind::Double => Some(Action::ToggleMaximize),
        mouse::Button::Left => Some(Action::Drag),
        mouse::Button::Right => Some(Action::ShowMenu),
        _ => None,
    }
}

/// The width of the resize band along each edge of an undecorated window, in
/// logical pixels.
pub const RESIZE_BORDER: f32 = 6.0;

/// How far from a corner, along the edge, a press resizes diagonally.
const RESIZE_CORNER: f32 = 16.0;

/// The resize direction for a press at `position` in a window of `size`, or
/// `None` when it is not within [`RESIZE_BORDER`] of an edge.
pub fn resize_direction(size: Size, position: Point) -> Option<Direction> {
    let Point { x, y } = position;

    let on_edge = x < RESIZE_BORDER
        || y < RESIZE_BORDER
        || x >= size.width - RESIZE_BORDER
        || y >= size.height - RESIZE_BORDER;

    if !on_edge || x < 0.0 || y < 0.0 || x >= size.width || y >= size.height {
        return None;
    }

    let west = x < RESIZE_CORNER;
    let east = x >= size.width - RESIZE_CORNER;
    let north = y < RESIZE_CORNER;
    let south = y >= size.height - RESIZE_CORNER;

    // On an edge but near a corner: the corner wins.
    Some(match (north, south, west, east) {
        (true, _, true, _) => Direction::NorthWest,
        (true, _, _, true) => Direction::NorthEast,
        (_, true, true, _) => Direction::SouthWest,
        (_, true, _, true) => Direction::SouthEast,
        _ if y < RESIZE_BORDER => Direction::North,
        _ if y >= size.height - RESIZE_BORDER => Direction::South,
        _ if x < RESIZE_BORDER => Direction::West,
        _ => Direction::East,
    })
}

/// The cursor shown over a resize band facing `direction`.
pub fn resize_interaction(direction: Direction) -> mouse::Interaction {
    match direction {
        Direction::North | Direction::South => mouse::Interaction::ResizingVertically,
        Direction::East | Direction::West => mouse::Interaction::ResizingHorizontally,
        Direction::NorthEast | Direction::SouthWest => mouse::Interaction::ResizingDiagonallyUp,
        Direction::NorthWest | Direction::SouthEast => mouse::Interaction::ResizingDiagonallyDown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use mouse::click::Kind;

    const WINDOW: Size = Size {
        width: 900.0,
        height: 600.0,
    };

    #[test]
    fn the_edges_resize_and_the_inside_does_not() {
        let at = |x, y| resize_direction(WINDOW, Point::new(x, y));

        assert_eq!(at(450.0, 2.0), Some(Direction::North));
        assert_eq!(at(450.0, 597.0), Some(Direction::South));
        assert_eq!(at(1.0, 300.0), Some(Direction::West));
        assert_eq!(at(899.0, 300.0), Some(Direction::East));
        assert_eq!(at(450.0, 6.0), None);
        assert_eq!(at(450.0, 300.0), None);
        assert_eq!(at(-1.0, 300.0), None);
        assert_eq!(at(900.0, 300.0), None);
    }

    #[test]
    fn near_a_corner_the_edges_resize_diagonally() {
        let at = |x, y| resize_direction(WINDOW, Point::new(x, y));

        assert_eq!(at(2.0, 2.0), Some(Direction::NorthWest));
        assert_eq!(at(10.0, 2.0), Some(Direction::NorthWest));
        assert_eq!(at(2.0, 10.0), Some(Direction::NorthWest));
        assert_eq!(at(890.0, 2.0), Some(Direction::NorthEast));
        assert_eq!(at(2.0, 590.0), Some(Direction::SouthWest));
        assert_eq!(at(897.0, 597.0), Some(Direction::SouthEast));
        assert_eq!(at(20.0, 2.0), Some(Direction::North));
    }

    #[test]
    fn each_direction_has_its_cursor() {
        assert_eq!(
            resize_interaction(Direction::West),
            mouse::Interaction::ResizingHorizontally
        );
        assert_eq!(
            resize_interaction(Direction::South),
            mouse::Interaction::ResizingVertically
        );
        assert_eq!(
            resize_interaction(Direction::NorthEast),
            mouse::Interaction::ResizingDiagonallyUp
        );
        assert_eq!(
            resize_interaction(Direction::SouthEast),
            mouse::Interaction::ResizingDiagonallyDown
        );
    }

    const STRIP: Rectangle = Rectangle {
        x: 120.0,
        y: 0.0,
        width: 300.0,
        height: 36.0,
    };

    #[test]
    fn a_left_press_in_a_region_drags() {
        let action = action(
            &[STRIP],
            None,
            Point::new(200.0, 10.0),
            mouse::Button::Left,
            Kind::Single,
        );

        assert_eq!(action, Some(Action::Drag));
    }

    #[test]
    fn a_double_click_toggles_maximization_and_a_triple_drags() {
        let at = Point::new(200.0, 10.0);

        assert_eq!(
            action(&[STRIP], None, at, mouse::Button::Left, Kind::Double),
            Some(Action::ToggleMaximize)
        );
        assert_eq!(
            action(&[STRIP], None, at, mouse::Button::Left, Kind::Triple),
            Some(Action::Drag)
        );
    }

    #[test]
    fn a_right_press_opens_the_window_menu() {
        let action = action(
            &[STRIP],
            None,
            Point::new(200.0, 10.0),
            mouse::Button::Right,
            Kind::Single,
        );

        assert_eq!(action, Some(Action::ShowMenu));
    }

    #[test]
    fn presses_outside_the_regions_or_with_other_buttons_do_nothing() {
        let inside = Point::new(200.0, 10.0);

        assert_eq!(
            action(
                &[STRIP],
                None,
                Point::new(119.0, 10.0),
                mouse::Button::Left,
                Kind::Single
            ),
            None
        );
        assert_eq!(
            action(
                &[STRIP],
                None,
                Point::new(200.0, 37.0),
                mouse::Button::Left,
                Kind::Single
            ),
            None
        );
        assert_eq!(
            action(&[STRIP], None, inside, mouse::Button::Middle, Kind::Single),
            None
        );
        assert_eq!(
            action(&[], None, inside, mouse::Button::Left, Kind::Single),
            None
        );
    }

    #[test]
    fn the_maximize_region_wins_over_a_caption_region() {
        let maximize = Rectangle {
            x: 380.0,
            y: 0.0,
            width: 40.0,
            height: 36.0,
        };

        assert_eq!(
            action(
                &[STRIP],
                Some(maximize),
                Point::new(400.0, 10.0),
                mouse::Button::Left,
                Kind::Single
            ),
            None
        );
        assert_eq!(
            action(
                &[STRIP],
                Some(maximize),
                Point::new(200.0, 10.0),
                mouse::Button::Left,
                Kind::Single
            ),
            Some(Action::Drag)
        );
    }
}
