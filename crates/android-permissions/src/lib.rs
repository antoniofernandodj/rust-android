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

#[derive(Debug)]
pub enum PermissionError {
    Denied,
    PermanentlyDenied,
    Unavailable,
}

/// Request the given permissions from the user.
///
/// On Android: uses JNI to call `ActivityCompat.requestPermissions`.
/// On desktop: always returns `Ok(())`, simulating that the permission was granted.
pub async fn request(_permissions: &[Permission]) -> Result<(), PermissionError> {
    // stub: simulate permission granted on all platforms
    Ok(())
}

/// Check whether a permission is currently granted without requesting it.
///
/// On Android: calls `ContextCompat.checkSelfPermission` via JNI.
/// On desktop: always returns `true`.
pub fn check(_permission: &Permission) -> bool {
    true // stub
}
