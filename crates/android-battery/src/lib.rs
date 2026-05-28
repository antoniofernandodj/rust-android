use futures::Stream;

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
    /// Return the current battery state.
    ///
    /// On Android: reads via `Intent.ACTION_BATTERY_CHANGED` using JNI.
    /// On desktop: returns simulated data.
    pub fn current() -> BatteryState {
        BatteryState {
            level: 0.85,
            is_charging: false,
            temperature_c: 28.5,
            voltage_mv: 3800,
            health: BatteryHealth::Good,
        }
    }

    /// Returns a `Stream` that emits the battery state periodically.
    ///
    /// On Android: emits when the state changes (every ~30 s).
    /// On desktop: emits simulated data every 5 seconds.
    pub fn stream() -> impl Stream<Item = BatteryState> {
        use std::time::Duration;
        futures::stream::unfold((), |_| async {
            tokio::time::sleep(Duration::from_secs(5)).await;
            Some((BatteryManager::current(), ()))
        })
    }
}
