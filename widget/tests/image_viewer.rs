//! Tests for the image `Viewer` widget.
#![cfg(feature = "image")]
use iced_test::{Simulator, simulator::release_key};
use iced_widget::core::event;
use iced_widget::core::keyboard::{self, key::Named};
use iced_widget::core::{Point, mouse};
use iced_widget::image::{Handle, Viewer};

fn viewer<'a>() -> Simulator<'a, ()> {
    Simulator::new(Viewer::new(Handle::from_rgba(1, 1, vec![0; 4])))
}

fn escape() -> iced_widget::core::Event {
    release_key(keyboard::Key::Named(Named::Escape))
}

/// Escape is left to the application when the viewer is untouched and the
/// cursor is elsewhere.
#[test]
fn escape_is_ignored_when_default_and_cursor_away() {
    let mut simulator = viewer();
    simulator.point_at(Point::new(-100.0, -100.0));

    assert_eq!(simulator.simulate([escape()]), [event::Status::Ignored]);
}

/// Escape over a zoomed viewer resets it and is captured; once reset and
/// with the cursor away it is ignored again.
#[test]
fn escape_resets_a_zoomed_viewer_under_the_cursor() {
    let mut simulator = viewer();
    simulator.point_at(Point::new(0.5, 0.5));
    let _ = simulator.scroll(mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 });

    assert_eq!(simulator.simulate([escape()]), [event::Status::Captured]);

    simulator.point_at(Point::new(-100.0, -100.0));
    assert_eq!(simulator.simulate([escape()]), [event::Status::Ignored]);
}

/// A zoomed viewer is reset by Escape even if the cursor has left it.
#[test]
fn escape_resets_a_zoomed_viewer_with_cursor_away() {
    let mut simulator = viewer();
    simulator.point_at(Point::new(0.5, 0.5));
    let _ = simulator.scroll(mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 });
    simulator.point_at(Point::new(-100.0, -100.0));

    assert_eq!(simulator.simulate([escape()]), [event::Status::Captured]);
}
