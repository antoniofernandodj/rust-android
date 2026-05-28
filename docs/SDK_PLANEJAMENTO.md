# Planejamento: rust-android-sdk

> Visão de longo prazo para transformar este projeto num SDK Rust completo para
> desenvolvimento de apps Android, com acesso a periféricos do dispositivo via
> JNI, streams assíncronos e widgets prontos para iced.

---

## O problema

Hoje o ecossistema Rust para Android está fragmentado: existem crates isoladas
para algumas APIs do NDK, mas nenhum SDK coeso que permita a um dev Rust acessar
câmera, Bluetooth, sensores ou notificações com a mesma naturalidade com que
faria em Kotlin. Este projeto está posicionado para ser essa fundação.

---

## A visão

Um workspace com ~20 crates, organizadas em quatro camadas:

```
rust-android-sdk/
├── app/                        ← app de demonstração
├── desktop/                    ← runner desktop para testes rápidos
├── crates/
│   ├── camada-1-infraestrutura/
│   │   ├── android-jni-bridge/
│   │   ├── android-permissions/
│   │   └── android-async/
│   ├── camada-2-perifericos/
│   │   ├── android-sensors/
│   │   ├── android-location/
│   │   ├── android-bluetooth/
│   │   ├── android-wifi/
│   │   ├── android-nfc/
│   │   ├── android-camera/
│   │   ├── android-audio/
│   │   ├── android-notifications/
│   │   ├── android-haptics/
│   │   ├── android-biometrics/
│   │   ├── android-storage/
│   │   ├── android-battery/
│   │   ├── android-display/
│   │   ├── android-network/
│   │   └── android-usb/
│   ├── camada-3-widgets/
│   │   └── iced-android-widgets/
│   └── camada-4-tooling/
│       └── android-dev-tools/
└── cargo-template/             ← template para novos projetos
```

---

## Camada 1 — Infraestrutura

Estes três crates são a base que todos os outros dependem. Sem eles, cada crate
de periférico seria verboso e inconsistente.

### `android-jni-bridge`

O maior problema de escrever JNI manualmente é o boilerplate: nomes de função
no formato `Java_com_example_pacote_Classe_metodo`, conversão de tipos Java ↔
Rust, tratamento de exceções Java. Este crate resolve com um proc-macro:

```rust
// sem android-jni-bridge
#[no_mangle]
pub unsafe extern "system" fn Java_com_example_rustandroid_Bridge_getBatteryLevel(
    env: JNIEnv,
    _class: JClass,
) -> jint {
    // ...
}

// com android-jni-bridge
#[jni_call(package = "com.example.rustandroid", class = "Bridge")]
fn get_battery_level(env: &JNIEnv) -> i32 {
    // ...
}
```

O macro gera o nome correto, faz o casting de tipos e envolve o corpo em
`catch_unwind` para evitar undefined behavior se o Rust entrar em panic
cruzando a fronteira JNI.

**Dependências:** `proc-macro2`, `quote`, `syn`, `jni`

---

### `android-permissions`

Toda API Android sensível exige permissão em tempo de execução (Android 6+). Sem
um gerenciador centralizado, cada crate de periférico precisaria reimplementar
a mesma lógica de diálogo → callback → resultado. Este crate expõe:

```rust
use android_permissions::{Permission, request};

// retorna Ok(()) se concedida, Err(Denied) se negada
request(&[Permission::Camera, Permission::AccessFineLocation]).await?;
```

Internamente usa `ActivityCompat.requestPermissions` via JNI e converte o
callback `onRequestPermissionsResult` em um `Future` com um `oneshot::channel`.

**Dependências:** `android-jni-bridge`, `tokio`, `jni`

---

### `android-async`

A maioria das APIs Android é baseada em callbacks Java. Este crate fornece
adaptadores para converter esses callbacks em primitivas async Rust:

