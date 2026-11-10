//! Tests for the primary clipboard (select to copy, middle-click to paste)
//! in `text_input` and `text_editor`.
//!
//! Nothing touches a real clipboard: the `Simulator` records the requests.
use iced_test::Simulator;
use iced_widget::core::clipboard::{self, ClipboardKind};
use iced_widget::core::{Event, Point, keyboard, mouse};
use iced_widget::{text_editor, text_input};

use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
enum Message {
    Input(String),
    Edit(text_editor::Action),
}

const TEXT: &str = "hello world";

fn input<'a>(on_input: bool, secure: bool) -> Simulator<'a, Message> {
    let input = text_input("", TEXT).secure(secure);

    Simulator::new(if on_input {
        input.on_input(Message::Input)
    } else {
        input
    })
}

fn press(simulator: &mut Simulator<'_, Message>, at: Point, button: mouse::Button) {
    simulator.point_at(at);
    let _ = simulator.simulate([Event::Mouse(mouse::Event::ButtonPressed(button))]);
}

fn release(simulator: &mut Simulator<'_, Message>, button: mouse::Button) {
    let _ = simulator.simulate([Event::Mouse(mouse::Event::ButtonReleased(button))]);
}

fn drag(simulator: &mut Simulator<'_, Message>, from: Point, to: Point) {
    press(simulator, from, mouse::Button::Left);

    simulator.point_at(to);
    let _ = simulator.simulate([Event::Mouse(mouse::Event::CursorMoved { position: to })]);

    release(simulator, mouse::Button::Left);
}

fn ctrl(simulator: &mut Simulator<'_, Message>, key: &str) {
    let key = keyboard::Key::Character(key.into());

    let _ = simulator.simulate([Event::Keyboard(keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::COMMAND,
        repeat: false,
        text: None,
    })]);
}

fn text(content: &str) -> (ClipboardKind, clipboard::Content) {
    (
        ClipboardKind::Primary,
        clipboard::Content::Text(content.to_owned()),
    )
}

const START: Point = Point::new(1.0, 10.0);
const END: Point = Point::new(600.0, 10.0);

#[test]
fn text_input_selection_writes_the_primary_clipboard() {
    let mut simulator = input(true, false);

    drag(&mut simulator, START, END);

    let requests = simulator.take_clipboard();
    assert_eq!(requests.write, Some(text(TEXT)));
    assert!(requests.reads.is_empty());
}

#[test]
fn text_input_click_without_selection_writes_nothing() {
    let mut simulator = input(true, false);

    press(&mut simulator, START, mouse::Button::Left);
    release(&mut simulator, mouse::Button::Left);

    assert_eq!(simulator.take_clipboard().write, None);
}

#[test]
fn secure_text_input_selection_never_reaches_the_primary_clipboard() {
    let mut simulator = input(true, true);

    drag(&mut simulator, START, END);

    assert_eq!(simulator.take_clipboard().write, None);
}

#[test]
fn text_input_middle_click_pastes_the_primary_clipboard_at_the_click() {
    let mut simulator = input(true, false);

    press(&mut simulator, START, mouse::Button::Middle);

    let requests = simulator.take_clipboard();
    assert_eq!(
        requests.reads,
        vec![(ClipboardKind::Primary, clipboard::Kind::Text)]
    );
    assert_eq!(requests.write, None);

    let _ = simulator.simulate([Event::Clipboard(clipboard::Event::Read(Ok(Arc::new(
        clipboard::Content::Text("X".to_owned()),
    ))))]);

    // The caret moved to the click (the start) before the paste landed.
    let messages: Vec<_> = simulator.drain().collect();
    assert_eq!(messages, vec![Message::Input(format!("X{TEXT}"))]);
}

#[test]
fn secure_text_input_does_not_paste_the_primary_clipboard() {
    let mut simulator = input(true, true);

    press(&mut simulator, START, mouse::Button::Middle);

    assert!(simulator.take_clipboard().reads.is_empty());
}

#[test]
fn text_input_middle_click_outside_does_nothing() {
    let mut simulator = input(true, false);

    press(
        &mut simulator,
        Point::new(10.0, 500.0),
        mouse::Button::Middle,
    );

    let requests = simulator.take_clipboard();
    assert!(requests.reads.is_empty());
    assert_eq!(requests.write, None);
}

#[test]
fn disabled_text_input_does_not_paste_or_copy() {
    let mut simulator = input(false, false);

    press(&mut simulator, START, mouse::Button::Middle);
    drag(&mut simulator, START, END);

    let requests = simulator.take_clipboard();
    assert!(requests.reads.is_empty());
    assert_eq!(requests.write, None);
}

#[test]
fn standard_copy_and_paste_are_unchanged() {
    let mut simulator = input(true, false);

    drag(&mut simulator, START, END);
    let _ = simulator.take_clipboard();

    ctrl(&mut simulator, "c");
    let requests = simulator.take_clipboard();
    assert_eq!(
        requests.write,
        Some((
            ClipboardKind::Standard,
            clipboard::Content::Text(TEXT.to_owned())
        ))
    );

    ctrl(&mut simulator, "v");
    let requests = simulator.take_clipboard();
    assert_eq!(
        requests.reads,
        vec![(ClipboardKind::Standard, clipboard::Kind::Text)]
    );
}

#[test]
fn a_pending_standard_write_is_not_replaced_by_a_primary_one() {
    let mut pending = clipboard::Clipboard::new();
    pending.write = Some((
        ClipboardKind::Standard,
        clipboard::Content::Text("copied".to_owned()),
    ));

    let mut later = clipboard::Clipboard::new();
    later.write = Some(text("selected"));

    pending.merge(&mut later);

    assert_eq!(
        pending.write,
        Some((
            ClipboardKind::Standard,
            clipboard::Content::Text("copied".to_owned())
        ))
    );
}

fn editor<'a>(content: &'a text_editor::Content, on_action: bool) -> Simulator<'a, Message> {
    let editor = text_editor(content);

    Simulator::new(if on_action {
        editor.on_action(Message::Edit)
    } else {
        editor
    })
}

#[test]
fn text_editor_middle_click_requests_the_primary_clipboard() {
    let content = text_editor::Content::with_text(TEXT);
    let mut simulator = editor(&content, true);

    press(&mut simulator, START, mouse::Button::Middle);

    assert_eq!(
        simulator.take_clipboard().reads,
        vec![(ClipboardKind::Primary, clipboard::Kind::Text)]
    );

    // The caret is moved to the click before the paste.
    let messages: Vec<_> = simulator.drain().collect();
    assert!(
        matches!(
            messages.first(),
            Some(Message::Edit(text_editor::Action::Click(_, _)))
        ),
        "{messages:?}"
    );
}

#[test]
fn read_only_text_editor_does_not_request_the_primary_clipboard() {
    let content = text_editor::Content::with_text(TEXT);
    let mut simulator = editor(&content, false);

    press(&mut simulator, START, mouse::Button::Middle);

    assert!(simulator.take_clipboard().reads.is_empty());
}
