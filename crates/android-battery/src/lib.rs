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
            android_impl::read().unwrap_or_else(stub)
        }
        #[cfg(not(target_os = "android"))]
        {
            stub()
        }
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

// ── Implementação Android via JNI ─────────────────────────────────────────────

#[cfg(target_os = "android")]
mod android_impl {
    use super::*;
    use jni::{
        objects::{JObject, JValue},
        JavaVM,
    };

    pub fn read() -> Option<BatteryState> {
        let ctx = ndk_context::android_context();

        let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }.ok()?;
        let mut env = vm.attach_current_thread().ok()?;

        // Contexto da Activity (jobject do NativeActivity.clazz)
        let context = unsafe { JObject::from_raw(ctx.context().cast()) };

        // IntentFilter para ACTION_BATTERY_CHANGED (sticky broadcast)
        let filter = env
            .new_object("android/content/IntentFilter", "()V", &[])
            .ok()?;
        let action = env
            .new_string("android.intent.action.BATTERY_CHANGED")
            .ok()?;
        env.call_method(
            &filter,
            "addAction",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&action)],
        )
        .ok()?;

        // registerReceiver(null, filter) retorna o Intent do sticky broadcast
        let null_receiver = JObject::null();
        let intent = env
            .call_method(
                &context,
                "registerReceiver",
                "(Landroid/content/BroadcastReceiver;\
                  Landroid/content/IntentFilter;)\
                  Landroid/content/Intent;",
                &[
                    JValue::Object(&null_receiver),
                    JValue::Object(&filter),
                ],
            )
            .ok()?
            .l()
            .ok()?;

        if intent.is_null() {
            return None;
        }

        let level       = get_int_extra(&mut env, &intent, "level", -1);
        let scale       = get_int_extra(&mut env, &intent, "scale", 100);
        let status      = get_int_extra(&mut env, &intent, "status", 1);
        let temperature = get_int_extra(&mut env, &intent, "temperature", 0);
        let voltage     = get_int_extra(&mut env, &intent, "voltage", 0);
        let health_raw  = get_int_extra(&mut env, &intent, "health", 1);

        if scale <= 0 || level < 0 {
            return None;
        }

        // BATTERY_STATUS_CHARGING = 2, BATTERY_STATUS_FULL = 5
        let is_charging = status == 2 || status == 5;

        let health = match health_raw {
            2 => BatteryHealth::Good,
            3 => BatteryHealth::Overheat,
            4 => BatteryHealth::Dead,
            5 => BatteryHealth::OverVoltage,
            _ => BatteryHealth::Unknown,
        };

        Some(BatteryState {
            level: level as f32 / scale as f32,
            is_charging,
            temperature_c: temperature as f32 / 10.0, // Android retorna em décimos de grau
            voltage_mv: voltage as u32,
            health,
        })
    }

    fn get_int_extra(
        env: &mut jni::JNIEnv<'_>,
        intent: &JObject<'_>,
        key: &str,
        default: i32,
    ) -> i32 {
        let Ok(key_str) = env.new_string(key) else {
            return default;
        };
        env.call_method(
            intent,
            "getIntExtra",
            "(Ljava/lang/String;I)I",
            &[JValue::Object(&key_str), JValue::Int(default)],
        )
        .and_then(|v| v.i())
        .unwrap_or(default)
    }
}

// ── Stream (feature "stream") ─────────────────────────────────────────────────

#[cfg(feature = "stream")]
pub fn stream() -> impl futures::Stream<Item = BatteryState> {
    use std::time::Duration;
    futures::stream::unfold((), |_| async {
        tokio::time::sleep(Duration::from_secs(30)).await;
        Some((BatteryManager::current(), ()))
    })
}