```rust
// JavaCallback<T>: converte callback Java em Future Rust
let level = JavaCallback::<i32>::new(|tx| {
    // registra listener Java que chama tx.send(valor) quando disponível
}).await;

// JavaStream<T>: converte listener Java em Stream contínuo
let frames: impl Stream<Item = Frame> = JavaStream::new(|tx| {
    // registra ImageReader.OnImageAvailableListener
});
```

**Dependências:** `tokio`, `futures`, `jni`

---

## Camada 2 — Periféricos

Cada crate encapsula uma categoria de APIs Android. A interface pública
segue sempre o mesmo padrão: permissão verificada na inicialização, dados
entregues como `Stream<T>` ou `Future<T>`, erros tipados.

---

### `android-sensors`

Acesso ao `SensorManager` do Android. Cobre todos os sensores de hardware:

| Sensor | Tipo Android | Struct Rust |
|---|---|---|
| Acelerômetro | `TYPE_ACCELEROMETER` | `Vec3 { x, y, z }` m/s² |
| Giroscópio | `TYPE_GYROSCOPE` | `Vec3` rad/s |
| Magnetômetro | `TYPE_MAGNETIC_FIELD` | `Vec3` µT |
| Barômetro | `TYPE_PRESSURE` | `f32` hPa |
| Sensor de luz | `TYPE_LIGHT` | `f32` lux |
| Proximidade | `TYPE_PROXIMITY` | `f32` cm |
| Contador de passos | `TYPE_STEP_COUNTER` | `u64` passos |
| Vetor de rotação | `TYPE_ROTATION_VECTOR` | `Quaternion` |

```rust
use android_sensors::{Sensor, SamplingRate};

let mut stream = Sensor::Accelerometer.stream(SamplingRate::Game);
while let Some(event) = stream.next().await {
    println!("x={:.2} y={:.2} z={:.2}", event.x, event.y, event.z);
}
```

**Permissões necessárias:** nenhuma (sensores de movimento e posição são livres;
`BODY_SENSORS` só para frequência cardíaca e similares).

---

### `android-location`

Acesso ao `FusedLocationProviderClient` (API de localização moderna do Google)
com fallback para `LocationManager` nativo:

```rust
use android_location::{Location, Accuracy};

// leitura única
let pos: Location = android_location::last_known().await?;

// stream contínuo
let mut updates = android_location::updates(Accuracy::High).await?;
while let Some(loc) = updates.next().await {
    println!("lat={} lon={} alt={:?}", loc.latitude, loc.longitude, loc.altitude);
}
```

Também expõe geofencing: definir regiões circulares e receber eventos de
entrada/saída.

**Permissões:** `ACCESS_FINE_LOCATION`, `ACCESS_COARSE_LOCATION`,
`ACCESS_BACKGROUND_LOCATION` (para localização em segundo plano)

---

### `android-bluetooth`

Cobertura completa de Bluetooth Clássico e BLE (Bluetooth Low Energy):

**BLE (caso de uso principal):**
```rust
use android_bluetooth::ble::{Scanner, ScanFilter, GattClient};

// scanning
let mut scan = Scanner::start(ScanFilter::by_service(MY_SERVICE_UUID));
while let Some(device) = scan.next().await {
    println!("encontrado: {} RSSI={}", device.name(), device.rssi());
}

// conexão GATT
let client = GattClient::connect(device).await?;
let value = client.read_characteristic(CHAR_UUID).await?;
client.write_characteristic(CHAR_UUID, &[0x01]).await?;
client.subscribe(CHAR_UUID, |data| { /* notificações */ }).await?;
```

**Bluetooth Clássico:**
- Descoberta de dispositivos
- RFCOMM (serial over Bluetooth)
- Perfis: HID (teclado/mouse), A2DP (áudio), SPP (serial)

**Periférico BLE (modo servidor):**
- Advertising de serviços GATT
- Resposta a leituras/escritas de características

