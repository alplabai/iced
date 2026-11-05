//! Tests for the `text_input` widget.
use iced_test::Simulator;
use iced_widget::core::widget::{Id, operation};
use iced_widget::core::{Event, Point, keyboard, mouse};
use iced_widget::text_input;

#[derive(Debug, Clone, PartialEq)]
enum Message {
    Input(String),
    Focus,
    Blur,
}

fn view<'a>(on_input: bool) -> text_input::TextInput<'a, Message> {
    let input = text_input("Placeholder", "")
        .id("input")
        .on_focus(Message::Focus)
        .on_blur(Message::Blur);

    if on_input {
        input.on_input(Message::Input)
    } else {
        input
    }
}

fn simulator<'a>(on_input: bool) -> Simulator<'a, Message> {
    Simulator::new(view(on_input))
}

fn count(simulator: &mut Simulator<'_, Message>, message: Message) -> usize {
    simulator.drain().filter(|m| *m == message).count()
}

fn click_at(simulator: &mut Simulator<'_, Message>, point: Point) {
    simulator.point_at(point);
    let _ = simulator.simulate(iced_test::simulator::click());
}

#[test]
fn click_focuses_once() {
    let mut simulator = simulator(true);

    click_at(&mut simulator, Point::new(10.0, 10.0));
    assert_eq!(count(&mut simulator, Message::Focus), 1);

    // Clicking again inside does not change the focus.
    click_at(&mut simulator, Point::new(12.0, 10.0));
    let messages: Vec<_> = simulator.drain().collect();
    assert!(messages.is_empty(), "{messages:?}");
}

#[test]
fn click_outside_blurs_once() {
    let mut simulator = simulator(true);

    click_at(&mut simulator, Point::new(10.0, 10.0));
    let _ = simulator.drain().count();

    click_at(&mut simulator, Point::new(10.0, 500.0));
    let messages: Vec<_> = simulator.drain().collect();
    assert_eq!(messages, vec![Message::Blur]);

    // Clicking outside again does not blur twice.
    click_at(&mut simulator, Point::new(10.0, 500.0));
    assert_eq!(simulator.drain().count(), 0);
}

#[test]
fn escape_blurs() {
    let mut simulator = simulator(true);

    click_at(&mut simulator, Point::new(10.0, 10.0));
    let _ = simulator.drain().count();

    let _ = simulator.tap_key(keyboard::key::Named::Escape);
    let messages: Vec<_> = simulator.drain().collect();
    assert_eq!(messages, vec![Message::Blur]);
}

#[test]
fn focus_operations_emit() {
    let mut simulator = simulator(true);

    simulator.operate(&mut operation::focusable::focus::<()>(Id::new("input")));
    // The change is reported on the next event the widget receives.
    let _ = simulator.simulate([Event::Mouse(mouse::Event::CursorLeft)]);
    assert_eq!(simulator.drain().collect::<Vec<_>>(), vec![Message::Focus]);

    simulator.operate(&mut operation::focusable::unfocus::<()>());
    let _ = simulator.simulate([Event::Mouse(mouse::Event::CursorLeft)]);
    assert_eq!(simulator.drain().collect::<Vec<_>>(), vec![Message::Blur]);

    // No state change, no message.
    let _ = simulator.simulate([Event::Mouse(mouse::Event::CursorLeft)]);
    assert_eq!(simulator.drain().count(), 0);
}

#[test]
fn no_message_without_focus_change() {
    let mut simulator = simulator(true);

    let _ = simulator.simulate([Event::Mouse(mouse::Event::CursorLeft)]);
    click_at(&mut simulator, Point::new(10.0, 500.0));
    assert_eq!(simulator.drain().count(), 0);
}

#[test]
fn disabled_input_is_not_focused_by_click() {
    let mut simulator = simulator(false);

    click_at(&mut simulator, Point::new(10.0, 10.0));
    assert_eq!(simulator.drain().count(), 0);
}
