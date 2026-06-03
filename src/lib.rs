use android_battery::BatteryManager;
use android_haptics::{Effect, Vibrator, Waveform};
use android_network::ConnectivityManager;
use android_notifications::{Channel, Notification, Priority};
use android_permissions::Permission;
#[allow(unused_imports)]
use android_sensors::{SamplingRate, Sensor, SensorEvent};
use iced::widget::{button, center, column, container, row, scrollable, text};
use iced::{Alignment, Color, Element, Font, Length, Subscription, Theme};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const FIRA_SANS: &[u8] = include_bytes!("fonts/FiraSans-Regular.ttf");

// ── Shared sensor state (updated by background task) ─────────────────────────

#[derive(Default, Clone)]
struct SensorSnapshot {
    accel: [f32; 3],
    gyro: [f32; 3],
    light: f32,
    steps: u64,
    active: bool,
    tick: u64,
}

fn sensor_state() -> &'static Mutex<SensorSnapshot> {
    static S: OnceLock<Mutex<SensorSnapshot>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(SensorSnapshot::default()))
}

// ── Shared async results (permission requests, etc.) ─────────────────────────

fn async_log() -> &'static Mutex<Option<String>> {
    static L: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(None))
}

fn take_async_log() -> Option<String> {
    async_log().lock().unwrap().take()
}

fn set_async_log(s: impl Into<String>) {
    *async_log().lock().unwrap() = Some(s.into());
}

// ── Screens ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Battery,
    Haptics,
    Network,
    Notifications,
    Permissions,
    Sensors,
}

impl Screen {
    const ALL: &'static [Screen] = &[
        Screen::Battery,
        Screen::Haptics,
        Screen::Network,
        Screen::Notifications,
        Screen::Permissions,
        Screen::Sensors,
    ];

    fn label(self) -> &'static str {
        match self {
            Screen::Battery => "Bateria",
            Screen::Haptics => "Haptics",
            Screen::Network => "Rede",
            Screen::Notifications => "Notif",
            Screen::Permissions => "Perms",
            Screen::Sensors => "Sensores",
        }
    }
}

// ── Messages ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum Message {
    GoTo(Screen),
    Tick,

    // Battery
    RefreshBattery,

    // Haptics
    HapticEffect(Effect),
    HapticBuzz,
    HapticPattern,
    HapticCancel,

    // Network
    RefreshNetwork,

    // Notifications
    RegisterChannel,
    ShowNotification,
    ShowProgress,
    CancelNotification,

    // Permissions
    CheckAll,
    RequestCamera,
    RequestLocation,
    RequestBodySensors,

    // Sensors
    StartSensor,
    StopSensor,
    SensorUpdate,
}

// ── App state ─────────────────────────────────────────────────────────────────

struct App {
    screen: Screen,

    // Battery
    battery: android_battery::BatteryState,

    // Haptics
    haptic_log: Vec<String>,

    // Network
    network: android_network::NetworkState,

    // Notifications
    notif_channel_ok: bool,
    notif_count: u32,
    notif_log: Vec<String>,

    // Permissions
    camera_ok: bool,
    location_ok: bool,
    body_sensors_ok: bool,
    perm_log: Vec<String>,

    // Sensors
    sensor_active: bool,
    sensor_snap: SensorSnapshot,
}

impl Default for App {
    fn default() -> Self {
        Self {
            screen: Screen::Battery,
            battery: BatteryManager::current(),
            haptic_log: Vec::new(),
            network: ConnectivityManager::current(),
            notif_channel_ok: false,
            notif_count: 0,
            notif_log: Vec::new(),
            camera_ok: android_permissions::check(&Permission::Camera),
            location_ok: android_permissions::check(&Permission::AccessFineLocation),
            body_sensors_ok: android_permissions::check(&Permission::BodySensors),
            perm_log: Vec::new(),
            sensor_active: false,
            sensor_snap: SensorSnapshot::default(),
        }
    }
}

// ── Update ────────────────────────────────────────────────────────────────────

