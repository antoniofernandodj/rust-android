# Integrando Rust com APIs Nativas do Android (JNI)

Este guia explica como expandir este projeto para acessar serviços de baixo nível do Android (GPS, Câmera, Notificações) utilizando JNI (Java Native Interface).

## 1. A Arquitetura Híbrida

Atualmente, o app roda inteiramente em Rust através do `NativeActivity`. Para acessar APIs que o Rust ainda não mapeou, usamos o JNI como uma ponte:

```text
[ Camada Java/Kotlin ] <---- JNI ----> [ Camada Rust (Iced) ]
(GPS, Camera, Push)                     (UI, Lógica, Renderização)
```

## 2. Configurando o Ambiente

### Passo 1: Adicionar a dependência
No `Cargo.toml`, adicione a crate `jni`:

```toml
[target.'cfg(target_os = "android")'.dependencies]
jni = "0.21"
```

### Passo 2: O "Ponto de Contato" no Rust
No seu `src/lib.rs`, você define funções que seguem a convenção de nome do JNI:

```rust
use jni::objects::{JClass, JString};
use jni::JNIEnv;

#[no_mangle]
pub extern "system" fn Java_com_example_rustandroid_MainActivity_triggerVibration(
    mut env: JNIEnv,
    _class: JClass,
    duration: jni::sys::jlong,
) {
    // Lógica para vibrar o celular (chamando o serviço de sistema)
}
```

## 3. Exemplos Práticos

### A. Acessando o GPS (Location Manager)
Para o GPS, o Rust precisa pedir ao contexto do Android o `LocationManager`.

```rust
pub fn get_location(env: &mut JNIEnv, context: jni::objects::JObject) {
    let location_service = env.new_string("location").unwrap();
    let manager = env.call_method(
        context,
        "getSystemService",
        "(Ljava/lang/String;)Ljava/lang/Object;",
        &[location_service.into()],
    ).unwrap().l().unwrap();
    
    // Agora 'manager' pode chamar 'getLastKnownLocation'
}
```

### B. Notificações Push
Notificações geralmente envolvem o Firebase (FCM). O Rust não recebe o push diretamente; o `FirebaseMessagingService` (Java) recebe e então chama uma função `extern "C"` no seu código Rust para atualizar a UI do Iced.

### C. Câmera
A câmera é complexa para rodar puramente em Rust via JNI devido ao fluxo de dados de imagem. A melhor abordagem é:
1.  Abrir a câmera via Java/Kotlin (Camera2 API).
2.  Passar o `SurfaceTexture` para o Rust.
3.  O Rust (através do `wgpu`) renderiza o frame da câmera como uma textura dentro do app Iced.

## 4. Como integrar com o ciclo de vida do Iced

O maior desafio é: **como o Java avisa o Iced que algo aconteceu?**

Você deve usar um **Canal (mpsc)**:
1.  No `App::update`, você escuta um canal de mensagens.
2.  Sua função JNI (que é chamada pelo Android) envia dados para esse canal.
3.  O Iced recebe a mensagem e atualiza a tela (ex: mostra a latitude/longitude recebida).

## 5. Próximos Passos recomendados

Se você planeja usar muitos serviços do sistema, considere as seguintes crates que já fazem o "trabalho sujo" de JNI para você:
*   `ndk`: Para acesso direto a APIs C do Android (sensores, áudio).
*   `android_logger`: Para ver os logs no Logcat (já incluso neste projeto).
*   `crossbeam-channel`: Para comunicação segura entre as threads do JNI e a thread da UI.
