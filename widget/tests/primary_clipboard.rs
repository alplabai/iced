//! Tests for the primary clipboard (select to copy, middle-click to paste)
//! in `text_input` and `text_editor`.
//!
//! Nothing touches a real clipboard: the `Simulator` records the requests.
//! The feature only exists where `primary_selection_supported()` holds; the
//! tests assert the matching behaviour for whichever host runs them.
use iced_test::Simulator;
use iced_widget::core::clipboard::{self, ClipboardKind, primary_selection_supported};
use iced_widget::core::text::editor::{self, Action, Binding, Editor as _, Update};
use iced_widget::core::{Event, Font, Padding, Pixels, Point, Rectangle, keyboard, mouse, text};
use iced_widget::core::{event, window};
use iced_widget::{column, text_editor, text_input};

use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
enum Message {
    Input(u8, String),
    Focus(u8),
    Edit(text_editor::Action),
}

const TEXT: &str = "hello world";

fn field<'a>(
    n: u8,
    value: &'a str,
    on_input: bool,
    secure: bool,
) -> text_input::TextInput<'a, Message> {
    let input = text_input("", value)
        .secure(secure)
        .on_focus(Message::Focus(n));

    if on_input {
        input.on_input(move |text| Message::Input(n, text))
    } else {
        input
    }
}

fn input<'a>(on_input: bool, secure: bool) -> Simulator<'a, Message> {
    Simulator::new(field(1, TEXT, on_input, secure))
}

/// Two inputs, far enough apart for `A_AT` and `B_AT` to hit one each.
fn pair<'a>() -> Simulator<'a, Message> {
    Simulator::new(
        column![field(1, "aaaa", true, false), field(2, "bbbb", true, false)].spacing(40),
    )
}

const A_AT: Point = Point::new(1.0, 10.0);
const B_AT: Point = Point::new(1.0, 90.0);

fn press(simulator: &mut Simulator<'_, Message>, at: Point, button: mouse::Button) {
    simulator.point_at(at);
    let _ = simulator.simulate([Event::Mouse(mouse::Event::ButtonPressed(button))]);
}

fn release(simulator: &mut Simulator<'_, Message>, button: mouse::Button) {
    let _ = simulator.simulate([Event::Mouse(mouse::Event::ButtonReleased(button))]);
}

fn click(simulator: &mut Simulator<'_, Message>, at: Point) {
    press(simulator, at, mouse::Button::Left);
    release(simulator, mouse::Button::Left);
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

fn primary(content: &str) -> Option<(ClipboardKind, clipboard::Content)> {
    Some((
        ClipboardKind::Primary,
        clipboard::Content::Text(content.to_owned()),
    ))
}

/// Picks the expectation for this platform.
fn on_supported<T>(supported: T, unsupported: T) -> T {
    if primary_selection_supported() {
        supported
    } else {
        unsupported
    }
}

fn inputs(simulator: &mut Simulator<'_, Message>) -> Vec<Message> {
    simulator
        .drain()
        .filter(|message| matches!(message, Message::Input(..)))
        .collect()
}

fn paste(simulator: &mut Simulator<'_, Message>, text: &str) {
    let _ = simulator.simulate([Event::Clipboard(clipboard::Event::Read(Ok(Arc::new(
        clipboard::Content::Text(text.to_owned()),
    ))))]);
}

const START: Point = Point::new(1.0, 10.0);
const END: Point = Point::new(600.0, 10.0);

#[test]
fn text_input_selection_writes_the_primary_clipboard() {
    let mut simulator = input(true, false);

    drag(&mut simulator, START, END);

    let requests = simulator.take_clipboard();
    assert_eq!(requests.write, on_supported(primary(TEXT), None));
    assert!(requests.reads.is_empty());
}

#[test]
fn text_input_double_click_writes_the_word() {
    let mut simulator = input(true, false);

    click(&mut simulator, START);
    click(&mut simulator, START);

    assert_eq!(
        simulator.take_clipboard().write,
        on_supported(primary("hello"), None)
    );
}

#[test]
fn text_input_triple_click_writes_the_line() {
    let mut simulator = input(true, false);

    click(&mut simulator, START);
    click(&mut simulator, START);
    click(&mut simulator, START);

    assert_eq!(
        simulator.take_clipboard().write,
        on_supported(primary(TEXT), None)
    );
}

#[test]
fn text_input_click_without_selection_writes_nothing() {
    let mut simulator = input(true, false);

    click(&mut simulator, START);

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
        on_supported(
            vec![(ClipboardKind::Primary, clipboard::Kind::Text)],
            vec![]
        )
    );
    assert_eq!(requests.write, None);

    paste(&mut simulator, "X");

    // The caret moved to the click (the start) before the paste landed.
    assert_eq!(
        inputs(&mut simulator),
        on_supported(vec![Message::Input(1, format!("X{TEXT}"))], vec![])
    );
}

