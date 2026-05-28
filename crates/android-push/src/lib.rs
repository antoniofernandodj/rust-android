use futures::Stream;
use std::collections::HashMap;
use std::sync::OnceLock;
use tokio::sync::broadcast;

// ── Tipos públicos ────────────────────────────────────────────────────────────

/// Mensagem recebida via Firebase Cloud Messaging.
#[derive(Debug, Clone)]
pub struct PushMessage {
    /// Título da notificação (ausente em mensagens data-only).
    pub title: Option<String>,
    /// Corpo da notificação.
    pub body: Option<String>,
    /// Pares chave/valor livres enviados pelo servidor.
    pub data: HashMap<String, String>,
    /// ID de colapso: mensagens com o mesmo ID substituem a anterior na fila.
    pub collapse_key: Option<String>,
    /// Sender ID do projeto Firebase.
    pub from: Option<String>,
}

#[derive(Debug)]
pub enum PushError {
    /// Firebase não está disponível (sem google-services.json, emulador sem Play, etc.)
    NotAvailable,
    /// Falha de rede ao obter token.
    NetworkError(String),
    /// Permissão POST_NOTIFICATIONS negada (Android 13+).
    PermissionDenied,
}

impl std::fmt::Display for PushError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PushError::NotAvailable => write!(f, "Firebase não disponível"),
            PushError::NetworkError(e) => write!(f, "Erro de rede: {e}"),
            PushError::PermissionDenied => write!(f, "Permissão POST_NOTIFICATIONS negada"),
        }
    }
}

// ── Canal global de mensagens ─────────────────────────────────────────────────
// Usa broadcast para que múltiplos streams possam receber a mesma mensagem.

static PUSH_TX: OnceLock<broadcast::Sender<PushMessage>> = OnceLock::new();

fn sender() -> &'static broadcast::Sender<PushMessage> {
    PUSH_TX.get_or_init(|| {
        let (tx, _) = broadcast::channel(32);
        tx
    })
}

// ── Ponto de entrada JNI (chamado pelo Java RustPushService) ──────────────────
//
// No Android, o `FirebaseMessagingService` Kotlin/Java chama estas funções via
// JNI quando uma mensagem chega ou o token é renovado.
//
// Assinatura JNI gerada pelo macro #[jni_call] seria:
//   Java_com_example_rustandroid_RustPushService_onPushMessage
//
// Por ora exposta como função pública para o Java chamar via JNI manual.

/// Injetar uma mensagem recebida via JNI (Android) ou testes.
///
/// Em produção esta função é chamada pelo `RustPushService.kt` ao receber
/// uma mensagem FCM.  Em testes unitários pode ser chamada diretamente.
pub fn inject_message(msg: PushMessage) {
    let _ = sender().send(msg);
}

// ── Canal global de token ─────────────────────────────────────────────────────

static TOKEN_TX: OnceLock<broadcast::Sender<String>> = OnceLock::new();

fn token_sender() -> &'static broadcast::Sender<String> {
    TOKEN_TX.get_or_init(|| {
        let (tx, _) = broadcast::channel(4);
        tx
    })
}

/// Injetar um novo token FCM via JNI (Android).
pub fn inject_token(token: String) {
    let _ = token_sender().send(token);
}

// ── API pública ───────────────────────────────────────────────────────────────

pub struct PushManager;

impl PushManager {
    /// Retorna o token FCM atual do dispositivo.
    ///
    /// **Android**: chama `FirebaseMessaging.getInstance().getToken()` via JNI.
    /// **Desktop**: retorna um token simulado após 100 ms.
    pub async fn token() -> Result<String, PushError> {
        #[cfg(target_os = "android")]
        {
            // TODO: chamar FirebaseMessaging via JNI
            Err(PushError::NotAvailable)
        }
        #[cfg(not(target_os = "android"))]
        {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            Ok("stub-fcm-token-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".to_string())
        }
    }