**Permissões:** `BLUETOOTH_SCAN`, `BLUETOOTH_CONNECT`, `BLUETOOTH_ADVERTISE`
(Android 12+); `BLUETOOTH`, `BLUETOOTH_ADMIN` (Android < 12)

---

### `android-wifi`

```rust
use android_wifi::{WifiManager, Network};

// scan de redes
let networks: Vec<Network> = WifiManager::scan().await?;

// conexão
WifiManager::connect("MinhaRede", "senha123").await?;

// estado atual
let state = WifiManager::state(); // Connected { ssid, bssid, rssi, ip }

// hotspot (SoftAP)
WifiManager::start_hotspot("MeuHotspot", "senha456").await?;

// WiFi Direct (P2P)
let peers = WifiManager::p2p_discover().await?;
```

**Permissões:** `ACCESS_WIFI_STATE`, `CHANGE_WIFI_STATE`,
`ACCESS_FINE_LOCATION` (obrigatório para scan no Android 10+)

---

### `android-nfc`

```rust
use android_nfc::{NfcAdapter, Tag, NdefMessage, NdefRecord};

// leitura de tag (aguarda aproximação)
let tag: Tag = NfcAdapter::wait_for_tag().await?;

// ler NDEF
if let Some(msg) = tag.read_ndef().await? {
    for record in msg.records() {
        println!("tipo={:?} payload={:?}", record.tnf(), record.payload());
    }
}

// escrever NDEF
let msg = NdefMessage::new(vec![NdefRecord::uri("https://example.com")]);
tag.write_ndef(msg).await?;

// emulação de cartão (HCE — Host Card Emulation)
NfcAdapter::emulate_card(AID, |apdu| async move {
    // responde APDUs do terminal NFC
    vec![0x90, 0x00]
}).await;
```

**Permissões:** `NFC`

---

### `android-camera`

Baseado na Camera2 API (Android 5+), a mais flexível disponível sem deps externas:

```rust
use android_camera::{Camera, CameraFacing, ImageFormat, Resolution};

// abrir câmera
let camera = Camera::open(CameraFacing::Back).await?;

// stream de preview
let preview: impl Stream<Item = Frame> = camera
    .preview(Resolution::Hd720p)
    .await?;

// captura de foto
let jpeg: Vec<u8> = camera.capture_jpeg(Quality(85)).await?;

// gravação de vídeo
camera.record_mp4("/sdcard/video.mp4", Duration::from_secs(10)).await?;

// controles
camera.set_flash(FlashMode::Auto);
camera.set_zoom(2.0);
camera.set_focus(FocusMode::ContinuousVideo);

// scan de QR/barcode (sem deps externas, usando ML Kit via JNI)
let qr: String = camera.scan_qr().await?;
```

**Permissões:** `CAMERA`, `RECORD_AUDIO` (para vídeo com som),
`WRITE_EXTERNAL_STORAGE` (para salvar em /sdcard)

---

### `android-audio`

```rust
use android_audio::{Microphone, Speaker, SampleRate, ChannelConfig};

// gravação
let mic = Microphone::open(SampleRate::Hz44100, ChannelConfig::Mono)?;
let mut stream: impl Stream<Item = Vec<i16>> = mic.record();

// playback
let speaker = Speaker::open(SampleRate::Hz44100, ChannelConfig::Stereo)?;
speaker.play(&audio_data).await?;

// reconhecimento de fala (via SpeechRecognizer JNI)
let texto: String = android_audio::recognize_speech(Language::PtBr).await?;

// controle de volume por categoria
android_audio::set_volume(AudioStream::Media, 0.8);
android_audio::set_volume(AudioStream::Alarm, 1.0);
```

**Permissões:** `RECORD_AUDIO`, `MODIFY_AUDIO_SETTINGS`

---

### `android-notifications`