impl App {
    fn update(&mut self, msg: Message) {
        // Absorb any pending async log before processing the message
        if let Some(log) = take_async_log() {
            match self.screen {
                Screen::Permissions => self.perm_log.push(log),
                _ => {}
            }
        }

        match msg {
            Message::GoTo(s) => self.screen = s,

            Message::Tick | Message::SensorUpdate => {
                // Absorb async log again on every tick
                if let Some(log) = take_async_log() {
                    match self.screen {
                        Screen::Permissions => self.perm_log.push(log),
                        _ => {}
                    }
                }
                // Animate stub values on desktop
                #[cfg(not(target_os = "android"))]
                {
                    let is_active = sensor_state().lock().unwrap().active;
                    if is_active {
                        let mut s = sensor_state().lock().unwrap();
                        let t = s.tick as f32 * 0.08;
                        s.accel = [t.sin() * 2.0, t.cos() * 1.5, 9.8 + t.sin() * 0.3];
                        s.gyro = [(t * 0.5).sin() * 0.3, (t * 0.3).cos() * 0.2, 0.0];
                        s.light = 120.0 + t.sin() * 40.0;
                        s.steps += if (s.tick % 8) == 0 { 1 } else { 0 };
                        s.tick += 1;
                    }
                }
                self.sensor_snap = sensor_state().lock().unwrap().clone();
            }

            // ── Battery ──────────────────────────────────────────────────────
            Message::RefreshBattery => {
                self.battery = BatteryManager::current();
            }

            // ── Haptics ──────────────────────────────────────────────────────
            Message::HapticEffect(e) => {
                Vibrator::play(e);
                self.haptic_log.push(format!("play({:?})", e));
                trim_log(&mut self.haptic_log);
            }
            Message::HapticBuzz => {
                Vibrator::buzz(Duration::from_millis(500));
                self.haptic_log.push("buzz(500ms)".into());
                trim_log(&mut self.haptic_log);
            }
            Message::HapticPattern => {
                let w = Waveform::new().on(80).off(60).on(80).off(60).on(200);
                Vibrator::pattern(w);
                self.haptic_log.push("pattern(80·80·200ms)".into());
                trim_log(&mut self.haptic_log);
            }
            Message::HapticCancel => {
                Vibrator::cancel();
                self.haptic_log.push("cancel()".into());
                trim_log(&mut self.haptic_log);
            }

            // ── Network ──────────────────────────────────────────────────────
            Message::RefreshNetwork => {
                self.network = ConnectivityManager::current();
            }

            // ── Notifications ────────────────────────────────────────────────
            Message::RegisterChannel => {
                Channel::new("demo", "Demo Notifications")
                    .importance(Priority::Default)
                    .register();
                self.notif_channel_ok = true;
                self.notif_log.push("Canal 'demo' registrado".into());
                trim_log(&mut self.notif_log);
            }
            Message::ShowNotification => {
                self.notif_count += 1;
                Notification::new("demo")
                    .title("rustandroid")
                    .body(format!("Notificação #{}", self.notif_count))
                    .show(self.notif_count);
                self.notif_log.push(format!("show(id={})", self.notif_count));
                trim_log(&mut self.notif_log);
            }
            Message::ShowProgress => {
                self.notif_count += 1;
                Notification::new("demo")
                    .title("Download")
                    .body("Em progresso…")
                    .progress(65, 100)
                    .ongoing(true)
                    .show(self.notif_count);
                self.notif_log.push(format!("progress(id={}, 65/100)", self.notif_count));
                trim_log(&mut self.notif_log);
            }
            Message::CancelNotification => {
                Notification::cancel(self.notif_count);
                self.notif_log.push(format!("cancel(id={})", self.notif_count));
                trim_log(&mut self.notif_log);
            }

            // ── Permissions ──────────────────────────────────────────────────
            Message::CheckAll => {
                self.camera_ok = android_permissions::check(&Permission::Camera);
                self.location_ok =
                    android_permissions::check(&Permission::AccessFineLocation);
                self.body_sensors_ok =
                    android_permissions::check(&Permission::BodySensors);
                self.perm_log.push("check() concluído".into());
                trim_log(&mut self.perm_log);
            }
            Message::RequestCamera => {
                tokio::spawn(async {
                    match android_permissions::request(&[Permission::Camera]).await {
                        Ok(()) => set_async_log("Camera: concedida ✓"),
                        Err(e) => set_async_log(format!("Camera: negada ({:?})", e)),
                    }
                });
                self.perm_log.push("request(Camera) disparado…".into());
                trim_log(&mut self.perm_log);
            }
            Message::RequestLocation => {
                tokio::spawn(async {
                    match android_permissions::request(&[
                        Permission::AccessFineLocation,
                        Permission::AccessCoarseLocation,
                    ])
                    .await
                    {
                        Ok(()) => set_async_log("Localização: concedida ✓"),
                        Err(e) => set_async_log(format!("Localização: negada ({:?})", e)),
                    }
                });
                self.perm_log.push("request(Location) disparado…".into());
                trim_log(&mut self.perm_log);
            }
            Message::RequestBodySensors => {
                tokio::spawn(async {
                    match android_permissions::request(&[Permission::BodySensors]).await {
                        Ok(()) => set_async_log("BodySensors: concedida ✓"),
                        Err(e) => set_async_log(format!("BodySensors: negada ({:?})", e)),
                    }
                });
                self.perm_log.push("request(BodySensors) disparado…".into());
                trim_log(&mut self.perm_log);
            }

            // ── Sensors ──────────────────────────────────────────────────────
            Message::StartSensor => {
                sensor_state().lock().unwrap().active = true;
                self.sensor_active = true;

                // On Android: spawn real sensor tasks
                #[cfg(target_os = "android")]
                spawn_sensor_tasks();
            }
            Message::StopSensor => {
                sensor_state().lock().unwrap().active = false;
                self.sensor_active = false;
            }
        }
    }

