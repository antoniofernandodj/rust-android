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
    pub steps: Vec<(u64, u8)>,
}

impl Waveform {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on(mut self, ms: u64) -> Self {
        self.steps.push((ms, 255));
        self
    }

    pub fn off(mut self, ms: u64) -> Self {
        self.steps.push((ms, 0));
        self
    }

    pub fn step(mut self, duration: u64, amplitude: u8) -> Self {
        self.steps.push((duration, amplitude));
        self
    }
}

pub struct Vibrator;

impl Vibrator {
    /// Play a pre-defined haptic effect.
    ///
    /// On Android: uses `VibrationEffect` via JNI (API 26+) or legacy `vibrate(long)` fallback.
    /// On desktop: no-op.
    pub fn play(effect: Effect) {
        #[cfg(target_os = "android")]
        android_impl::play(effect);
        #[cfg(not(target_os = "android"))]
        let _ = effect;
    }

    /// Vibrate continuously for `duration`.
    ///
    /// On Android: uses `VibrationEffect.createOneShot` (API 26+) or legacy fallback.
    /// On desktop: no-op.
    pub fn buzz(duration: Duration) {
        #[cfg(target_os = "android")]
        android_impl::buzz(duration);
        #[cfg(not(target_os = "android"))]
        let _ = duration;
    }

    /// Play a waveform pattern.
    ///
    /// On Android: uses `VibrationEffect.createWaveform` (API 26+) or legacy fallback.
    /// On desktop: no-op.
    pub fn pattern(waveform: Waveform) {
        #[cfg(target_os = "android")]
        android_impl::pattern(waveform);
        #[cfg(not(target_os = "android"))]
        let _ = waveform;
    }

    /// Alias for `pattern`.
    pub fn waveform(waveform: Waveform) {
        Self::pattern(waveform);
    }

    /// Stop any ongoing vibration.
    ///
    /// On Android: calls `Vibrator.cancel()` via JNI.
    /// On desktop: no-op.
    pub fn cancel() {
        #[cfg(target_os = "android")]
        android_impl::cancel();
    }
}

// ── Android JNI implementation ────────────────────────────────────────────────

#[cfg(target_os = "android")]
mod android_impl {
    use super::*;
    use jni::{
        objects::{JObject, JValue},
        JavaVM,
    };

    fn with_vibrator<F>(f: F)
    where
        F: FnOnce(&mut jni::JNIEnv<'_>, &JObject<'_>),
    {
        let ctx = ndk_context::android_context();
        let Ok(vm) = (unsafe { JavaVM::from_raw(ctx.vm().cast()) }) else { return };
        let Ok(mut env) = vm.attach_current_thread() else { return };
        let context = unsafe { JObject::from_raw(ctx.context().cast()) };

        let Ok(svc) = env.new_string("vibrator") else { return };
        let Ok(v) = env.call_method(
            &context,
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::Object(&svc)],
        ) else { return };
        let Ok(vibrator) = v.l() else { return };
        if vibrator.is_null() { return }

