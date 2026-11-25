//! A window that draws its own title bar. On Windows 11 it keeps the native
//! frame: resize borders, shadow, rounded corners and, through a caption
//! region, native dragging, Aero Snap, double-click to maximize and the system
//! menu. Its maximize button is a maximize region, so hovering it shows Snap
//! Layouts. On Linux and macOS the window has no decorations, and the caption
//! region drags it, toggles maximization on a double click and opens the
//! window menu on a right click. On Linux its edges resize it.
//!
//! The window events it receives are printed to stdout, so a test script can
//! follow them.
use iced::widget::{button, column, container, pick_list, row, space, text, title_bar_region};
use iced::window::{self, TitleBarRegion};
use iced::{Center, Fill, Subscription, Task, Widget};

pub fn main() -> iced::Result {
    #[allow(unused_mut)]
    let mut settings = window::Settings {
        size: iced::Size::new(900.0, 600.0),
        ..Default::default()
    };

    // Windows 11 keeps the native frame around the app-drawn title bar; the
    // other platforms draw the whole window.
    #[cfg(target_os = "windows")]
    {
        settings.platform_specific.extend_content_into_title_bar = true;
    }
    #[cfg(not(target_os = "windows"))]
    {
        settings.decorations = false;
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
            // Hovering the maximize button shows Snap Layouts on Windows 11.
            title_bar_region(
                TitleBarRegion::Maximize,
                button(text("☐").size(13)).on_press(Message::ToggleMaximize),
            ),
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
        window::events().map(|(_id, event)| Message::Window(event))
    }
}
