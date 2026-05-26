use iced::widget::{center, text};
use iced::{Element, Font, Theme};

// FiraSans bundled here so Android (which has no system fonts configured for
// iced) gets a working SansSerif font.  On desktop iced finds system fonts
// automatically, but on Android fontdb can't read /system/fonts reliably.
const FIRA_SANS: &[u8] = include_bytes!("fonts/FiraSans-Regular.ttf");

pub fn run() -> iced::Result {
    iced::application("Hello World", App::update, App::view)
        .theme(|_| Theme::Dark)
        // Load FiraSans so text renders correctly on Android
        .font(FIRA_SANS)
        .default_font(Font::with_name("Fira Sans"))
        .run()
}

#[derive(Default)]
struct App;

impl App {
    fn update(&mut self, _message: ()) {}

    fn view(&self) -> Element<'_, ()> {
        center(
            text("Hello, World!")
                .size(48)
                .color([1.0, 1.0, 1.0]),
        )
        .into()
    }
}

// ── Android entry point ──────────────────────────────────────────────────────
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(android_app: iced_winit::android::AndroidApp) {
    use android_logger::Config;
    use log::LevelFilter;

    android_logger::init_once(
        Config::default().with_max_level(LevelFilter::Debug),
    );

    log::info!("android_main: setting AndroidApp");
    iced_winit::android::set_android_app(android_app);

    log::info!("android_main: starting iced");
    run().expect("iced app failed");
}
