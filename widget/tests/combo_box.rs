//! Tests for the `combo_box` widget.
use iced_test::Simulator;
use iced_widget::combo_box::{self, ComboBox};
use iced_widget::core::keyboard::key;
use iced_widget::core::{Event, Point, mouse, window};

use std::time::Instant;

/// Moving the hovered option with the keyboard past the bottom of the menu
/// scrolls the menu, so the option under the cursor follows it.
#[test]
fn keyboard_navigation_scrolls_the_menu() {
    let state: &'static combo_box::State<String> = Box::leak(Box::new(combo_box::State::new(
        (0..100).map(|i| format!("option {i:02}")).collect(),
    )));

    let mut simulator: Simulator<'_, String> = Simulator::new(
        ComboBox::new(state, "", None, |option| option)
            .on_option_hovered(|option| option)
            .menu_height(100),
    );

    // Focus the input to open the menu
    simulator.point_at(Point::new(10.0, 10.0));
    let _ = simulator.simulate(iced_test::simulator::click());

    for _ in 0..30 {
        let _ = simulator.tap_key(key::Named::ArrowDown);
        let _ = simulator.simulate([Event::Window(
            window::Event::RedrawRequested(Instant::now()),
        )]);
    }

    assert_eq!(simulator.drain().last().as_deref(), Some("option 30"));

    // Without scrolling, every option under a 100px menu would be one of the
    // first few
    let position = Point::new(10.0, 60.0);
    simulator.point_at(position);
    let _ = simulator.simulate([Event::Mouse(mouse::Event::CursorMoved { position })]);

    let hovered = simulator
        .drain()
        .last()
        .expect("hovering the menu publishes an option");
    let index: usize = hovered["option ".len()..].parse().unwrap();

    assert!(index >= 20, "menu did not scroll: hovered {hovered}");
}
