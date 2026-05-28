#[derive(Debug, Clone, PartialEq)]
pub enum BatteryHealth {
    Good,
    Overheat,
    Dead,
    OverVoltage,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct BatteryState {
    pub level: f32,
    pub is_charging: bool,
    pub temperature_c: f32,
    pub voltage_mv: u32,
    pub health: BatteryHealth,
}

impl BatteryState {
    pub fn level_percent(&self) -> u8 {
        (self.level * 100.0) as u8
    }

    pub fn is_charging(&self) -> bool {
        self.is_charging
    }

    pub fn temperature_c(&self) -> f32 {
        self.temperature_c
    }

    pub fn voltage_mv(&self) -> u32 {
        self.voltage_mv
    }
}

pub struct BatteryManager;

impl BatteryManager {
    /// Retorna o estado atual da bateria.
    ///
    /// **Android**: lê via `Intent.ACTION_BATTERY_CHANGED` por JNI.
    /// **Desktop**: retorna dados simulados.
    pub fn current() -> BatteryState {
        #[cfg(target_os = "android")]
        {
            // TODO: implementar leitura real via JNI
            // Por ora retorna stub enquanto a implementação JNI não está pronta.
            stub()
        }
        #[cfg(not(target_os = "android"))]
        stub()
    }
}

fn stub() -> BatteryState {
    BatteryState {
        level: 0.85,
        is_charging: false,
        temperature_c: 28.5,
        voltage_mv: 3800,
        health: BatteryHealth::Good,
    }
}

/// Stream que emite o estado da bateria periodicamente.
///
/// Requer feature `stream` (ativa tokio + futures).
/// **Não use no crate principal do app Android** — pode conflitar com o
/// runtime que o iced/winit configura internamente.
#[cfg(feature = "stream")]
pub fn stream() -> impl futures::Stream<Item = BatteryState> {
    use std::time::Duration;
    futures::stream::unfold((), |_| async {
        tokio::time::sleep(Duration::from_secs(5)).await;
        Some((BatteryManager::current(), ()))
    })
}
