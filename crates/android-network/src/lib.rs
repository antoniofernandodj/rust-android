#[derive(Debug, Clone, PartialEq)]
pub enum NetworkType {
    Wifi,
    Cellular,
    Ethernet,
    None,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct NetworkState {
    pub is_connected: bool,
    pub network_type: NetworkType,
    pub ssid: Option<String>,
    pub signal_strength: Option<i32>,
}

impl NetworkState {
    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    pub fn network_type(&self) -> &NetworkType {
        &self.network_type
    }
}

pub struct ConnectivityManager;

impl ConnectivityManager {
    /// Retorna o estado atual da rede.
    ///
    /// **Android**: lê via `ConnectivityManager` por JNI (requer ACCESS_NETWORK_STATE).
    /// **Desktop**: retorna dados simulados.
    pub fn current() -> NetworkState {
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

fn stub() -> NetworkState {
    NetworkState {
        is_connected: true,
        network_type: NetworkType::Wifi,
        ssid: Some("Desktop-Stub".to_string()),
        signal_strength: Some(-50),
    }
}

/// Stream que emite o estado da rede periodicamente.
///
/// Requer feature `stream` (ativa tokio + futures).
#[cfg(feature = "stream")]
pub fn stream() -> impl futures::Stream<Item = NetworkState> {
    use std::time::Duration;
    futures::stream::unfold((), |_| async {
        tokio::time::sleep(Duration::from_secs(10)).await;
        Some((ConnectivityManager::current(), ()))
    })
}

// ── Android JNI implementation ────────────────────────────────────────────────

#[cfg(target_os = "android")]
mod android_impl {
    use super::*;
    use jni::{
        objects::{JObject, JValue},
        JavaVM,
    };

    // Android NetworkCapabilities transport constants
    const TRANSPORT_CELLULAR: i32 = 0;
    const TRANSPORT_WIFI: i32 = 1;
    const TRANSPORT_ETHERNET: i32 = 3;
    // NET_CAPABILITY_INTERNET = 12
    const NET_CAPABILITY_INTERNET: i32 = 12;

    pub fn read() -> Option<NetworkState> {
        let ctx = ndk_context::android_context();
        let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }.ok()?;
        let mut env = vm.attach_current_thread().ok()?;
        let context = unsafe { JObject::from_raw(ctx.context().cast()) };

        // context.getSystemService("connectivity")
        let svc = env.new_string("connectivity").ok()?;
        let cm = env
            .call_method(
                &context,
                "getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                &[JValue::Object(&svc)],
            )
            .ok()?
            .l()
            .ok()?;

        if cm.is_null() {
            return None;
        }

        // getActiveNetwork() -> Network  (API 23+)
        let active_net = env
            .call_method(&cm, "getActiveNetwork", "()Landroid/net/Network;", &[])
            .ok()?
            .l()
            .ok()?;

        if active_net.is_null() {
            return Some(NetworkState {
                is_connected: false,
                network_type: NetworkType::None,
                ssid: None,
                signal_strength: None,
            });
        }

        // getNetworkCapabilities(network) -> NetworkCapabilities
        let caps = env
            .call_method(
                &cm,
                "getNetworkCapabilities",
                "(Landroid/net/Network;)Landroid/net/NetworkCapabilities;",
                &[JValue::Object(&active_net)],
            )
            .ok()?
            .l()
            .ok()?;

        if caps.is_null() {
            return Some(NetworkState {
                is_connected: true,
                network_type: NetworkType::Unknown,
                ssid: None,
                signal_strength: None,
            });
        }

        let has_internet = env
            .call_method(&caps, "hasCapability", "(I)Z", &[JValue::Int(NET_CAPABILITY_INTERNET)])
            .ok()
            .and_then(|v| v.z().ok())
            .unwrap_or(false);

        let has_wifi = transport(&mut env, &caps, TRANSPORT_WIFI);
        let has_cellular = transport(&mut env, &caps, TRANSPORT_CELLULAR);
        let has_ethernet = transport(&mut env, &caps, TRANSPORT_ETHERNET);

        let network_type = if has_wifi {
            NetworkType::Wifi
        } else if has_cellular {
            NetworkType::Cellular
        } else if has_ethernet {
            NetworkType::Ethernet
        } else {
            NetworkType::Unknown
        };

        Some(NetworkState {
            is_connected: has_internet,
            network_type,
            ssid: None,
            signal_strength: None,
        })
    }

    fn transport(env: &mut jni::JNIEnv<'_>, caps: &JObject<'_>, transport: i32) -> bool {
        env.call_method(caps, "hasTransport", "(I)Z", &[JValue::Int(transport)])
            .ok()
            .and_then(|v| v.z().ok())
            .unwrap_or(false)
    }
}
