use std::sync::OnceLock;
use tokio::sync::broadcast;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Permission {
    Camera,
    RecordAudio,
    AccessFineLocation,
    AccessCoarseLocation,
    AccessBackgroundLocation,
    BluetoothScan,
    BluetoothConnect,
    BluetoothAdvertise,
    Bluetooth,
    BluetoothAdmin,
    AccessWifiState,
    ChangeWifiState,
    Nfc,
    Vibrate,
    PostNotifications,
    ReadMediaImages,
    ReadMediaVideo,
    ReadExternalStorage,
    WriteExternalStorage,
    UseBiometric,
    UseFingerprint,
    WakeLock,
    WriteSettings,
    AccessNetworkState,
    ChangeNetworkState,
    BodySensors,
}

impl Permission {
    /// Returns the Android permission string (e.g. `"android.permission.CAMERA"`).
    pub fn android_name(&self) -> &'static str {
        match self {
            Permission::Camera => "android.permission.CAMERA",
            Permission::RecordAudio => "android.permission.RECORD_AUDIO",
            Permission::AccessFineLocation => "android.permission.ACCESS_FINE_LOCATION",
            Permission::AccessCoarseLocation => "android.permission.ACCESS_COARSE_LOCATION",
            Permission::AccessBackgroundLocation => {
                "android.permission.ACCESS_BACKGROUND_LOCATION"
            }
            Permission::BluetoothScan => "android.permission.BLUETOOTH_SCAN",
            Permission::BluetoothConnect => "android.permission.BLUETOOTH_CONNECT",
            Permission::BluetoothAdvertise => "android.permission.BLUETOOTH_ADVERTISE",
            Permission::Bluetooth => "android.permission.BLUETOOTH",
            Permission::BluetoothAdmin => "android.permission.BLUETOOTH_ADMIN",
            Permission::AccessWifiState => "android.permission.ACCESS_WIFI_STATE",
            Permission::ChangeWifiState => "android.permission.CHANGE_WIFI_STATE",
            Permission::Nfc => "android.permission.NFC",
            Permission::Vibrate => "android.permission.VIBRATE",
            Permission::PostNotifications => "android.permission.POST_NOTIFICATIONS",
            Permission::ReadMediaImages => "android.permission.READ_MEDIA_IMAGES",
            Permission::ReadMediaVideo => "android.permission.READ_MEDIA_VIDEO",
            Permission::ReadExternalStorage => "android.permission.READ_EXTERNAL_STORAGE",
            Permission::WriteExternalStorage => "android.permission.WRITE_EXTERNAL_STORAGE",
            Permission::UseBiometric => "android.permission.USE_BIOMETRIC",
            Permission::UseFingerprint => "android.permission.USE_FINGERPRINT",
            Permission::WakeLock => "android.permission.WAKE_LOCK",
            Permission::WriteSettings => "android.permission.WRITE_SETTINGS",
            Permission::AccessNetworkState => "android.permission.ACCESS_NETWORK_STATE",
            Permission::ChangeNetworkState => "android.permission.CHANGE_NETWORK_STATE",
            Permission::BodySensors => "android.permission.BODY_SENSORS",
        }
    }
}

#[derive(Debug)]
pub enum PermissionError {
    Denied,
    PermanentlyDenied,
    Unavailable,
}

// ── Global channel for permission request results ─────────────────────────────
//
// Java's onRequestPermissionsResult calls `on_request_permissions_result` (below),
// which sends (request_code, all_granted) to this channel.
// `request()` awaits the matching result.

static RESULT_TX: OnceLock<broadcast::Sender<(i32, bool)>> = OnceLock::new();
#[cfg(target_os = "android")]
static REQUEST_CODE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(1000);

fn result_sender() -> &'static broadcast::Sender<(i32, bool)> {
    RESULT_TX.get_or_init(|| {
        let (tx, _) = broadcast::channel(16);
        tx
    })
}

/// Called from the Android Activity's `onRequestPermissionsResult` via JNI.
///
/// `granted` should be `true` iff every permission in the request was granted.
///
/// ## Wiring (in your Kotlin Activity):
/// ```kotlin
/// override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<String>, grantResults: IntArray) {
///     super.onRequestPermissionsResult(requestCode, permissions, grantResults)
///     val allGranted = grantResults.all { it == PackageManager.PERMISSION_GRANTED }
///     onPermissionsResult(requestCode, allGranted)
/// }
/// external fun onPermissionsResult(requestCode: Int, allGranted: Boolean)
/// ```
pub fn on_request_permissions_result(request_code: i32, all_granted: bool) {
    let _ = result_sender().send((request_code, all_granted));
}

