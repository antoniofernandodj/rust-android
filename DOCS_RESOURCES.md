# Recursos para Estudo: Android, Drivers e Sistemas Operacionais

Este documento contém referências curadas para aprofundar o entendimento sobre o funcionamento interno do Android, gerenciamento de memória, drivers de GPU e a interação com linguagens de sistemas como Rust.

## 1. Arquitetura do Android e Kernel
*   **Livro: "Android Internals: A Conclusive Guide" (Jonathan Levin)**: A "bíblia" para entender o que acontece abaixo da superfície do Android, desde o boot até o gerenciamento de processos.
*   **Livro: "Embedded Android" (Karim Yaghmour)**: Focado em como o Android é construído e como ele interage com o hardware.
*   **Documentação Oficial: [Android Open Source Project (AOSP)](https://source.android.com/)**: O melhor lugar para entender a HAL (Hardware Abstraction Layer) e como os drivers são estruturados no Android.

## 2. Drivers de Vídeo e Computação Gráfica (GPU)
*   **Artigo: "Vulkan in 30 Minutes" (Dustin Land)**: Excelente para entender por que as APIs modernas (como a que o `wgpu` usa) são explícitas e por que inicializá-las consome memória.
*   **Blog: [Igalia Graphics](https://blogs.igalia.com/)**: Frequentemente publicam artigos técnicos profundos sobre drivers de GPU open-source (Mesa, Turnip, Freedreno), que são a base do que roda em muitos dispositivos Android.
*   **Livro: "Real-Time Rendering" (Tomas Akenine-Möller et al.)**: Essencial para entender como a memória de vídeo e os buffers funcionam, explicando o custo fixo de memória que vimos no app.

## 3. Gerenciamento de Memória em Sistemas Operacionais
*   **Livro: "Modern Operating Systems" (Andrew S. Tanenbaum)**: Conceitos fundamentais de Memória Virtual vs. Memória Real (RSS).
*   **Artigo: [Memory Management in Android](https://developer.android.com/topic/performance/memory)**: Explica como o Android decide quando matar processos e como ele lida com o compartilhamento de páginas de memória entre o Zygote e os apps.

## 4. Rust no Android
*   **[The Rust on Android Integrators Guide](https://source.android.com/docs/setup/build/rust/building-rust-modules)**: Como o Google está integrando Rust no sistema operacional para substituir C++.
*   **[wgpu Documentation](https://wgpu.rs/)**: Documentação da biblioteca usada pelo `iced`, detalhando como ela mapeia conceitos nativos de GPU para o código Rust.

## 5. Ferramentas de Análise
*   **Perfetto**: A ferramenta de rastreamento de performance padrão do Android.
*   **ADB (Android Debug Bridge) + `procrank`**: Para uma visão detalhada de PSS (Proportional Set Size), que é uma métrica de memória mais precisa que o RSS para apps que compartilham drivers.