        f(&mut env, &vibrator);
        // Always clear any pending Java exception before the env is dropped.
        // Dropping AttachGuard with a pending exception calls DetachCurrentThread
        // which aborts the JVM.
        let _ = env.exception_clear();
    }

    /// `vibrator.vibrate(long)` — deprecated since API 26 but works on API 23–25.
    fn vibrate_legacy(env: &mut jni::JNIEnv<'_>, vibrator: &JObject<'_>, ms: i64) {
        let _ = env.call_method(vibrator, "vibrate", "(J)V", &[JValue::Long(ms)]);
    }

    /// `VibrationEffect.createOneShot(ms, amplitude)` then `vibrator.vibrate(effect)`.
    /// Falls back to legacy if API < 26.
    fn vibrate_oneshot(
        env: &mut jni::JNIEnv<'_>,
        vibrator: &JObject<'_>,
        ms: i64,
        amplitude: i32,
    ) {
        let ve_class = env.find_class("android/os/VibrationEffect");
        match ve_class {
            Err(_) => {
                let _ = env.exception_clear();
                vibrate_legacy(env, vibrator, ms);
            }
            Ok(cls) => {
                let ve = env.call_static_method(
                    &cls,
                    "createOneShot",
                    "(JI)Landroid/os/VibrationEffect;",
                    &[JValue::Long(ms), JValue::Int(amplitude)],
                );
                match ve.and_then(|v| v.l()) {
                    Ok(effect) if !effect.is_null() => {
                        let _ = env.call_method(
                            vibrator,
                            "vibrate",
                            "(Landroid/os/VibrationEffect;)V",
                            &[JValue::Object(&effect)],
                        );
                    }
                    _ => {
                        let _ = env.exception_clear();
                        vibrate_legacy(env, vibrator, ms);
                    }
                }
            }
        }
    }

    pub fn play(effect: Effect) {
        // Predefined effect IDs (API 29+): CLICK=0, DOUBLE_CLICK=1, TICK=2, HEAVY_CLICK=5
        let predefined_id: i32 = match effect {
            Effect::Click => 0,
            Effect::DoubleClick => 1,
            Effect::Tick => 2,
            Effect::HeavyClick => 5,
        };
        let fallback_ms: i64 = match effect {
            Effect::Click | Effect::Tick => 30,
            Effect::DoubleClick => 60,
            Effect::HeavyClick => 80,
        };

        with_vibrator(|env, vibrator| {
            let used = (|| -> Option<()> {
                let cls = env.find_class("android/os/VibrationEffect").ok()?;
                let ve = env
                    .call_static_method(
                        &cls,
                        "createPredefined",
                        "(I)Landroid/os/VibrationEffect;",
                        &[JValue::Int(predefined_id)],
                    )
                    .ok()
                    .and_then(|v| v.l().ok())
                    .filter(|v| !v.is_null())?;

                env.call_method(
                    vibrator,
                    "vibrate",
                    "(Landroid/os/VibrationEffect;)V",
                    &[JValue::Object(&ve)],
                )
                .ok()?;
                Some(())
            })();

            if used.is_none() {
                let _ = env.exception_clear();
                vibrate_oneshot(env, vibrator, fallback_ms, 255);
            }
        });
    }

    pub fn buzz(duration: Duration) {
        let ms = duration.as_millis() as i64;
        with_vibrator(|env, vibrator| {
            vibrate_oneshot(env, vibrator, ms, 255);
        });
    }

    pub fn pattern(waveform: Waveform) {
        if waveform.steps.is_empty() {
            return;
        }
        with_vibrator(|env, vibrator| {
            let used = (|| -> Option<()> {
                let cls = env.find_class("android/os/VibrationEffect").ok()?;

                let timings: Vec<i64> = waveform.steps.iter().map(|(ms, _)| *ms as i64).collect();
                let amplitudes: Vec<i32> = waveform.steps.iter().map(|(_, a)| *a as i32).collect();
                let n = timings.len() as i32;

                let timing_arr = env.new_long_array(n).ok()?;
                env.set_long_array_region(&timing_arr, 0, &timings).ok()?;

                let amp_arr = env.new_int_array(n).ok()?;
                env.set_int_array_region(&amp_arr, 0, &amplitudes).ok()?;

                let ve = env
                    .call_static_method(
                        &cls,
                        "createWaveform",
                        "([J[II)Landroid/os/VibrationEffect;",
                        &[
                            JValue::Object(timing_arr.as_ref()),
                            JValue::Object(amp_arr.as_ref()),
                            JValue::Int(-1), // no repeat
                        ],
                    )
                    .ok()
                    .and_then(|v| v.l().ok())
                    .filter(|v| !v.is_null())?;

                env.call_method(
                    vibrator,
                    "vibrate",
                    "(Landroid/os/VibrationEffect;)V",
                    &[JValue::Object(&ve)],
                )
                .ok()?;
                Some(())
            })();

            if used.is_none() {
                let _ = env.exception_clear();
                let total_ms: u64 = waveform
                    .steps
                    .iter()
                    .filter(|(_, amp)| *amp > 0)
                    .map(|(ms, _)| ms)
                    .sum();
                if total_ms > 0 {
                    vibrate_legacy(env, vibrator, total_ms as i64);
                }
            }
        });
    }

    pub fn cancel() {
        with_vibrator(|env, vibrator| {
            let _ = env.call_method(vibrator, "cancel", "()V", &[]);
        });
    }
}