#[test]
fn text_input_middle_click_is_only_captured_and_focused_where_supported() {
    let mut simulator = input(true, false);

    simulator.point_at(START);
    let statuses = simulator.simulate([Event::Mouse(mouse::Event::ButtonPressed(
        mouse::Button::Middle,
    ))]);

    let captured = statuses.contains(&event::Status::Captured);
    assert_eq!(captured, primary_selection_supported());

    let focused = simulator
        .drain()
        .any(|message| message == Message::Focus(1));
    assert_eq!(focused, primary_selection_supported());
}

#[test]
fn text_input_middle_click_on_another_input_unfocuses_the_focused_one() {
    let mut simulator = pair();

    click(&mut simulator, B_AT);
    let _ = simulator.drain().count();

    press(&mut simulator, A_AT, mouse::Button::Middle);
    paste(&mut simulator, "X");
    let _ = simulator.typewrite("z");

    // Only the focused input receives the paste and the typing.
    let focused = on_supported(1, 2);
    let messages = inputs(&mut simulator);

    assert!(!messages.is_empty());
    assert!(
        messages
            .iter()
            .all(|message| matches!(message, Message::Input(n, _) if *n == focused)),
        "{messages:?}"
    );
}

#[test]
fn text_input_middle_click_on_another_input_unfocuses_a_focused_secure_one() {
    let mut simulator = Simulator::new(
        column![field(1, "aaaa", true, false), field(2, "bbbb", true, true)].spacing(40),
    );

    click(&mut simulator, B_AT);
    let _ = simulator.drain().count();

    press(&mut simulator, A_AT, mouse::Button::Middle);
    paste(&mut simulator, "X");
    let _ = simulator.typewrite("z");

    // Where supported the plain input took the focus; the secure one must
    // receive neither the paste nor the typing.
    let messages = inputs(&mut simulator);

    assert!(
        messages
            .iter()
            .all(|message| matches!(message, Message::Input(n, _) if *n == on_supported(1, 2))),
        "{messages:?}"
    );
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
fn secure_text_input_middle_click_does_not_move_the_caret_or_paste() {
    let mut simulator = Simulator::new(field(1, "abcdefgh", true, true));

    // Focus with the caret at the end, then middle-click at the start.
    click(&mut simulator, END);
    let _ = simulator.drain().count();
    let _ = simulator.take_clipboard();

    press(&mut simulator, START, mouse::Button::Middle);
    assert!(simulator.take_clipboard().reads.is_empty());

    let _ = simulator.typewrite("Z");

    assert_eq!(
        inputs(&mut simulator),
        vec![Message::Input(1, "abcdefghZ".to_owned())]
    );
}

#[test]
fn secure_text_input_middle_click_does_not_focus_it() {
    let mut simulator = input(true, true);

    press(&mut simulator, START, mouse::Button::Middle);
    let _ = simulator.typewrite("Z");

    let messages: Vec<_> = simulator.drain().collect();
    assert!(messages.is_empty(), "{messages:?}");
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
    later.write = primary("selected");

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
        on_supported(
            vec![(ClipboardKind::Primary, clipboard::Kind::Text)],
            vec![]
        )
    );

    // The caret is moved to the click before the paste.
    let messages: Vec<_> = simulator.drain().collect();
    let clicked = matches!(
        messages.first(),
        Some(Message::Edit(text_editor::Action::Click(_, _)))
    );
    assert_eq!(clicked, primary_selection_supported(), "{messages:?}");
}

#[test]
fn read_only_text_editor_does_not_request_the_primary_clipboard() {
    let content = text_editor::Content::with_text(TEXT);
    let mut simulator = editor(&content, false);

    press(&mut simulator, START, mouse::Button::Middle);

    assert!(simulator.take_clipboard().reads.is_empty());
}

// `text_editor` publishes its actions as messages, which the application
// applies after the event batch. These tests drive the shared editor state
// by hand to control that order.
type Ed = <iced_widget::Renderer as text::Renderer>::Editor;

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 400.0,
    height: 100.0,
};

fn laid_out() -> Ed {
    let mut editor = Ed::with_text(TEXT);

    editor.update(
        BOUNDS.size(),
        Font::default(),
        Pixels(16.0),
        text::LineHeight::default(),
        text::Wrapping::None,
        text::Alignment::Default,
        None,
        &mut text::parser::PlainText,
    );

    editor
}

fn feed(state: &mut editor::State, editor: &Ed, event: Event, at: Point) -> Option<Update<()>> {
    state.update(
        editor,
        &event,
        BOUNDS,
        Padding::ZERO,
        mouse::Cursor::Available(at),
        |_| None::<Binding<()>>,
    )
}

fn left(event: fn(mouse::Button) -> mouse::Event) -> Event {
    Event::Mouse(event(mouse::Button::Left))
}

fn redraw() -> Event {
    Event::Window(window::Event::RedrawRequested(std::time::Instant::now()))
}

