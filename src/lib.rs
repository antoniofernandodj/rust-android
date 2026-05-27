use iced::widget::{center, column, text};
use iced::{Alignment, Element, Font, Subscription, Theme};
use std::time::Duration;
use sysinfo::System;

// FiraSans bundled here so Android (which has no system fonts configured for
// iced) gets a working SansSerif font.  On desktop iced finds system fonts
// automatically, but on Android fontdb can't read /system/fonts reliably.
const FIRA_SANS: &[u8] = include_bytes!("fonts/FiraSans-Regular.ttf");

pub fn run() -> iced::Result {
    iced::application("Memory Usage", App::update, App::view)
        .theme(|_| Theme::Dark)
        .subscription(App::subscription)
        // Load FiraSans so text renders correctly on Android
        .font(FIRA_SANS)
        .default_font(Font::with_name("Fira Sans"))
        .run()
}

struct App {
    sys: System,
    pid: sysinfo::Pid,
    memory_usage: u64,
    virtual_memory: u64,
}

impl Default for App {
    fn default() -> Self {
        let mut sys = System::new_all();
        let pid = sysinfo::get_current_pid().expect("Failed to get current PID");
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]));
        let (memory_usage, virtual_memory) = sys.process(pid)
            .map(|p| (p.memory(), p.virtual_memory()))
            .unwrap_or((0, 0));
        Self {
            sys,
            pid,
            memory_usage,
            virtual_memory,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Tick,
}

impl App {
    fn update(&mut self, message: Message) {
        match message {
            Message::Tick => {
                self.sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[self.pid]));
                if let Some(process) = self.sys.process(self.pid) {
                    self.memory_usage = process.memory();
                    self.virtual_memory = process.virtual_memory();
                }
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        center(
            column![
                text("Memory Usage Monitoring")
                    .size(32)
                    .color([1.0, 1.0, 1.0]),
                column![
                    text(format!("RSS: {:.2} MB", self.memory_usage as f64 / 1024.0 / 1024.0))
                        .size(48)
                        .color([0.3, 0.8, 0.3]),
                    text(format!("Virtual: {:.2} MB", self.virtual_memory as f64 / 1024.0 / 1024.0))
                        .size(24)
                        .color([0.5, 0.5, 0.8]),
                ]
                .spacing(10)
                .align_x(Alignment::Center),
                text("Updated every 3 seconds")
                    .size(16)
                    .color([0.6, 0.6, 0.6]),
            ]
            .spacing(30)
            .align_x(Alignment::Center),
        )
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::time::every(Duration::from_secs(3)).map(|_| Message::Tick)
    }
}

// ── Android entry point ──────────────────────────────────────────────────────
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(android_app: iced_winit::android::AndroidApp) {
    use android_logger::Config;
    use log::LevelFilter;

    android_logger::init_once(Config::default().with_max_level(LevelFilter::Debug));

    log::info!("android_main: setting AndroidApp");
    iced_winit::android::set_android_app(android_app);

    log::info!("android_main: starting iced");
    run().expect("iced app failed");
}