    // ── Subscription ─────────────────────────────────────────────────────────

    fn subscription(&self) -> Subscription<Message> {
        let tick = iced::time::every(Duration::from_secs(3)).map(|_| Message::Tick);
        if self.sensor_active {
            let sensor_tick =
                iced::time::every(Duration::from_millis(150)).map(|_| Message::SensorUpdate);
            Subscription::batch([tick, sensor_tick])
        } else {
            tick
        }
    }

    // ── View ─────────────────────────────────────────────────────────────────

    fn view(&self) -> Element<'_, Message> {
        let content: Element<'_, Message> = match self.screen {
            Screen::Battery => self.view_battery(),
            Screen::Haptics => self.view_haptics(),
            Screen::Network => self.view_network(),
            Screen::Notifications => self.view_notifications(),
            Screen::Permissions => self.view_permissions(),
            Screen::Sensors => self.view_sensors(),
        };

        let nav = row(Screen::ALL.iter().map(|&s| {
            let active = s == self.screen;
            let lbl = text(s.label()).size(13);
            let btn = if active {
                button(lbl)
                    .style(button::primary)
                    .on_press(Message::GoTo(s))
            } else {
                button(lbl)
                    .style(button::secondary)
                    .on_press(Message::GoTo(s))
            };
            btn.into()
        }))
        .spacing(4)
        .padding(8);

        column![
            container(nav)
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(iced::Background::Color(Color::from_rgb(0.1, 0.1, 0.12))),
                    ..Default::default()
                }),
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(16),
        ]
        .into()
    }

    // ── Battery screen ────────────────────────────────────────────────────────

    fn view_battery(&self) -> Element<'_, Message> {
        let pct = self.battery.level_percent();
        let batt_color = match pct {
            80..=100 => Color::from_rgb(0.2, 0.9, 0.3),
            40..=79 => Color::from_rgb(0.9, 0.8, 0.1),
            _ => Color::from_rgb(0.9, 0.2, 0.2),
        };

        center(
            column![
                title("Bateria"),
                row![
                    text(format!("{}%", pct)).size(72).color(batt_color),
                    text(if self.battery.is_charging() { " ⚡" } else { "" })
                        .size(36)
                        .color(Color::from_rgb(0.4, 0.8, 1.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                info(format!(
                    "{:.1} °C  •  {} mV  •  {:?}",
                    self.battery.temperature_c(),
                    self.battery.voltage_mv(),
                    self.battery.health,
                )),
                btn("Atualizar", Message::RefreshBattery),
            ]
            .spacing(16)
            .align_x(Alignment::Center),
        )
        .into()
    }

    // ── Haptics screen ────────────────────────────────────────────────────────

    fn view_haptics(&self) -> Element<'_, Message> {
        let log_view = log_column(&self.haptic_log);
        column![
            title("Haptics / Vibração"),
            text("Efeitos pré-definidos (API 29+):").size(14).color(DIM),
            row![
                btn("Click", Message::HapticEffect(Effect::Click)),
                btn("DoubleClick", Message::HapticEffect(Effect::DoubleClick)),
                btn("HeavyClick", Message::HapticEffect(Effect::HeavyClick)),
                btn("Tick", Message::HapticEffect(Effect::Tick)),
            ]
            .spacing(8)
            .wrap(),
            sep(),
            text("Outros:").size(14).color(DIM),
            row![
                btn("Buzz 500ms", Message::HapticBuzz),
                btn("Padrão SOS", Message::HapticPattern),
                btn_danger("Cancelar", Message::HapticCancel),
            ]
            .spacing(8)
            .wrap(),
            sep(),
            text("Log:").size(14).color(DIM),
            log_view,
        ]
        .spacing(12)
        .into()
    }

    // ── Network screen ────────────────────────────────────────────────────────

    fn view_network(&self) -> Element<'_, Message> {
        let n = &self.network;
        let status_color = if n.is_connected() {
            Color::from_rgb(0.2, 0.9, 0.3)
        } else {
            Color::from_rgb(0.9, 0.2, 0.2)
        };

        center(
            column![
                title("Rede"),
                text(if n.is_connected() {
                    "● Conectado"
                } else {
                    "○ Sem conexão"
                })
                .size(28)
                .color(status_color),
                info(format!("Tipo: {:?}", n.network_type())),
                info(format!(
                    "SSID: {}",
                    n.ssid.as_deref().unwrap_or("—")
                )),
                info(format!(
                    "Sinal: {} dBm",
                    n.signal_strength
                        .map(|s| s.to_string())
                        .unwrap_or("—".into())
                )),
                btn("Atualizar", Message::RefreshNetwork),
            ]
            .spacing(16)
            .align_x(Alignment::Center),
        )
        .into()
    }

    // ── Notifications screen ──────────────────────────────────────────────────

    fn view_notifications(&self) -> Element<'_, Message> {
        let log_view = log_column(&self.notif_log);
        let channel_status = if self.notif_channel_ok {
            text("Canal: registrado ✓").size(14).color(Color::from_rgb(0.2, 0.9, 0.3))
        } else {
            text("Canal: não registrado").size(14).color(DIM)
        };

        column![
            title("Notificações"),
            text("1. Registre o canal antes de exibir (Android 8+):")
                .size(14)
                .color(DIM),
            btn("Registrar canal 'demo'", Message::RegisterChannel),
            channel_status,
            sep(),
            text("2. Exibir notificações:").size(14).color(DIM),
            row![
                btn("Simples", Message::ShowNotification),
                btn("Com progresso", Message::ShowProgress),
                btn_danger("Cancelar última", Message::CancelNotification),
            ]
            .spacing(8)
            .wrap(),
            info(format!("Última ID: {}", self.notif_count)),
            sep(),
            text("Log:").size(14).color(DIM),
            log_view,
        ]
        .spacing(12)
        .into()
    }

    // ── Permissions screen ────────────────────────────────────────────────────

    fn view_permissions(&self) -> Element<'_, Message> {
        let log_view = log_column(&self.perm_log);

        column![
            title("Permissões"),
            text("Estado atual (check()):").size(14).color(DIM),
            perm_row("CAMERA", self.camera_ok),
            perm_row("ACCESS_FINE_LOCATION", self.location_ok),
            perm_row("BODY_SENSORS", self.body_sensors_ok),
            btn("Checar todas", Message::CheckAll),
            sep(),
            text("Solicitar ao usuário (requer wiring Kotlin):").size(14).color(DIM),
            row![
                btn("Camera", Message::RequestCamera),
                btn("Localização", Message::RequestLocation),
                btn("Sensores", Message::RequestBodySensors),
            ]
            .spacing(8)
            .wrap(),
            sep(),
            text("Log:").size(14).color(DIM),
            log_view,
        ]
        .spacing(12)
        .into()
    }

    // ── Sensors screen ────────────────────────────────────────────────────────

    fn view_sensors(&self) -> Element<'_, Message> {
        let s = &self.sensor_snap;
        let active = self.sensor_active;

        let status_color = if active {
            Color::from_rgb(0.2, 0.9, 0.3)
        } else {
            Color::from_rgb(0.5, 0.5, 0.5)
        };

        column![
            title("Sensores"),
            row![
                btn("▶ Iniciar", Message::StartSensor),
                btn_danger("■ Parar", Message::StopSensor),
            ]
            .spacing(8),
            text(if active { "● Streaming ativo" } else { "○ Inativo" })
                .size(16)
                .color(status_color),
            sep(),
            text("Acelerômetro (m/s²)").size(14).color(DIM),
            info(format!(
                "X {:+.3}  Y {:+.3}  Z {:+.3}",
                s.accel[0], s.accel[1], s.accel[2]
            )),
            text("Giroscópio (rad/s)").size(14).color(DIM),
            info(format!(
                "X {:+.3}  Y {:+.3}  Z {:+.3}",
                s.gyro[0], s.gyro[1], s.gyro[2]
            )),
            text("Luz (lux)").size(14).color(DIM),
            info(format!("{:.1}", s.light)),
            text("Contador de passos").size(14).color(DIM),
            info(format!("{} passos", s.steps)),
            if !cfg!(target_os = "android") {
                text("(valores simulados no desktop)").size(12).color(DIM)
            } else {
                text("(leitura via ASensorManager NDK)").size(12).color(DIM)
            },
        ]
        .spacing(12)
        .into()
    }
}

