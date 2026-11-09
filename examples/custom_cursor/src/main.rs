//! Creates a custom mouse cursor and shows it over an area of the window.
use iced::mouse;
use iced::widget::{center, column, container, mouse_area, text};
use iced::window::{self, cursor};
use iced::{Fill, Size, Task, Vector, Widget};

/// The side of the crosshair cursor, in pixels.
const SIDE: u16 = 32;

pub fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view).run()
}

#[derive(Default)]
struct App {
    crosshair: Option<cursor::Id>,
}

#[derive(Debug, Clone)]
enum Message {
    CursorCreated(cursor::Id),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let center = SIDE / 2;

        (
            Self::default(),
            window::create_cursor(
                crosshair(),
                Size::new(SIDE, SIDE),
                Vector::new(center, center),
            )
            .map(Message::CursorCreated),
        )
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::CursorCreated(id) => self.crosshair = Some(id),
        }
    }

    fn view(&self) -> impl Widget<Message> {
        let area = container(center(text("Hover here for the crosshair")))
            .width(Fill)
            .height(Fill)
            .style(container::bordered_box);

        let area = match self.crosshair {
            Some(id) => mouse_area(area).interaction(mouse::Interaction::Custom(id)),
            None => mouse_area(area),
        };

        column![
            text(match self.crosshair {
                Some(id) => format!("Custom cursor created: {id}"),
                None => "Creating the custom cursor…".to_owned(),
            }),
            area,
        ]
        .spacing(10)
        .padding(20)
    }
}

/// RGBA pixels of a black crosshair with a white outline, row by row.
fn crosshair() -> Vec<u8> {
    let side = usize::from(SIDE);
    let center = side / 2;

    (0..side * side)
        .flat_map(|i| {
            let (x, y) = (i % side, i / side);
            let distance = x.abs_diff(center).min(y.abs_diff(center));

            match distance {
                0 => [0, 0, 0, 255],
                1 => [255, 255, 255, 255],
                _ => [0, 0, 0, 0],
            }
        })
        .collect()
}
