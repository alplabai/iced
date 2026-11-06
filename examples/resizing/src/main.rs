//! Reproducer for `window::resize` relayout (upstream iced-rs/iced#3156).
//!
//! On Wayland the buttons used to stay where they were after a button
//! resized the window; they should re-center at once.
use iced::widget::{button, center, column};
use iced::{Size, Task, Widget, window};

pub fn main() -> iced::Result {
    iced::application(|| (), update, view).run()
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Resize(Size),
}

fn update(_state: &mut (), message: Message) -> Task<Message> {
    match message {
        Message::Resize(size) => window::latest().and_then(move |id| window::resize(id, size)),
    }
}

fn view(_state: &()) -> impl Widget<Message> {
    center(
        column![
            button("Resize to 300x300").on_press(Message::Resize(Size::new(300.0, 300.0))),
            button("Resize to 500x400").on_press(Message::Resize(Size::new(500.0, 400.0))),
        ]
        .spacing(20),
    )
}
