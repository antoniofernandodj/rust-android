# rustandroid — Como funciona

Documentação técnica do projeto: catálogo de crates, arquitetura do SDK e
explicação detalhada de como o Rust chama código Java via JNI.

---

## O que foi construído

O projeto é um **workspace Cargo** com um app Android em Rust (usando `iced`
para a UI) e um SDK modular para acessar periféricos do dispositivo.

```
rustandroid/
├── src/lib.rs                  ← app principal (UI com iced)
├── desktop/                    ← runner para testar no computador
├── patches/iced_winit/         ← patch local do iced_winit para Android
└── crates/
    ├── android-jni-bridge/     ← proc-macro para gerar funções JNI
    ├── android-async/          ← adapta callbacks Java em Future/Stream
    ├── android-permissions/    ← solicitar permissões em runtime
    ├── android-battery/        ← nível e estado da bateria ← implementado de verdade
    ├── android-network/        ← estado de conectividade
    ├── android-sensors/        ← acelerômetro, giroscópio, etc.
    ├── android-haptics/        ← vibração e feedback háptico
    ├── android-notifications/  ← notificações locais
    ├── android-push/           ← push notifications via FCM
    └── android-workmanager/    ← tarefas em background agendadas
```

---

## Catálogo de crates

### `android-jni-bridge` — proc-macro

Elimina o boilerplate de escrever funções JNI manualmente.

```rust
// Sem o macro: nome no formato Java_pacote_Classe_metodo + unsafe + catch_unwind manual
#[no_mangle]
pub unsafe extern "system" fn Java_com_example_rustandroid_Bridge_getBatteryLevel(
    env: JNIEnv, _class: JClass,
) -> jint { ... }

// Com o macro: nome Rust normal, expansão automática
#[jni_call(package = "com.example.rustandroid", class = "Bridge")]
pub fn get_battery_level() -> i32 { ... }
```

O macro converte `get_battery_level` → `getBatteryLevel` (camelCase), monta o
nome JNI completo, adiciona `#[no_mangle] pub unsafe extern "system"` e
envolve o corpo em `std::panic::catch_unwind` para evitar undefined behavior
se o Rust entrar em panic cruzando a fronteira JNI.

---

### `android-async` — adaptadores callback→async

As APIs Android são baseadas em callbacks Java. Este crate converte esses
callbacks em primitivas nativas de Rust.

```rust
// JavaCallback<T>: uma chamada Java que responde uma vez → Future
let nivel: i32 = JavaCallback::new(|tx| {
    // registra listener Java; quando o resultado chegar: tx.send(valor)
}).await;

// JavaStream<T>: listener Java que emite múltiplos eventos → Stream
let eventos: impl Stream<Item = SensorEvent> = JavaStream::new(|tx| {
    // registra listener; cada evento: tx.send(evento)
});
```

Internamente usa `tokio::sync::oneshot` (para `JavaCallback`) e
`tokio::sync::mpsc::unbounded_channel` (para `JavaStream`).

---

### `android-permissions`

Solicita permissões em runtime (Android 6+) de forma assíncrona.

```rust
use android_permissions::{Permission, request};

request(&[Permission::Camera, Permission::AccessFineLocation]).await?;
```

No Android real chama `ActivityCompat.requestPermissions` via JNI e converte
o callback `onRequestPermissionsResult` num `Future`. No desktop simula sempre
`Ok(())`.

---

### `android-battery` — implementado com JNI real

Lê o estado real da bateria. **Única crate com implementação Android completa
até agora** — os outros periféricos têm stubs. Detalhes na seção seguinte.

---

### `android-network`

Monitora conectividade via `ConnectivityManager`.

```rust
let estado = ConnectivityManager::current(); // Wifi, Cellular, None…
let stream = android_network::stream();       // emite ao mudar (feature "stream")
```

Implementação JNI pendente (retorna stub no Android por ora).

---

### `android-sensors`

Acesso ao `SensorManager` — acelerômetro, giroscópio, barômetro, luz, etc.

```rust
let mut stream = Sensor::Accelerometer.stream(SamplingRate::Game);
while let Some(ev) = stream.next().await {
    if let SensorEvent::Accelerometer(v) = ev {
        println!("x={} y={} z={}", v.x, v.y, v.z);
    }
}
```

No desktop emite dados simulados (seno/cosseno) no intervalo configurado.
Feature `stream` necessária para usar `Sensor::stream()`.

---

### `android-haptics`

Vibração via `VibrationEffect` (Android 8+).

```rust
Vibrator::play(Effect::Click);
Vibrator::buzz(Duration::from_millis(200));
Vibrator::pattern(Waveform::new().on(100).off(50).on(200));
```

No desktop é no-op. Implementação JNI pendente.

---

### `android-notifications`

Notificações locais via `NotificationManagerCompat`.

```rust
Channel::new("alertas", "Alertas do App")
    .importance(Priority::High)
    .register();

Notification::new("alertas")
    .title("Olá!")
    .body("Mensagem de teste")
    .action(Action::new("ok", "OK"))
    .show(1);
```