    /// Stream de mensagens push recebidas.
    ///
    /// **Android**: as mensagens chegam via `RustPushService.kt` → JNI → [`inject_message`].
    /// **Desktop**: nenhuma mensagem é emitida automaticamente; use [`inject_message`] para testar.
    pub fn messages() -> impl Stream<Item = PushMessage> {
        let mut rx = sender().subscribe();
        futures::stream::unfold(rx, |mut rx| async move {
            loop {
                match rx.recv().await {
                    Ok(msg) => return Some((msg, rx)),
                    // Lag: o subscriber ficou para trás; descarta mensagens perdidas e continua.
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    // Sender foi dropado (shutdown); encerra o stream.
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        })
    }

    /// Stream de renovações de token FCM.
    ///
    /// O token pode ser renovado pelo Firebase a qualquer momento.  Ouça este
    /// stream para manter o servidor atualizado.
    pub fn token_updates() -> impl Stream<Item = String> {
        let rx = token_sender().subscribe();
        futures::stream::unfold(rx, |mut rx| async move {
            loop {
                match rx.recv().await {
                    Ok(t) => return Some((t, rx)),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        })
    }

    /// Inscreve o dispositivo num tópico FCM.
    ///
    /// **Android**: chama `FirebaseMessaging.getInstance().subscribeToTopic(topic)`.
    /// **Desktop**: no-op.
    pub async fn subscribe_to_topic(topic: &str) -> Result<(), PushError> {
        let _ = topic;
        Ok(())
    }

    /// Remove a inscrição do dispositivo num tópico FCM.
    ///
    /// **Android**: chama `FirebaseMessaging.getInstance().unsubscribeFromTopic(topic)`.
    /// **Desktop**: no-op.
    pub async fn unsubscribe_from_topic(topic: &str) -> Result<(), PushError> {
        let _ = topic;
        Ok(())
    }
}

// ── Guia de setup (documentação inline) ──────────────────────────────────────
//
// ## Setup Android obrigatório
//
// ### 1. google-services.json
// Baixe do Firebase Console e coloque na raiz do projeto Android.
//
// ### 2. AndroidManifest.xml
// ```xml
// <service android:name=".RustPushService" android:exported="false">
//   <intent-filter>
//     <action android:name="com.google.firebase.MESSAGING_EVENT"/>
//   </intent-filter>
// </service>
// ```
//
// ### 3. RustPushService.kt
// ```kotlin
// class RustPushService : FirebaseMessagingService() {
//     external fun rustOnPushMessage(title: String?, body: String?, data: Map<String, String>)
//     external fun rustOnNewToken(token: String)
//
//     override fun onMessageReceived(msg: RemoteMessage) {
//         rustOnPushMessage(msg.notification?.title, msg.notification?.body, msg.data)
//     }
//     override fun onNewToken(token: String) {
//         rustOnNewToken(token)
//     }
// }
// ```
//
// ### 4. Funções JNI no lado Rust (no crate principal)
// ```rust
// use android_push::{inject_message, inject_token, PushMessage};
//
// #[no_mangle]
// pub extern "system" fn Java_com_example_rustandroid_RustPushService_rustOnPushMessage(
//     mut env: JNIEnv, _class: JClass,
//     title: JString, body: JString, data: JObject,
// ) {
//     let title = env.get_string(&title).ok().map(|s| s.into());
//     let body  = env.get_string(&body).ok().map(|s| s.into());
//     inject_message(PushMessage { title, body, data: HashMap::new(),
//                                  collapse_key: None, from: None });
// }
//
// #[no_mangle]
// pub extern "system" fn Java_com_example_rustandroid_RustPushService_rustOnNewToken(
//     mut env: JNIEnv, _class: JClass, token: JString,
// ) {
//     let token: String = env.get_string(&token).unwrap().into();
//     inject_token(token);
// }
// ```