// ── Android sensor tasks ──────────────────────────────────────────────────────

#[cfg(target_os = "android")]
fn spawn_sensor_tasks() {
    use futures::StreamExt as _;

    tokio::spawn(async {
        let stream = Sensor::Accelerometer.stream(SamplingRate::Ui);
        futures::pin_mut!(stream);
        while let Some(event) = stream.next().await {
            let mut s = sensor_state().lock().unwrap();
            if !s.active {
                break;
            }
            if let SensorEvent::Accelerometer(v) = event {
                s.accel = [v.x, v.y, v.z];
                s.tick += 1;
            }
        }
    });

    tokio::spawn(async {
        let stream = Sensor::Gyroscope.stream(SamplingRate::Ui);
        futures::pin_mut!(stream);
        while let Some(event) = stream.next().await {
            let mut s = sensor_state().lock().unwrap();
            if !s.active {
                break;
            }
            if let SensorEvent::Gyroscope(v) = event {
                s.gyro = [v.x, v.y, v.z];
            }
        }
    });

    tokio::spawn(async {
        let stream = Sensor::Light.stream(SamplingRate::Normal);
        futures::pin_mut!(stream);
        while let Some(event) = stream.next().await {
            let mut s = sensor_state().lock().unwrap();
            if !s.active {
                break;
            }
            if let SensorEvent::Light(l) = event {
                s.light = l;
            }
        }
    });

    tokio::spawn(async {
        let stream = Sensor::StepCounter.stream(SamplingRate::Normal);
        futures::pin_mut!(stream);
        while let Some(event) = stream.next().await {
            let mut s = sensor_state().lock().unwrap();
            if !s.active {
                break;
            }
            if let SensorEvent::StepCounter(n) = event {
                s.steps = n;
            }
        }
    });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const DIM: Color = Color {
    r: 0.55,
    g: 0.55,
    b: 0.55,
    a: 1.0,
};

fn title(s: &str) -> iced::widget::Text<'_> {
    text(s).size(24).color(Color::from_rgb(0.9, 0.9, 0.9))
}

