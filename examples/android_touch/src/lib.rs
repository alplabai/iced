//! A small Android test app for issue #131: taps, drag, scroll, the soft
//! keyboard, rotation and suspend/resume, with either renderer.
//!
//! Pick the renderer before launch with a system property, for example
//! `adb shell setprop debug.iced.backend vello-cpu` (or `wgpu`), and the wgpu
//! API with `adb shell setprop debug.iced.wgpu_backend gl` (or `vulkan`).
use iced::widget::{button, column, row, scrollable, slider, text, text_input};
use iced::{Center, Event, Fill, Subscription, Widget, event, window};

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: iced::android::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    // Rust panics go to stderr, which logcat drops; route them through `log`.
    std::panic::set_hook(Box::new(|info| log::error!("panic: {info}")));

    // Android apps get no environment, so map system properties onto the
    // variables iced and wgpu already read.
    for (property, variable) in [
        ("debug.iced.backend", "ICED_BACKEND"),
        ("debug.iced.wgpu_backend", "WGPU_BACKEND"),
    ] {
        if let Some(value) = system_property(property) {
            log::info!("{variable}={value}");
            // SAFETY: single-threaded here, before iced starts any thread.
            unsafe { std::env::set_var(variable, value) };
        }
    }

    iced::android::set_app(app);

    if let Err(error) = run() {
        log::error!("iced exited with an error: {error}");
    }
}

#[cfg(target_os = "android")]
fn system_property(name: &str) -> Option<String> {
    let output = std::process::Command::new("getprop").arg(name).output().ok()?;
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();

    (!value.is_empty()).then_some(value)
}

pub fn run() -> iced::Result {
    iced::application(Touch::default, Touch::update, Touch::view)
        .subscription(Touch::subscription)
        .run()
}

#[derive(Default)]
struct Touch {
    count: i64,
    input: String,
    submitted: String,
    level: f32,
    size: Option<iced::Size>,
    last_event: String,
    resumed: u32,
}

#[derive(Debug, Clone)]
enum Message {
    Increment,
    Decrement,
    Input(String),
    Submit,
    Level(f32),
    Event(Event),
}

impl Touch {
    fn update(&mut self, message: Message) {
        match message {
            Message::Increment => self.count += 1,
            Message::Decrement => self.count -= 1,
            Message::Input(input) => self.input = input,
            Message::Submit => self.submitted = std::mem::take(&mut self.input),
            Message::Level(level) => self.level = level,
            Message::Event(event) => {
                match &event {
                    Event::Window(window::Event::Resized(size)) => self.size = Some(*size),
                    Event::Window(window::Event::Opened { size, .. }) => {
                        self.resumed += 1;
                        self.size = Some(*size);
                    }
                    _ => {}
                }

                log::info!("event: {event:?}");
                self.last_event = format!("{event:?}").chars().take(80).collect();
            }
        }
    }

    fn view(&self) -> impl Widget<Message> {
        let size = self
            .size
            .map(|size| format!("{:.0} x {:.0}", size.width, size.height))
            .unwrap_or_default();

        let rows = (1..=60).map(|i| text!("Row {i}").size(22));

        column![
            text("iced on Android").size(28),
            text!("window {size}, opened {} times", self.resumed),
            text!("last event: {}", self.last_event).size(12),
            row![
                button(text("-").size(28)).on_press(Message::Decrement),
                text(self.count).size(40),
                button(text("+").size(28)).on_press(Message::Increment),
            ]
            .spacing(20)
            .align_y(Center),
            text_input("Type here", &self.input)
                .on_input(Message::Input)
                .on_submit(Message::Submit)
                .size(22),
            text!("submitted: {}", self.submitted),
            slider(0.0..=100.0, self.level, Message::Level),
            text!("slider: {:.0}", self.level),
            scrollable(column(rows).spacing(8).width(Fill))
                .height(Fill),
        ]
        .spacing(12)
        .padding(24)
    }

    fn subscription(&self) -> Subscription<Message> {
        event::listen_with(|event, _status, _window| Some(Message::Event(event)))
    }
}
