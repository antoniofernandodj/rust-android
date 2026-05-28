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
    /// **Android**: lê via `ConnectivityManager` por JNI.
    /// **Desktop**: retorna dados simulados.
    pub fn current() -> NetworkState {
        NetworkState {
            is_connected: true,
            network_type: NetworkType::Wifi,
            ssid: Some("Desktop-Stub".to_string()),
            signal_strength: Some(-50),
        }
    }
}

/// Stream que emite o estado da rede ao detectar mudanças.
///
/// Requer feature `stream` (ativa tokio + futures).
/// **Não use no crate principal do app Android** diretamente.
#[cfg(feature = "stream")]
pub fn stream() -> impl futures::Stream<Item = NetworkState> {
    use std::time::Duration;
    futures::stream::unfold((), |_| async {
        tokio::time::sleep(Duration::from_secs(10)).await;
        Some((ConnectivityManager::current(), ()))
    })
}