/// Applies an update to the editor, as the application would, returning the
/// text it asked to copy to the primary clipboard.
fn apply(editor: &mut Ed, update: Option<Update<()>>) -> Vec<String> {
    fn go(editor: &mut Ed, update: Update<()>, copied: &mut Vec<String>) {
        match update {
            Update::Action(action) => editor.perform(action),
            Update::CopyPrimary(text) => copied.push(text),
            Update::Sequence(updates) => {
                for update in updates {
                    go(editor, update, copied);
                }
            }
            _ => {}
        }
    }

    let mut copied = Vec::new();

    if let Some(update) = update {
        go(editor, update, &mut copied);
    }

    copied
}

#[test]
fn text_editor_writes_the_new_selection_once_the_application_applied_the_drag() {
    let mut editor = laid_out();
    let mut state = editor::State::with_deferred_primary_copy().with_primary_selection(true);
    let to = Point::new(40.0, 10.0);

    // An old selection is left in the content.
    editor.perform(Action::SelectAll);

    // Press, drag and release in one batch: nothing is applied in between.
    let pressed = feed(
        &mut state,
        &editor,
        left(mouse::Event::ButtonPressed),
        START,
    );
    let moved = feed(
        &mut state,
        &editor,
        Event::Mouse(mouse::Event::CursorMoved { position: to }),
        to,
    );
    let released = feed(&mut state, &editor, left(mouse::Event::ButtonReleased), to);

    // The stale selection must not be written at release.
    assert!(!matches!(released, Some(Update::CopyPrimary(_))));

    // The application applies the batch...
    assert!(apply(&mut editor, pressed).is_empty());
    assert!(apply(&mut editor, moved).is_empty());

    // ...and the next redraw writes the new selection.
    let selected = editor.copy().expect("a selection");
    assert_ne!(selected, TEXT);

    let update = feed(&mut state, &editor, redraw(), to);

    assert_eq!(apply(&mut editor, update), vec![selected]);
}

#[test]
fn text_editor_double_click_writes_the_word_on_the_next_redraw() {
    let mut editor = laid_out();
    let mut state = editor::State::with_deferred_primary_copy().with_primary_selection(true);

    for _ in 0..2 {
        let pressed = feed(
            &mut state,
            &editor,
            left(mouse::Event::ButtonPressed),
            START,
        );
        let released = feed(
            &mut state,
            &editor,
            left(mouse::Event::ButtonReleased),
            START,
        );

        // Both events are handled before the application applies anything.
        assert!(!matches!(released, Some(Update::CopyPrimary(_))));
        let _ = apply(&mut editor, pressed);
    }

    let update = feed(&mut state, &editor, redraw(), START);

    assert_eq!(apply(&mut editor, update), vec!["hello".to_owned()]);
}

#[test]
fn text_input_state_writes_the_selection_at_release() {
    let mut editor = laid_out();
    let mut state = editor::State::new().with_primary_selection(true);
    let to = Point::new(300.0, 10.0);

    let pressed = feed(
        &mut state,
        &editor,
        left(mouse::Event::ButtonPressed),
        START,
    );
    let _ = apply(&mut editor, pressed);

    let moved = feed(
        &mut state,
        &editor,
        Event::Mouse(mouse::Event::CursorMoved { position: to }),
        to,
    );
    let _ = apply(&mut editor, moved);

    let released = feed(&mut state, &editor, left(mouse::Event::ButtonReleased), to);

    assert_eq!(apply(&mut editor, released), vec![TEXT.to_owned()]);
}

#[test]
fn state_middle_click_on_another_widget_unfocuses_this_one() {
    let editor = laid_out();
    let mut state = editor::State::new().with_primary_selection(true);
    let outside = Point::new(500.0, 500.0);

    let _ = feed(
        &mut state,
        &editor,
        left(mouse::Event::ButtonPressed),
        START,
    );
    assert!(state.is_focused());

    let update = feed(
        &mut state,
        &editor,
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)),
        outside,
    );

    assert!(matches!(update, Some(Update::Unfocus)));
    assert!(!state.is_focused());
}

#[test]
fn state_middle_click_does_nothing_where_the_primary_selection_is_unsupported() {
    let editor = laid_out();
    let mut state = editor::State::new().with_primary_selection(false);

    let update = feed(
        &mut state,
        &editor,
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)),
        START,
    );

    assert!(update.is_none());
    assert!(!state.is_focused());
}

#[test]
fn state_selection_is_not_written_where_the_primary_selection_is_unsupported() {
    let mut editor = laid_out();
    let mut state = editor::State::new().with_primary_selection(false);
    let to = Point::new(300.0, 10.0);

    let pressed = feed(
        &mut state,
        &editor,
        left(mouse::Event::ButtonPressed),
        START,
    );
    let _ = apply(&mut editor, pressed);

    let moved = feed(
        &mut state,
        &editor,
        Event::Mouse(mouse::Event::CursorMoved { position: to }),
        to,
    );
    let _ = apply(&mut editor, moved);

    let released = feed(&mut state, &editor, left(mouse::Event::ButtonReleased), to);

    assert!(matches!(released, Some(Update::Release)));
}