```rust
use android_notifications::{Notification, Channel, Priority, Action};

// criar canal (obrigatório Android 8+)
Channel::new("alertas", "Alertas do App")
    .importance(Priority::High)
    .register();

// notificação simples
Notification::new("alertas")
    .title("Título")
    .body("Corpo da notificação")
    .icon(R::drawable::ic_notification)
    .show(id: 1);

// notificação com progresso
Notification::new("downloads")
    .title("Baixando arquivo...")
    .progress(current: 45, total: 100)
    .ongoing(true)
    .show(id: 2);

// notificação com ações
Notification::new("mensagens")
    .title("Nova mensagem")
    .body("Olá!")
    .action(Action::new("reply", "Responder"))
    .action(Action::new("dismiss", "Dispensar"))
    .on_action(|action_id| async move {
        if action_id == "reply" { /* ... */ }
    })
    .show(id: 3);
```

**Permissões:** `POST_NOTIFICATIONS` (Android 13+)

---

### `android-haptics`

```rust
use android_haptics::{Vibrator, Effect, Waveform};

// efeito predefinido
Vibrator::play(Effect::Click);
Vibrator::play(Effect::HeavyClick);
Vibrator::play(Effect::DoubleClick);

// duração simples
Vibrator::buzz(Duration::from_millis(200));

// padrão personalizado: vibrar, pausar, vibrar
Vibrator::pattern(Waveform::new()
    .on(100)
    .off(50)
    .on(200));

// intensidade variável (requer hardware compatível)
Vibrator::waveform(Waveform::new()
    .step(duration: 100, amplitude: 50)
    .step(duration: 100, amplitude: 255));
```

**Permissões:** `VIBRATE`

---

### `android-biometrics`

```rust
use android_biometrics::{BiometricPrompt, Authenticator, AuthResult};

// autenticação com impressão digital ou rosto
let result: AuthResult = BiometricPrompt::new()
    .title("Confirmar identidade")
    .subtitle("Use sua digital ou rosto")
    .authenticators(Authenticator::Biometric | Authenticator::DeviceCredential)
    .authenticate()
    .await?;

match result {
    AuthResult::Success(crypto_object) => { /* prosseguir */ }
    AuthResult::Failed => { /* biometria não reconhecida */ }
    AuthResult::Error(e) => { /* lockout, hardware indisponível etc. */ }
}
```

**Permissões:** `USE_BIOMETRIC`, `USE_FINGERPRINT` (legado)

---

### `android-storage`

```rust
use android_storage::{FilePicker, MediaStore, FileType};

// abrir seletor de arquivo (SAF)
let uri = FilePicker::pick(FileType::Image).await?;
let bytes: Vec<u8> = android_storage::read_uri(&uri).await?;

// salvar na galeria
MediaStore::save_image(&jpeg_bytes, "foto.jpg").await?;
MediaStore::save_video(&mp4_bytes, "video.mp4").await?;

// app-private storage (sem permissão)
let path = android_storage::app_dir().join("dados.bin");
tokio::fs::write(&path, &data).await?;
```

**Permissões:** `READ_MEDIA_IMAGES`, `READ_MEDIA_VIDEO` (Android 13+);
`READ_EXTERNAL_STORAGE` (Android < 13)

---

### `android-battery`

```rust
use android_battery::{BatteryManager, BatteryState};

// leitura única
let state: BatteryState = BatteryManager::current();
println!("nível={}% carregando={}", state.level_percent(), state.is_charging());

// stream de mudanças
let mut stream = BatteryManager::stream();
while let Some(state) = stream.next().await {
    println!("temperatura={:.1}°C tensão={}mV", state.temperature_c(), state.voltage_mv());
}
```

**Permissões:** nenhuma

---

### `android-display`

```rust
use android_display::{Display, WakeLock, Orientation};

// brilho (0.0 a 1.0, ou Auto)
Display::set_brightness(0.5);
Display::set_brightness(Brightness::Auto);

// manter tela ligada enquanto o guard existir
let _lock = WakeLock::acquire(WakeLockType::ScreenBright);

// orientação
Display::set_orientation(Orientation::Landscape);
Display::set_orientation(Orientation::SensorPortrait);

// informações
let info = Display::info(); // resolução, densidade, taxa de atualização
```

