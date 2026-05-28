use futures::Stream;

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
    /// Return the current network state.
    ///
    /// On Android: reads via `ConnectivityManager` using JNI.
    /// On desktop: returns simulated data.
    pub fn current() -> NetworkState {
        NetworkState {
            is_connected: true,
            network_type: NetworkType::Wifi,
            ssid: Some("Desktop-Stub".to_string()),
            signal_strength: Some(-50),
        }
    }

    /// Returns a `Stream` that emits the network state periodically.
    ///
    /// On Android: emits when connectivity changes.
    /// On desktop: emits simulated data every 10 seconds.
    pub fn stream() -> impl Stream<Item = NetworkState> {
        use std::time::Duration;
        futures::stream::unfold((), |_| async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Some((ConnectivityManager::current(), ()))
        })
    }
}
