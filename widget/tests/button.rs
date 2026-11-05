//! Tests for keyboard focus on the `button` widget.
use iced_test::{Simulator, simulator::press_key};
use iced_widget::core::keyboard::{self, key::Named};
use iced_widget::core::widget::Operation;
use iced_widget::core::widget::operation::{self, focusable};
use iced_widget::core::{Event, Point, mouse};
use iced_widget::{Theme, button, column, text, text_input};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Message {
    First,
    Disabled,
    Last,
    Typed,
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

/// Enter and Space with a modifier held are left to the application.
#[test]
fn modified_activation_keys_are_not_captured() {
    let mut simulator = dialog();
    next(&mut simulator);

    let Event::Keyboard(keyboard::Event::KeyPressed {
        key,
        modified_key,
        physical_key,
        location,
        repeat,
        text,
        ..
    }) = key(Named::Enter)
    else {
        unreachable!()
    };

    let statuses = simulator.simulate([Event::Keyboard(keyboard::Event::KeyPressed {
        key,
        modified_key,
        physical_key,
        location,
        modifiers: keyboard::Modifiers::CTRL,
        repeat,
        text,
    })]);
    assert_eq!(statuses, [iced_widget::core::event::Status::Ignored]);
    assert_eq!(simulator.drain().count(), 0);
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

/// A press captured by an earlier sibling still unfocuses the button.
#[test]
fn press_captured_by_a_sibling_unfocuses() {
    let mut simulator = Simulator::new(column![
        text_input("", "").on_input(|_| Message::Typed),
        button(text("only")).on_press(Message::First),
    ]);

    run(&mut simulator, focusable::focus_next::<()>());
    run(&mut simulator, focusable::focus_next::<()>());
    assert_eq!(focused(&mut simulator).total, 2);
    // Focus the button (second focusable), then click the text input.
    assert_eq!(focused(&mut simulator).focused, Some(1));

    simulator.point_at(Point::new(5.0, 5.0));
    let statuses = simulator.simulate([Event::Mouse(mouse::Event::ButtonPressed(
        mouse::Button::Left,
    ))]);
    assert_eq!(statuses, [iced_widget::core::event::Status::Captured]);

    assert_ne!(focused(&mut simulator).focused, Some(1));
    let _ = simulator.simulate([key(Named::Enter)]);
    assert!(!simulator.drain().any(|message| message == Message::First));
}

/// A click does not arm Enter and Space for the button.
#[test]
fn click_does_not_focus_the_button() {
    let mut simulator = dialog();

    simulator.point_at(Point::new(5.0, 5.0));
    let _ = simulator.simulate([
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ]);
    assert_eq!(simulator.drain().collect::<Vec<_>>(), [Message::First]);
    assert_eq!(focused(&mut simulator).focused, None);

    let statuses = simulator.simulate([key(Named::Space), key(Named::Enter)]);
    assert_eq!(statuses, [iced_widget::core::event::Status::Ignored; 2]);
    assert_eq!(simulator.drain().count(), 0);
}

/// The ring is composited above content that is not a quad, such as an
/// image that fills the button.
#[test]
fn focus_ring_is_drawn_over_an_image() {
    use iced_widget::core::{Color, ContentFit, Length};
    use iced_widget::image;

    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    let theme = Theme::Light;

    let render = |focus: bool| {
        let handle = image::Handle::from_rgba(1, 1, vec![255, 255, 255, 255]);
        let mut simulator = Simulator::new(
            button(
                image(handle)
                    .width(Length::Fixed(40.0))
                    .height(Length::Fixed(20.0))
                    .content_fit(ContentFit::Fill),
            )
            .padding(0)
            .style(|_, _| button::Style {
                background: Some(Color::BLACK.into()),
                text_color: Color::BLACK,
                focus_ring: Some(
                    iced_widget::core::border::rounded(0)
                        .color(Color::BLACK)
                        .width(2),
                ),
                ..button::Style::default()
            })
            .on_press(Message::First),
        );

        if focus {
            next(&mut simulator);
        }

        let name = if focus {
            "image_ring_on"
        } else {
            "image_ring_off"
        };
        let path = directory.join(format!("{name}.png"));
        let tiny = directory.join(format!("{name}-tiny-skia.png"));
        let _ = std::fs::remove_file(&tiny);

        let _ = simulator
            .snapshot(&theme)
            .unwrap()
            .matches_image(&path)
            .unwrap();

        let mut reader =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&tiny).unwrap()))
                .read_info()
                .unwrap();
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut bytes).unwrap();
        let _ = std::fs::remove_file(&tiny);

        // Middle of the top edge at 2x.
        let i = (1 * info.width as usize + 30) * 4;
        i32::from(bytes[i]) + i32::from(bytes[i + 1]) + i32::from(bytes[i + 2])
    };

    assert!(render(false) > 600, "the image covers the button edge");
    assert!(render(true) < 100, "the ring is drawn above the image");
}

/// The focus ring is drawn inside the button, with a color that contrasts
/// with the button background, and only after keyboard focus.
#[test]
fn focus_ring_is_drawn_inside_the_button() {
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    let theme = Theme::Light;

    let render = |simulator: &mut Simulator<'_, Message>, name: &str| {
        let path = directory.join(format!("{name}.png"));
        let _ = std::fs::remove_file(directory.join(format!("{name}-tiny-skia.png")));

        let _ = simulator
            .snapshot(&theme)
            .unwrap()
            .matches_image(&path)
            .unwrap();

        let file = std::fs::File::open(directory.join(format!("{name}-tiny-skia.png"))).unwrap();
        let mut reader = png::Decoder::new(std::io::BufReader::new(file))
            .read_info()
            .unwrap();
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut bytes).unwrap();

        (info.width as usize, bytes)
    };

    // Snapshots are rendered at 2x: the 2px ring covers the outer 4 pixels.
    let pixel = |(width, bytes): &(usize, Vec<u8>), x: usize, y: usize| {
        let i = (y * width + x) * 4;
        [bytes[i], bytes[i + 1], bytes[i + 2]]
    };
    let brightness = |p: [u8; 3]| p.iter().map(|c| i32::from(*c)).sum::<i32>();

    let mut plain = dialog();
    let plain = render(&mut plain, "button_ring_plain");

    let mut keyboard = dialog();
    next(&mut keyboard);
    let keyboard = render(&mut keyboard, "button_ring_keyboard");

    let mut pointer = dialog();
    pointer.point_at(Point::new(5.0, 5.0));
    let _ = pointer.simulate([
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ]);
    pointer.point_at(Point::new(-100.0, -100.0));
    let pointer = render(&mut pointer, "button_ring_pointer");

    // The middle of the top edge of the first button.
    let x = 30;
    let fill = pixel(&plain, x, 1);

    assert_ne!(fill, [255, 255, 255], "the button is filled");
    assert_eq!(pixel(&pointer, x, 1), fill, "no ring after a click");
    assert!(
        (brightness(pixel(&keyboard, x, 1)) - brightness(fill)).abs() > 150,
        "the ring contrasts with the button fill: {:?} vs {fill:?}",
        pixel(&keyboard, x, 1)
    );
    assert_eq!(
        pixel(&keyboard, x, 8),
        pixel(&plain, x, 8),
        "the ring does not reach past its width"
    );

    for name in ["plain", "keyboard", "pointer"] {
        let _ = std::fs::remove_file(directory.join(format!("button_ring_{name}-tiny-skia.png")));
    }
}
