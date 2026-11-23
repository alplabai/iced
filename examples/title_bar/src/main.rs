//! A window that draws its own title bar on Windows 11 while keeping the
//! native frame: resize borders, shadow, rounded corners and, through a
//! caption region, native dragging, Aero Snap, double-click to maximize and
//! the system menu.
//!
//! The window events it receives are printed to stdout, so a test script can
//! follow them.
use iced::widget::{button, column, container, pick_list, row, space, text, title_bar_region};
use iced::window::{self, TitleBarRegion};
use iced::{Center, Fill, Subscription, Task, Widget, mouse};

pub fn main() -> iced::Result {
    #[allow(unused_mut)]
    let mut settings = window::Settings {
        size: iced::Size::new(900.0, 600.0),
        ..Default::default()
    };

    // Windows 11 only; other platforms keep their native title bar.
    #[cfg(target_os = "windows")]
    {
        settings.platform_specific.extend_content_into_title_bar = true;
    }

    iced::application(TitleBar::default, TitleBar::update, TitleBar::view)
        .title("Title bar regions")
        .subscription(TitleBar::subscription)
        .window(settings)
        .run()
}

#[derive(Default)]
struct TitleBar {
    clicks: usize,
    choice: Option<Choice>,
    last_event: String,
}

#[derive(Debug, Clone)]
enum Message {
    Clicked,
    Chose(Choice),
    Minimize,
    ToggleMaximize,
    Close,
    Window(window::Event),
    Mouse(mouse::Event, iced::event::Status),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    File,
    Edit,
    View,
}

impl Choice {
    const ALL: [Choice; 3] = [Choice::File, Choice::Edit, Choice::View];
}

impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Choice::File => "File",
            Choice::Edit => "Edit",
            Choice::View => "View",
        })
    }
}

impl TitleBar {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Clicked => {
                self.clicks += 1;
                println!("clicked {}", self.clicks);
                Task::none()
            }
            Message::Chose(choice) => {
                self.choice = Some(choice);
                println!("chose {choice}");
                Task::none()
            }
            Message::Minimize => window::latest().and_then(|id| window::minimize(id, true)),
            Message::ToggleMaximize => window::latest().and_then(window::toggle_maximize),
            Message::Close => window::latest().and_then(window::close),
            Message::Mouse(event, status) => {
                println!("mouse {event:?} {status:?}");
                Task::none()
            }
            Message::Window(event) => {
                let line = match event {
                    window::Event::Moved(position) => {
                        format!("moved {:.0},{:.0}", position.x, position.y)
                    }
                    window::Event::Resized(size) => {
                        format!("resized {:.0}x{:.0}", size.width, size.height)
                    }
                    window::Event::Focused => "focused".to_owned(),
                    window::Event::Unfocused => "unfocused".to_owned(),
                    _ => return Task::none(),
                };
                println!("{line}");
                self.last_event = line;
                Task::none()
            }
        }
    }

    fn view(&self) -> impl Widget<Message> {
        let strip = row![
            text("Title bar regions").size(14),
            button(text(format!("Clicks: {}", self.clicks)).size(13)).on_press(Message::Clicked),
            pick_list(self.choice, Choice::ALL, Choice::to_string)
                .on_select(Message::Chose)
                .placeholder("Menu")
                .text_size(13),
            // Everything else in the strip is the native caption.
            title_bar_region(TitleBarRegion::Caption, space::horizontal().height(Fill)),
            button(text("—").size(13)).on_press(Message::Minimize),
            button(text("☐").size(13)).on_press(Message::ToggleMaximize),
            button(text("✕").size(13)).on_press(Message::Close),
        ]
        .spacing(8)
        .padding([0, 8])
        .height(36)
        .align_y(Center);

        let body = container(
            column![
                text("Drag the empty part of the strip to move the window."),
                text("Double-click it to maximize, right-click it for the system menu."),
                text(format!("Last window event: {}", self.last_event)),
            ]
            .spacing(8),
        )
        .padding(16)
        .width(Fill)
        .height(Fill);

        column![strip, body]
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            window::events().map(|(_id, event)| Message::Window(event)),
            iced::event::listen_with(|event, status, _window| match event {
                iced::Event::Mouse(
                    mouse @ (mouse::Event::ButtonPressed(_)
                    | mouse::Event::ButtonReleased(_)
                    | mouse::Event::CursorEntered
                    | mouse::Event::CursorLeft),
                ) => Some(Message::Mouse(mouse, status)),
                _ => None,
            }),
        ])
    }
}