Implementação JNI pendente.

---

### `android-push` — push notifications via FCM

Recebe mensagens do Firebase Cloud Messaging como um `Stream<PushMessage>`.

```rust
// Token do dispositivo (necessário no servidor para enviar push)
let token = PushManager::token().await?;

// Stream de mensagens recebidas
let mut msgs = PushManager::messages();
while let Some(msg) = msgs.next().await {
    println!("push: {:?}", msg.title);
}
```

**Como funciona na prática:** o FCM entrega a mensagem ao processo Java
(`FirebaseMessagingService`), que chama `inject_message()` no Rust via JNI.
Isso alimenta um `broadcast::channel` global. O `Stream` retornado é um
subscriber desse channel. O crate inclui o boilerplate Java necessário
(`RustPushService.kt`) nos comentários do `lib.rs`.

---

### `android-workmanager` — tarefas em background

Agenda trabalho que persiste mesmo com o app fechado.

```rust
// Tarefa única com restrição de rede
let id = WorkManager::enqueue(
    OneTimeWorkRequest::new(|| async {
        sync_data().await;
        WorkResult::Success
    })
    .constraints(Constraints::new().requires_network()),
).await;

// Tarefa periódica (a cada 15 min)
WorkManager::enqueue_periodic(
    PeriodicWorkRequest::new(Duration::from_secs(900), || async {
        WorkResult::Success
    })
).await;

// Observar status
let mut status = WorkManager::status(&id);
while let Some(s) = status.next().await {
    println!("{:?}", s); // Enqueued → Running → Succeeded
}
```

No desktop executa numa tokio task respeitando os delays. Implementação JNI
pendente (delegaria ao `WorkManager` Jetpack).

---

## Como o Rust chama Java: JNI explicado

### O problema

Um app Android roda em dois mundos simultâneos:

```
┌─────────────────────────────────┐
│  ART (Android Runtime / JVM)    │  ← Java/Kotlin, framework Android
│  Activity, Intent, Context...   │
├─────────────────────────────────┤
│  Código Nativo (.so)            │  ← Rust, C, C++
│  librustandroid.so              │
└─────────────────────────────────┘
```

O Rust precisa chamar APIs Java (como ler a bateria) e o Java precisa chamar
o Rust (como entregar uma mensagem push). A ponte entre os dois é o **JNI
(Java Native Interface)** — um protocolo definido pela JVM para cruzar essa
fronteira.

---

### O que é o JNI

JNI é uma API em C que permite:
- **Java → Nativo**: declarar um método `native` no Java; a JVM procura e
  executa a função correspondente no `.so`
- **Nativo → Java**: pegar uma referência a um objeto Java e chamar seus
  métodos a partir de C/Rust

Tudo passa por dois ponteiros fundamentais:

| Ponteiro | O que é | Para que serve |
|---|---|---|
| `JavaVM*` | A máquina virtual inteira | "Logar" uma thread no JVM para obter um `JNIEnv` |
| `JNIEnv*` | Handle da thread atual | Criar objetos, chamar métodos, ler campos Java |

---

### Como o `ndk-context` guarda o ponteiro

O `android-activity` (crate que conecta a NativeActivity ao Rust) inicializa
o crate `ndk-context` durante o startup do app:

```
android_main() é chamado pelo Java
  └─ android-activity inicializa a AndroidApp
       └─ chama ndk_context::initialize_android_context(vm, activity_clazz)
            ├─ guarda o JavaVM* em um static global
            └─ guarda o jobject da Activity em um static global
```

A partir daí, qualquer código Rust pode obter esses ponteiros com:

```rust
let ctx = ndk_context::android_context();
// ctx.vm()      → *mut c_void  (o JavaVM*)
// ctx.context() → *mut c_void  (o jobject da Activity)
```

---

### Passo a passo: como `BatteryManager::current()` lê a bateria

O Android não expõe o nível de bateria como um arquivo ou syscall — ele usa
o sistema de broadcasts. Quando o nível muda, o sistema publica um
`Intent.ACTION_BATTERY_CHANGED`. Como esse broadcast é "sticky" (fica
guardado pelo sistema), qualquer app pode lê-lo a qualquer hora sem esperar
pelo próximo evento.

O truque: chamar `registerReceiver(null, filter)` com um receiver `null`
retorna imediatamente o último Intent publicado, sem registrar nada.

Aqui está o que acontece linha a linha no `android_impl::read()`:

