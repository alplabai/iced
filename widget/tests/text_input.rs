//! Tests for the `text_input` widget.
use iced_test::Simulator;
use iced_widget::core::widget::{Id, Operation, operation};
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

#[test]
fn disabling_a_focused_input_blurs() {
    let mut simulator = simulator(true);

    click_at(&mut simulator, Point::new(10.0, 10.0));
    assert_eq!(count(&mut simulator, Message::Focus), 1);

    let mut simulator = simulator.rebuild(view(false));
    click_at(&mut simulator, Point::new(10.0, 500.0));
    assert_eq!(simulator.drain().collect::<Vec<_>>(), vec![Message::Blur]);

    // Clicking outside again does not blur twice.
    click_at(&mut simulator, Point::new(10.0, 500.0));
    assert_eq!(simulator.drain().count(), 0);
}

#[test]
fn focusing_a_disabled_input_by_operation_emits_nothing() {
    let mut simulator = simulator(false);

    simulator.operate(&mut operation::focusable::focus::<()>(Id::new("input")));
    let _ = simulator.simulate([Event::Mouse(mouse::Event::CursorLeft)]);
    assert_eq!(simulator.drain().count(), 0);
}

#[test]
fn focus_next_skips_a_disabled_input() {
    let disabled: iced_widget::TextInput<'_, Pair> = text_input("Placeholder", "")
        .id("input-1")
        .on_focus(Pair::Focus(1))
        .on_blur(Pair::Blur(1));
    let enabled: iced_widget::TextInput<'_, Pair> = text_input("Placeholder", "")
        .id("input-2")
        .on_input(|_| Pair::Input)
        .on_focus(Pair::Focus(2))
        .on_blur(Pair::Blur(2));

    let mut simulator: Simulator<'_, Pair> =
        Simulator::new(iced_widget::column![disabled, enabled]);

    for _ in 0..2 {
        let mut count = operation::focusable::focus_next::<()>();
        simulator.operate(&mut count);

        if let operation::Outcome::Chain(mut next) = count.finish() {
            simulator.operate(next.as_mut());
        }

        let _ = simulator.simulate([Event::Mouse(mouse::Event::CursorLeft)]);
        let messages = simulator.drain().collect::<Vec<_>>();

        if messages == vec![Pair::Focus(2)] {
            return;
        }

        assert!(messages.is_empty(), "{messages:?}");
    }

    panic!("focus never reached the enabled input");
}

#[derive(Debug, Clone, PartialEq)]
enum Pair {
    Input,
    Focus(u8),
    Blur(u8),
}

macro_rules! pair {
    () => {{
        let input = |n: u8| {
            text_input("Placeholder", "")
                .id(format!("input-{n}"))
                .on_input(|_| Pair::Input)
                .on_focus(Pair::Focus(n))
                .on_blur(Pair::Blur(n))
        };

        iced_widget::column![input(1), input(2)]
    }};
}

#[test]
fn focus_next_moves_focus_between_inputs() {
    let mut simulator = Simulator::new(pair!());
    let step = |simulator: &mut Simulator<'_, Pair>| {
        // `focus_next` is a chain: the second stage does the focusing.
        let mut count = operation::focusable::focus_next::<()>();
        simulator.operate(&mut count);

        if let operation::Outcome::Chain(mut next) = count.finish() {
            simulator.operate(next.as_mut());
        }

        let _ = simulator.simulate([Event::Mouse(mouse::Event::CursorLeft)]);
        simulator.drain().collect::<Vec<_>>()
    };

    assert_eq!(step(&mut simulator), vec![Pair::Focus(1)]);

    // Both messages are produced; their relative order is not specified.
    let mut messages = step(&mut simulator);
    messages.sort_by_key(|m| format!("{m:?}"));
    assert_eq!(messages, vec![Pair::Blur(1), Pair::Focus(2)]);
}
