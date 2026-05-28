use android_battery::{BatteryManager, BatteryState};
use iced::widget::{center, column, row, text};
use iced::{Alignment, Element, Font, Subscription, Theme};
use std::time::Duration;
use sysinfo::System;

const FIRA_SANS: &[u8] = include_bytes!("fonts/FiraSans-Regular.ttf");

pub fn run() -> iced::Result {
    iced::application("rustandroid", App::update, App::view)
        .theme(|_| Theme::Dark)
        .subscription(App::subscription)
        .font(FIRA_SANS)
        .default_font(Font::with_name("Fira Sans"))
        .run()
}

struct App {
    sys: System,
    pid: sysinfo::Pid,
    memory_usage: u64,
    virtual_memory: u64,
    battery: BatteryState,
}

impl Default for App {
    fn default() -> Self {
        let mut sys = System::new_all();
        let pid = sysinfo::get_current_pid().expect("Failed to get current PID");
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]));
        let (memory_usage, virtual_memory) = sys
            .process(pid)
            .map(|p| (p.memory(), p.virtual_memory()))
            .unwrap_or((0, 0));
        Self {
            sys,
            pid,
            memory_usage,
            virtual_memory,
            battery: BatteryManager::current(),
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
                self.sys
                    .refresh_processes(sysinfo::ProcessesToUpdate::Some(&[self.pid]));
                if let Some(process) = self.sys.process(self.pid) {
                    self.memory_usage = process.memory();
                    self.virtual_memory = process.virtual_memory();
                }
                self.battery = BatteryManager::current();
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let pct = self.battery.level_percent();
        let battery_color = match pct {
            80..=100 => [0.2, 0.9, 0.3],
            40..=79  => [0.9, 0.8, 0.1],
            _        => [0.9, 0.2, 0.2],
        };
        let charging_label = if self.battery.is_charging() {
            " ⚡ carregando"
        } else {
            ""
        };

        center(
            column![
                // ── Bateria ──────────────────────────────────────────────
                text("Bateria")
                    .size(20)
                    .color([0.6, 0.6, 0.6]),
                row![
                    text(format!("{}%", pct))
                        .size(64)
                        .color(battery_color),
                    text(charging_label)
                        .size(20)
                        .color([0.4, 0.8, 1.0]),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                text(format!(
                    "{:.1} °C  •  {} mV  •  {:?}",
                    self.battery.temperature_c(),
                    self.battery.voltage_mv(),
                    self.battery.health,
                ))
                .size(16)
                .color([0.5, 0.5, 0.5]),

                // ── Separador ────────────────────────────────────────────
                text("──────────────────────")
                    .size(14)
                    .color([0.3, 0.3, 0.3]),

                // ── Memória ───────────────────────────────────────────────
                text("Memória do processo")
                    .size(20)
                    .color([0.6, 0.6, 0.6]),
                text(format!(
                    "RSS {:.1} MB  •  Virtual {:.1} MB",
                    self.memory_usage as f64 / 1_048_576.0,
                    self.virtual_memory as f64 / 1_048_576.0,
                ))
                .size(24)
                .color([0.3, 0.7, 1.0]),

                text("atualiza a cada 3 s")
                    .size(14)
                    .color([0.4, 0.4, 0.4]),
            ]
            .spacing(12)
            .align_x(Alignment::Center),
        )
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::time::every(Duration::from_secs(3)).map(|_| Message::Tick)
    }
}

// ── Android entry point ───────────────────────────────────────────────────────
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(android_app: iced_winit::android::AndroidApp) {
    use android_logger::Config;
    use log::LevelFilter;

    android_logger::init_once(Config::default().with_max_level(LevelFilter::Debug));
    log::info!("android_main: starting");
    iced_winit::android::set_android_app(android_app);
    run().expect("iced app failed");
}