```rust
// 1. Pegar os ponteiros globais guardados pelo ndk-context
let ctx = ndk_context::android_context();

// 2. Recriar o JavaVM a partir do ponteiro bruto
//    (não cria uma nova VM — apenas envolve o ponteiro existente)
let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }.ok()?;

// 3. Fazer a thread Rust atual se "logar" na JVM
//    Isso é necessário porque a JVM só aceita chamadas de threads registradas.
//    attach_current_thread() cria um JNIEnv para esta thread.
let mut env = vm.attach_current_thread().ok()?;

// 4. Pegar o objeto Java da Activity
//    É o mesmo "clazz" que o android-activity passou para o ndk-context.
let context = unsafe { JObject::from_raw(ctx.context().cast()) };

// 5. Criar um IntentFilter em Java: equivalente a
//    new IntentFilter()
let filter = env.new_object("android/content/IntentFilter", "()V", &[]).ok()?;

// 6. Chamar filter.addAction("android.intent.action.BATTERY_CHANGED")
let action = env.new_string("android.intent.action.BATTERY_CHANGED").ok()?;
env.call_method(&filter, "addAction", "(Ljava/lang/String;)V",
    &[JValue::Object(&action)]).ok()?;

// 7. Chamar context.registerReceiver(null, filter)
//    null como receiver = não registra nada; apenas lê o sticky broadcast
//    Retorna o Intent com os dados atuais da bateria
let null_receiver = JObject::null();
let intent = env.call_method(
    &context,
    "registerReceiver",
    "(Landroid/content/BroadcastReceiver;\
      Landroid/content/IntentFilter;)\
      Landroid/content/Intent;",
    &[JValue::Object(&null_receiver), JValue::Object(&filter)],
).ok()?.l().ok()?;

// 8. Ler os extras do Intent: equivalente a
//    intent.getIntExtra("level", -1)
let level = get_int_extra(&mut env, &intent, "level", -1);
let scale = get_int_extra(&mut env, &intent, "scale", 100);
// "level" vai de 0 a "scale" (geralmente 0–100, mas não sempre)
// Então a porcentagem real é: level / scale

// 9. Montar o BatteryState com os valores lidos
Some(BatteryState {
    level: level as f32 / scale as f32,   // ex: 40 / 100 = 0.40 = 40%
    is_charging: status == 2 || status == 5,
    temperature_c: temperature as f32 / 10.0, // Android usa décimos de grau
    voltage_mv: voltage as u32,
    health: ...,
})
```

### A assinatura de método Java no JNI

A string `"(Ljava/lang/String;I)I"` é o **descritor de tipo** do método Java
no formato JNI. A leitura é:

```
(Ljava/lang/String;I)I
 ─────────────────── ─
       argumentos    retorno

L → objeto (tipo que segue até o ;)
  java/lang/String → java.lang.String
I → int
```

Outros tipos comuns:

| Java | JNI |
|---|---|
| `int` | `I` |
| `long` | `J` |
| `float` | `F` |
| `double` | `D` |
| `boolean` | `Z` |
| `void` | `V` |
| `String` | `Ljava/lang/String;` |
| `Intent` | `Landroid/content/Intent;` |
| `int[]` | `[I` |

---

### Por que `attach_current_thread` é necessário

A JVM só aceita chamadas JNI de threads que estão "registradas" nela. Threads
criadas pelo Java já estão registradas. Threads criadas pelo Rust (ou pelo
tokio runtime) precisam se registrar com `vm.attach_current_thread()`.

O crate `jni` faz isso automaticamente e **desregistra a thread quando o
guard cai** (RAII), então não há leak de recursos.

---

### Fluxo contrário: Java → Rust (push notifications)

No caso do push, o fluxo é invertido — o Java chama o Rust:

```
FCM chega no servidor Firebase
  └─ Firebase entrega ao processo Java via GCM/FCM socket
       └─ RustPushService.onMessageReceived() é chamado
            └─ chama rustOnPushMessage() — declarado como `native` no Kotlin
                 └─ JVM procura Java_com_example_rustandroid_RustPushService_rustOnPushMessage
                      └─ função Rust é executada
                           └─ chama inject_message(msg)
                                └─ msg é enviada para o broadcast::channel global
                                     └─ PushManager::messages() Stream emite o item
```

O Rust não "espera" o push — ele disponibiliza um `Stream` que é alimentado
quando o Java chama a função JNI.

---

## Estado atual × próximos passos

| Crate | Desktop | Android |
|---|---|---|
| `android-jni-bridge` | ✅ macro funciona | ✅ gera símbolos JNI corretos |
| `android-async` | ✅ channels reais | ⏳ JNI pendente |
| `android-permissions` | ✅ stub Ok(()) | ⏳ JNI pendente |
| `android-battery` | ✅ stub 85% | ✅ **JNI real — lê bateria real** |
| `android-network` | ✅ stub Wifi | ⏳ JNI pendente |
| `android-sensors` | ✅ dados simulados | ⏳ JNI pendente |
| `android-haptics` | ✅ no-op | ⏳ JNI pendente |
| `android-notifications` | ✅ no-op | ⏳ JNI pendente |
| `android-push` | ✅ inject manual | ⏳ RustPushService.kt pendente |
| `android-workmanager` | ✅ tokio tasks | ⏳ JNI pendente |

A ordem sugerida para implementar o JNI dos próximos crates (do mais simples
ao mais complexo): `android-haptics` → `android-notifications` →
`android-sensors` → `android-network` → `android-permissions` →
`android-push` → `android-workmanager`.