**Permissões:** `WAKE_LOCK`, `WRITE_SETTINGS` (para brilho)

---

### `android-network`

```rust
use android_network::{ConnectivityManager, NetworkState, NetworkType};

// estado atual
let state: NetworkState = ConnectivityManager::current();
println!("online={} tipo={:?}", state.is_connected(), state.network_type());

// stream de mudanças de conectividade
let mut stream = ConnectivityManager::stream();
while let Some(state) = stream.next().await {
    match state.network_type() {
        NetworkType::Wifi => println!("conectado ao WiFi"),
        NetworkType::Cellular => println!("dados móveis"),
        NetworkType::None => println!("sem conexão"),
        _ => {}
    }
}
```

**Permissões:** `ACCESS_NETWORK_STATE`, `CHANGE_NETWORK_STATE`

---

### `android-usb`

Para apps que se comunicam com hardware via USB (modo host — o Android age
como controlador USB):

```rust
use android_usb::{UsbManager, UsbDevice, TransferType};

// listar dispositivos conectados
let devices: Vec<UsbDevice> = UsbManager::devices();

// solicitar permissão e abrir
let device = &devices[0];
let handle = UsbManager::open(device).await?;

// transferência bulk (ex: impressora, Arduino)
let interface = handle.interface(0)?;
let endpoint = interface.bulk_out_endpoint()?;
handle.bulk_transfer(endpoint, &data, timeout).await?;

// transferência interrupt (ex: HID)
let endpoint = interface.interrupt_in_endpoint()?;let response = handle.interrupt_transfer(endpoint, timeout).await?;
```

**Permissões:** declaração de `<uses-feature android:name="android.hardware.usb.host"/>`
no manifesto; permissão concedida via diálogo do sistema em tempo de execução.

---

## Camada 3 — Widgets iced

Este crate fornece componentes visuais prontos para uso com `iced`, construídos
sobre os streams da camada 2. O objetivo é que um dev possa adicionar uma
visualização de câmera ao app com 3 linhas de código.

### `iced-android-widgets`

```rust
use iced_android_widgets::{
    CameraPreview, SensorGraph, BleScanner,
    PermissionGate, MapTiles, BiometricLock,
};

// preview de câmera como widget iced
CameraPreview::new()
    .facing(CameraFacing::Back)
    .resolution(Resolution::Hd720p)

// gráfico em tempo real de sensor
SensorGraph::new(Sensor::Accelerometer)
    .window(Duration::from_secs(5))
    .axes(&[Axis::X, Axis::Y, Axis::Z])
    .color_scheme(ColorScheme::Default)

// scanner BLE com lista de dispositivos
BleScanner::new()
    .filter(ScanFilter::by_service(MY_UUID))
    .on_connect(|device| Message::DeviceConnected(device))

// wrapper que pede permissão antes de renderizar o filho
PermissionGate::new(Permission::Camera)
    .child(CameraPreview::new())
    .fallback(text("Permissão de câmera necessária"))

// mapa offline com tiles OpenStreetMap
MapTiles::new()
    .center(Location { latitude: -23.5505, longitude: -46.6333 })
    .zoom(14)
    .marker(poi_location)

// tela de autenticação biométrica
BiometricLock::new()
    .title("Acesso restrito")
    .on_success(Message::Authenticated)
```

---

## Camada 4 — Dev Tooling

### `android-dev-tools` (targets Makefile)

```
make watch          ← cargo-watch: rebuild + reinstall automático ao salvar
make logcat         ← logcat filtrado por PID do app, saída colorida por nível
make logcat-crash   ← filtra apenas fatais e panic, simboliza endereços
make screenshot     ← salva PNG com timestamp em ./screenshots/
make record         ← grava screenrecord MP4 até Ctrl-C
make profile        ← simpleperf on-device → flamegraph SVG local
make symbolize      ← lê crash log da stdin, resolve endereços para linhas Rust
make apk-info       ← exibe permissões, activities e metadados do APK compilado
make wifi-adb       ← configura ADB over TCP/IP (sem cabo)
```

