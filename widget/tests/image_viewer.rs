//! Tests for the image `Viewer` widget.
#![cfg(feature = "image")]
use iced_test::{Simulator, simulator::release_key};
use iced_widget::core::event;
use iced_widget::core::keyboard::{self, key::Named};
use iced_widget::core::{ContentFit, Event, Point, mouse};
use iced_widget::image::{Handle, Viewer};

fn viewer<'a>() -> Simulator<'a, ()> {
    Simulator::new(Viewer::new(Handle::from_rgba(1, 1, vec![0; 4])))
}

fn escape() -> Event {
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

/// A hovered viewer in its default state has nothing to reset, so Escape is
/// left to the application (for example a modal that closes on Escape).
#[test]
fn escape_is_ignored_when_default_and_hovered() {
    let mut simulator = viewer();
    simulator.point_at(Point::new(0.5, 0.5));

    assert_eq!(simulator.simulate([escape()]), [event::Status::Ignored]);
}

/// A viewer that was only panned (scale is still 1.0) is not in its default
/// state: Escape resets it and is captured, even with the cursor away.
#[test]
fn escape_resets_a_panned_viewer_with_cursor_away() {
    let mut simulator: Simulator<'_, ()> = Simulator::new(
        Viewer::new(Handle::from_rgba(100, 100, vec![0; 100 * 100 * 4]))
            .content_fit(ContentFit::None)
            .width(10)
            .height(10),
    );

    simulator.point_at(Point::new(5.0, 5.0));

    let _ = simulator.simulate([
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(1.0, 1.0),
        }),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ]);

    simulator.point_at(Point::new(-100.0, -100.0));
    assert_eq!(simulator.simulate([escape()]), [event::Status::Captured]);

    // The pan was reset, so a second Escape is left to the application.
    assert_eq!(simulator.simulate([escape()]), [event::Status::Ignored]);
}

/// Panning away and dragging back leaves the view at its default, even though
/// the drag start offset is stale: Escape is left to the application.
#[test]
fn escape_is_ignored_after_panning_back_to_default() {
    let mut simulator: Simulator<'_, ()> = Simulator::new(
        Viewer::new(Handle::from_rgba(100, 100, vec![0; 100 * 100 * 4]))
            .content_fit(ContentFit::None)
            .width(10)
            .height(10),
    );

    simulator.point_at(Point::new(5.0, 5.0));

    let drag = |to: Point| {
        [
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::CursorMoved { position: to }),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ]
    };

    let _ = simulator.simulate(drag(Point::new(1.0, 1.0)));

    simulator.point_at(Point::new(1.0, 1.0));
    let _ = simulator.simulate(drag(Point::new(5.0, 5.0)));

    simulator.point_at(Point::new(-100.0, -100.0));
    assert_eq!(simulator.simulate([escape()]), [event::Status::Ignored]);
}
