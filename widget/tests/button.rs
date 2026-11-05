//! Tests for keyboard focus on the `button` widget.
use iced_test::{Simulator, simulator::press_key};
use iced_widget::core::keyboard::{self, key::Named};
use iced_widget::core::widget::Operation;
use iced_widget::core::widget::operation::{self, focusable};
use iced_widget::core::{Event, Point, mouse};
use iced_widget::{Theme, button, column, text};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Message {
    First,
    Disabled,
    Last,
}

fn dialog<'a>() -> Simulator<'a, Message> {
    Simulator::new(column![
        button(text("first")).on_press(Message::First),
        button(text("disabled")),
        button(text("last")).on_press(Message::Last),
    ])
}

fn focused(simulator: &mut Simulator<'_, Message>) -> focusable::Count {
    let mut count = focusable::count();
    simulator.operate(&mut operation::black_box(&mut count));

    match count.finish() {
        operation::Outcome::Some(count) => count,
        _ => panic!("count should produce an outcome"),
    }
}

/// Runs an operation to completion, following its chained operations like the
/// runtime does.
fn run(simulator: &mut Simulator<'_, Message>, operation: impl Operation<()> + 'static) {
    let mut operation: Box<dyn Operation<()>> = Box::new(operation);

    loop {
        simulator.operate(&mut operation);

        match operation.finish() {
            operation::Outcome::Chain(next) => operation = next,
            _ => break,
        }
    }
}

fn next(simulator: &mut Simulator<'_, Message>) {
    run(simulator, focusable::focus_next::<()>());
}

fn key(named: Named) -> Event {
    press_key(keyboard::Key::Named(named), None)
}

/// `focus_next` visits enabled buttons in tree order and skips buttons
/// without an `on_press` handler.
#[test]
fn focus_next_skips_disabled_buttons() {
    let mut simulator = dialog();

    assert_eq!(focused(&mut simulator).total, 2);
    assert_eq!(focused(&mut simulator).focused, None);

    next(&mut simulator);
    assert_eq!(focused(&mut simulator).focused, Some(0));

    next(&mut simulator);
    assert_eq!(focused(&mut simulator).focused, Some(1));
}

/// Enter and Space press the focused button, and nothing else.
#[test]
fn enter_and_space_activate_the_focused_button() {
    let mut simulator = dialog();

    let _ = simulator.simulate([key(Named::Enter)]);
    assert_eq!(simulator.drain().count(), 0, "nothing is focused yet");

    next(&mut simulator);
    let _ = simulator.simulate([key(Named::Enter)]);
    let _ = simulator.simulate([key(Named::Space)]);
    assert_eq!(
        simulator.drain().collect::<Vec<_>>(),
        [Message::First, Message::First]
    );

    next(&mut simulator);
    let _ = simulator.simulate([key(Named::Enter)]);
    let messages: Vec<_> = simulator.drain().collect();
    assert_eq!(messages, [Message::Last]);
    assert!(!messages.contains(&Message::Disabled));
}

/// A key press that is not an activation key is left to the application.
#[test]
fn other_keys_are_not_captured() {
    let mut simulator = dialog();
    next(&mut simulator);

    let statuses = simulator.simulate([key(Named::Tab)]);
    assert_eq!(statuses, [iced_widget::core::event::Status::Ignored]);
}

/// Clicking elsewhere removes the focus.
#[test]
fn pressing_outside_unfocuses() {
    let mut simulator = dialog();
    next(&mut simulator);
    assert_eq!(focused(&mut simulator).focused, Some(0));

    simulator.point_at(Point::new(-100.0, -100.0));
    let _ = simulator.simulate([Event::Mouse(mouse::Event::ButtonPressed(
        mouse::Button::Left,
    ))]);

    assert_eq!(focused(&mut simulator).focused, None);
}

/// The focus ring is drawn after keyboard focus, but not after a click.
#[test]
fn focus_ring_is_shown_for_keyboard_focus_only() {
    let reference = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("button_focus_ring.png");
    let _ = std::fs::remove_file(&reference);
    let theme = Theme::Light;

    let mut keyboard = dialog();
    let _ = keyboard.snapshot(&theme).unwrap().matches_image(&reference);
    next(&mut keyboard);
    assert!(
        !keyboard
            .snapshot(&theme)
            .unwrap()
            .matches_image(&reference)
            .unwrap(),
        "keyboard focus should draw a ring"
    );

    let mut pointer = dialog();
    pointer.point_at(Point::new(5.0, 5.0));
    let _ = pointer.simulate([Event::Mouse(mouse::Event::ButtonPressed(
        mouse::Button::Left,
    ))]);
    let _ = pointer.simulate([Event::Mouse(mouse::Event::ButtonReleased(
        mouse::Button::Left,
    ))]);
    pointer.point_at(Point::new(-100.0, -100.0));
    assert_eq!(focused(&mut pointer).focused, Some(0));
    assert!(
        pointer
            .snapshot(&theme)
            .unwrap()
            .matches_image(&reference)
            .unwrap(),
        "pointer focus should not draw a ring"
    );

    let _ = std::fs::remove_file(&reference);
}