### `cargo-template` — gerador de projetos

```bash
cargo generate --git https://github.com/antoniofernandodj/rust-android-sdk \
    --name meu-app
```

O template interativo pergunta quais periféricos o projeto vai usar e:

- adiciona os crates corretos ao `Cargo.toml`
- declara as permissões necessárias no `AndroidManifest.xml`
- gera código boilerplate de inicialização
- cria exemplos mínimos para cada periférico escolhido

---

## Ordem de implementação sugerida

A dependência entre as camadas define a sequência natural:

```
Fase 1 (fundação)
  └── android-jni-bridge      ← proc-macro; desbloqueia todo o resto
  └── android-permissions     ← necessário para qualquer periférico

Fase 2 (periféricos sem hardware especializado)
  └── android-battery         ← mais simples; zero permissões; ótimo para validar o padrão
  └── android-network         ← também simples; sem permissões especiais
  └── android-sensors         ← nenhuma permissão para sensores de movimento
  └── android-haptics         ← só VIBRATE; feedback imediato de que funciona
  └── android-notifications   ← alta utilidade; POST_NOTIFICATIONS apenas

Fase 3 (periféricos de localização e conectividade)
  └── android-location        ← GPS + geofencing
  └── android-bluetooth       ← BLE primeiro, clássico depois
  └── android-wifi
  └── android-nfc

Fase 4 (mídia)
  └── android-audio           ← microfone e playback
  └── android-camera          ← mais complexo; Camera2 tem muitos estados

Fase 5 (sistema)
  └── android-storage         ← SAF + MediaStore
  └── android-display
  └── android-biometrics
  └── android-usb

Fase 6 (experiência de desenvolvimento)
  └── iced-android-widgets    ← depende de sensores + câmera + BLE
  └── android-dev-tools       ← Makefile targets + scripts
  └── cargo-template          ← fecha o ciclo; novo projeto em minutos
```

---

## Princípios de design

**1. Async-first.** Todo dado periódico é um `Stream<T>`. Toda operação única é
um `Future<T>`. Nenhuma API de callback Java vaza para o código Rust do app.

**2. Permissões explícitas.** Cada função que requer permissão recebe
`PermissionGuard` como parâmetro ou falha com `Error::PermissionDenied` se a
permissão não foi concedida. Não há verificação implícita silenciosa.

**3. Feature flags por crate.** Cada periférico é uma feature opcional.
O app final só linka o código do que realmente usa:

```toml
[dependencies]
rust-android-sdk = { version = "0.1", features = ["camera", "ble", "sensors"] }
```

**4. Desktop stubs.** Cada crate de periférico tem uma implementação stub para
`cfg(not(target_os = "android"))` que permite continuar usando `cargo run -p
desktop` durante o desenvolvimento, com dados simulados ou erros explícitos.

**5. Sem Java adicional.** Todo acesso ao Android é feito via JNI puro. Não há
Activity Kotlin/Java separada, não há Gradle, não há AAR. O `NativeActivity`
existente é suficiente.

---

## Estado atual vs. meta

| | Hoje | Meta |
|---|---|---|
| UI | iced funcionando | iced + widgets de periféricos |
| Sensores | nenhum | 10+ sensores como Stream |
| Câmera | nenhum | Camera2 completa |
| Bluetooth | nenhum | BLE + Clássico |
| GPS | nenhum | FusedLocation + Geofencing |
| Notificações | nenhum | Canais, ações, progresso |
| JNI | manual/verboso | proc-macro com `#[jni_call]` |
| Permissões | nenhum | API async unificada |
| Dev tooling | `make build/run` básico | watch, logcat, profile, symbolize |
| Novo projeto | clone manual | `cargo generate` em 2 minutos |