/// Check whether a permission is currently granted without requesting it.
///
/// On Android: calls `Context.checkSelfPermission` via JNI (API 23+).
/// On desktop: always returns `true`.
pub fn check(permission: &Permission) -> bool {
    #[cfg(target_os = "android")]
    {
        android_impl::check(permission.android_name())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = permission;
        true
    }
}

/// Fire a permission request dialog without waiting for the result.
///
/// **Must be called from the Android UI/main thread** (e.g. inside iced's `update()`).
/// After the user responds, call `check()` to read the updated state.
///
/// On desktop: no-op.
pub fn fire_request(permissions: &[Permission]) {
    #[cfg(target_os = "android")]
    {
        let code = REQUEST_CODE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        android_impl::request_permissions(permissions, code);
    }
    #[cfg(not(target_os = "android"))]
    let _ = permissions;
}

/// Request the given permissions from the user (async, awaits result).
///
/// On Android: calls `Activity.requestPermissions` via JNI and awaits the result
/// via `on_request_permissions_result`.  Times out after 60 s if the user ignores
/// the dialog.
///
/// **Requires Java wiring** — see `on_request_permissions_result` docs.
///
/// On desktop: always returns `Ok(())`.
pub async fn request(permissions: &[Permission]) -> Result<(), PermissionError> {
    #[cfg(target_os = "android")]
    {
        // Fast path: already granted
        if permissions.iter().all(check) {
            return Ok(());
        }

        let code = REQUEST_CODE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut rx = result_sender().subscribe();

        android_impl::request_permissions(permissions, code);

        // Wait up to 60 s for the user to respond
        let result = tokio::time::timeout(std::time::Duration::from_secs(60), async move {
            loop {
                match rx.recv().await {
                    Ok((c, granted)) if c == code => return granted,
                    Ok(_) => continue,
                    Err(_) => return false,
                }
            }
        })
        .await;

        match result {
            Ok(true) => Ok(()),
            Ok(false) => Err(PermissionError::Denied),
            Err(_) => Err(PermissionError::Denied),
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = permissions;
        Ok(())
    }
}

// ── Android JNI implementation ────────────────────────────────────────────────

#[cfg(target_os = "android")]
mod android_impl {
    use super::Permission;
    use jni::{
        objects::{JObject, JObjectArray, JValue},
        JavaVM,
    };

    // PackageManager.PERMISSION_GRANTED = 0
    const PERMISSION_GRANTED: i32 = 0;

    pub fn check(permission_name: &str) -> bool {
        let ctx = ndk_context::android_context();
        let Ok(vm) = (unsafe { JavaVM::from_raw(ctx.vm().cast()) }) else { return false };
        let Ok(mut env) = vm.attach_current_thread() else { return false };
        let context = unsafe { JObject::from_raw(ctx.context().cast()) };

        let Ok(perm) = env.new_string(permission_name) else { return false };
        let result = env.call_method(
            &context,
            "checkSelfPermission",
            "(Ljava/lang/String;)I",
            &[JValue::Object(&perm)],
        );
        // Must clear any pending Java exception BEFORE the env is dropped.
        // Dropping AttachGuard with a pending exception calls DetachCurrentThread,
        // which aborts the JVM.
        let _ = env.exception_clear();

        matches!(result.and_then(|v| v.i()), Ok(code) if code == PERMISSION_GRANTED)
    }

    pub fn request_permissions(permissions: &[Permission], request_code: i32) {
        let ctx = ndk_context::android_context();
        let Ok(vm) = (unsafe { JavaVM::from_raw(ctx.vm().cast()) }) else { return };
        let Ok(mut env) = vm.attach_current_thread() else { return };
        // NativeActivity.clazz IS the Activity instance
        let activity = unsafe { JObject::from_raw(ctx.context().cast()) };

        let Some(arr) = build_string_array(&mut env, permissions) else { return };

        let _ = env.call_method(
            &activity,
            "requestPermissions",
            "([Ljava/lang/String;I)V",
            &[JValue::Object(&arr), JValue::Int(request_code)],
        );
    }

    fn build_string_array<'a>(
        env: &mut jni::JNIEnv<'a>,
        permissions: &[Permission],
    ) -> Option<JObjectArray<'a>> {
        let string_cls = env.find_class("java/lang/String").ok()?;
        let arr = env
            .new_object_array(permissions.len() as i32, &string_cls, JObject::null())
            .ok()?;
        for (i, perm) in permissions.iter().enumerate() {
            let s = env.new_string(perm.android_name()).ok()?;
            env.set_object_array_element(&arr, i as i32, s).ok()?;
        }
        Some(arr)
    }
}