fn info(s: impl Into<String>) -> iced::widget::Text<'static> {
    text(s.into()).size(18).color(Color::from_rgb(0.3, 0.7, 1.0))
}

fn sep<'a>() -> iced::widget::Text<'a> {
    text("─────────────────────────")
        .size(12)
        .color(Color::from_rgb(0.25, 0.25, 0.25))
}

fn btn(label: &str, msg: Message) -> iced::widget::Button<'_, Message> {
    button(text(label).size(14))
        .on_press(msg)
        .style(button::primary)
        .padding([6, 14])
}

fn btn_danger(label: &str, msg: Message) -> iced::widget::Button<'_, Message> {
    button(text(label).size(14))
        .on_press(msg)
        .style(button::danger)
        .padding([6, 14])
}

fn perm_row<'a>(name: &'a str, granted: bool) -> Element<'a, Message> {
    let dot = if granted { "✓" } else { "✗" };
    let color = if granted {
        Color::from_rgb(0.2, 0.9, 0.3)
    } else {
        Color::from_rgb(0.9, 0.3, 0.3)
    };
    row![
        text(dot).size(16).color(color),
        text(name).size(14).color(Color::from_rgb(0.8, 0.8, 0.8)),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn log_column(entries: &[String]) -> Element<'_, Message> {
    if entries.is_empty() {
        return text("(sem entradas)").size(13).color(DIM).into();
    }
    scrollable(
        column(entries.iter().rev().map(|e| {
            text(e.as_str())
                .size(13)
                .color(Color::from_rgb(0.7, 0.9, 0.7))
                .into()
        }))
        .spacing(2),
    )
    .height(100)
    .into()
}

fn trim_log(log: &mut Vec<String>) {
    if log.len() > 20 {
        log.drain(..log.len() - 20);
    }
}

// ── Entry points ──────────────────────────────────────────────────────────────

pub fn run() -> iced::Result {
    iced::application("rustandroid", App::update, App::view)
        .theme(|_| Theme::Dark)
        .subscription(App::subscription)
        .font(FIRA_SANS)
        .default_font(Font::with_name("Fira Sans"))
        .run()
}

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
