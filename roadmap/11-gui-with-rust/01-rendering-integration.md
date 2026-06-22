# Rendering Integration

> **Seção:** GUI with Rust
> **Status:** ✅ preenchido

## Visão geral

Além de usar Rust só para a lógica, é possível **renderizar a própria UI em Rust**, útil para
jogos, apps gráficos ou interfaces custom (egui, Bevy, Slint, Skia). Nesse modelo o Android
fornece apenas uma superfície (`SurfaceView`/`NativeActivity`) e o Rust desenha diretamente
via GPU. Isso exige integrar o pipeline gráfico, o **event loop** e a propagação de input
entre o sistema Android e o renderer nativo.

## Subtópicos

### WGPU backend implementation

- **`wgpu`** é a abstração gráfica moderna em Rust (sobre Vulkan/GLES no Android), segura e
  multiplataforma. É a base de engines e toolkits de UI (egui, Bevy).
- No Android, o `wgpu` cria uma `Surface` a partir do `ANativeWindow` obtido da
  `SurfaceView`/`NativeActivity`. Use `raw-window-handle` para fazer essa ponte.
- Prefira Vulkan onde disponível; o `wgpu` faz fallback para GLES em aparelhos antigos.

### Skia graphics engine usage

- **Skia** é o motor 2D que o próprio Android/Chrome usa; em Rust acessível via
  `skia-safe`. Bom para UI 2D vetorial, texto e shapes com qualidade.
- Alternativa ao 3D do wgpu quando o foco é desenho 2D de alta fidelidade. Pode renderizar
  para uma surface GPU ou bitmap.

### Canvas native bridge

- A ponte com o Android se dá por **`NativeActivity`** (app 100% nativo, sem Java) ou por uma
  **`SurfaceView`/`TextureView`** embutida numa Activity Kotlin, passando o handle de janela
  ao Rust via JNI.
- `ndk-glue`/crate `ndk` expõem o `ANativeWindow` e os callbacks de ciclo de vida da surface
  (criada/redimensionada/destruída) que o renderer precisa respeitar.

### Event loop synchronization

- O renderer roda em um **event loop** (ex.: `winit` no Android, ou o loop da
  `NativeActivity`) que recebe eventos do sistema e dispara frames.
- Sincronize o desenho com o **VSync**/ritmo da surface para evitar tearing e desperdício;
  só renderize quando há mudança ou quando o sistema pede `redraw`. Pause o loop quando a
  Activity vai a background (surface destruída).

### Input handling propagation

- Toques, teclado e gestos chegam como **`InputEvent`** do Android e precisam ser
  encaminhados ao renderer Rust (via `ndk`/`winit`), convertidos em eventos da UI nativa.
- Cuide da conversão de coordenadas (densidade de tela) e do roteamento de foco. Se houver
  UI Kotlin sobreposta, defina claramente quem consome cada evento para evitar conflito.

### Frame buffer rendering

- O ciclo de frame: adquirir a próxima imagem da swapchain → desenhar a cena → apresentar
  (`present`). Com `wgpu`, isso é `surface.get_current_texture()` → render pass → `present()`.
- Gerencie corretamente o **redimensionamento** (reconfigurar a surface) e a perda de
  contexto. Faça o trabalho de render fora da UI thread quando possível e respeite o
  *back-pressure* do compositor para manter framerate estável.

## Referências

- wgpu: https://wgpu.rs/
- The `ndk` crate (Android): https://docs.rs/ndk
- winit (Android support): https://github.com/rust-windowing/winit
