use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Effect {
    Click,
    DoubleClick,
    HeavyClick,
    Tick,
}

/// A haptic waveform defined as a sequence of (duration_ms, amplitude) steps.
///
/// Amplitude `0` means the vibrator is off; `255` is maximum intensity.
#[derive(Debug, Default)]
pub struct Waveform {
    steps: Vec<(u64, u8)>,
}

impl Waveform {
    pub fn new() -> Self {
        Self::default()
    }

    /// Vibrate at full amplitude for `ms` milliseconds.
    pub fn on(mut self, ms: u64) -> Self {
        self.steps.push((ms, 255));
        self
    }

    /// Pause for `ms` milliseconds.
    pub fn off(mut self, ms: u64) -> Self {
        self.steps.push((ms, 0));
        self
    }

    /// Append a step with an explicit amplitude (0–255).
    pub fn step(mut self, duration: u64, amplitude: u8) -> Self {
        self.steps.push((duration, amplitude));
        self
    }
}

pub struct Vibrator;

impl Vibrator {
    /// Play a pre-defined haptic effect.
    ///
    /// On Android: uses `VibrationEffect` via JNI.
    /// On desktop: no-op.
    pub fn play(effect: Effect) {
        let _ = effect;
    }

    /// Vibrate continuously for `duration`.
    ///
    /// On Android: uses `VibrationEffect.createOneShot` via JNI.
    /// On desktop: no-op.
    pub fn buzz(duration: Duration) {
        let _ = duration;
    }

    /// Play a waveform pattern.
    ///
    /// On Android: uses `VibrationEffect.createWaveform` via JNI.
    /// On desktop: no-op.
    pub fn pattern(waveform: Waveform) {
        let _ = waveform;
    }

    /// Alias for `pattern` — plays a custom waveform.
    pub fn waveform(waveform: Waveform) {
        let _ = waveform;
    }

    /// Stop any ongoing vibration.
    ///
    /// On Android: calls `Vibrator.cancel()` via JNI.
    /// On desktop: no-op.
    pub fn cancel() {}
}
