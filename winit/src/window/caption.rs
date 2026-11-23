//! Caption regions on platforms without native caption hit testing.
//!
//! On Windows the winit fork answers `WM_NCHITTEST` for the regions. Elsewhere
//! (Linux, macOS) the press inside a caption region is turned into the window
//! request the native caption would make.
use crate::core::mouse;
use crate::core::{Point, Rectangle};

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

#[cfg(test)]
mod tests {
    use super::*;

    use mouse::click::Kind;

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
