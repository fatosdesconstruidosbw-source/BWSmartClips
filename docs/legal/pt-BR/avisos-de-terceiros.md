# Avisos de licenças de terceiros

O BWSmartClips é software proprietário. Ele usa os componentes de código aberto abaixo, cada um sob sua própria licença. Os textos completos das licenças acompanham o app na pasta `licenses` (e em `docs/legal/licencas` no código).

Para componentes sob licenças que dão direitos ao usuário (por exemplo GPL e MPL), esses direitos continuam valendo integralmente.

## Ferramentas incluídas ou baixadas pelo app

| Ferramenta | Licença | Edição | Observação |
|---|---|---|---|
| FFmpeg (build "essentials" de gyan.dev) | GPL-3.0-or-later | Store e direta | Programa separado, executado pelo app. Código-fonte correspondente: https://ffmpeg.org/download.html e configuração da build em https://www.gyan.dev/ffmpeg/builds/ ; também enviamos uma cópia do código-fonte da versão incluída a quem pedir por e-mail, por 3 anos. |
| whisper.cpp (ggml-org) | MIT | Store e direta | https://github.com/ggml-org/whisper.cpp |
| llama.cpp e ggml (ggml-org), CPU e Vulkan | MIT | Store e direta | https://github.com/ggml-org/llama.cpp |
| yt-dlp | Unlicense | somente direta (download por link) | https://github.com/yt-dlp/yt-dlp |
| Deno | MIT | somente direta (download por link) | https://github.com/denoland/deno |
| bgutil-ytdlp-pot-provider-rs | conferir na fonte (projeto da comunidade) | somente direta (download por link) | https://github.com/jim60105/bgutil-ytdlp-pot-provider-rs — não incluído na edição Microsoft Store. |
| Fonte Montserrat ExtraBold | OFL-1.1 | Store e direta | Ver Montserrat-OFL.txt |
| Modelo de detecção de rosto SeetaFace (via rustface) | BSD-2-Clause | Store e direta | Ver SeetaFace-BSD-2-Clause.txt |

## Modelos de IA (baixados a seu pedido, não fazem parte do pacote)

| Modelo | Licença | Origem |
|---|---|---|
| Qwen3.5 2B / 4B / 9B (Alibaba Qwen), quantizações GGUF de bartowski | Apache-2.0 | https://huggingface.co/Qwen |
| Whisper (OpenAI), conversões ggml de whisper.cpp | MIT | https://huggingface.co/ggerganov/whisper.cpp |
| Silero VAD (conversão ggml) | MIT | https://github.com/snakers4/silero-vad |

## Bibliotecas compiladas no app (470)

Bibliotecas Rust (crates.io). Quando há várias licenças separadas por OR, o componente é usado sob a licença permissiva (MIT ou Apache-2.0). Componentes MPL-2.0 são usados sem modificação; o código-fonte deles está em https://crates.io.

| Componente | Versão | Licença |
|---|---|---|
| adler2 | 2.0.1 | 0BSD OR Apache-2.0 OR MIT |
| aho-corasick | 1.1.5 | MIT OR Unlicense |
| alloc-no-stdlib | 3.0.0 | BSD-3-Clause |
| alloc-stdlib | 0.3.0 | BSD-3-Clause |
| android_system_properties | 0.1.6 | Apache-2.0 OR MIT |
| anstream | 1.0.0 | Apache-2.0 OR MIT |
| anstyle | 1.0.14 | Apache-2.0 OR MIT |
| anstyle-parse | 1.0.0 | Apache-2.0 OR MIT |
| anstyle-query | 1.1.5 | Apache-2.0 OR MIT |
| anstyle-wincon | 3.0.11 | Apache-2.0 OR MIT |
| anyhow | 1.0.105 | Apache-2.0 OR MIT |
| arbitrary | 1.5.0 | Apache-2.0 OR MIT |
| async-broadcast | 0.7.2 | Apache-2.0 OR MIT |
| async-channel | 2.5.0 | Apache-2.0 OR MIT |
| async-executor | 1.14.1 | Apache-2.0 OR MIT |
| async-io | 2.6.0 | Apache-2.0 OR MIT |
| async-lock | 3.5.0 | Apache-2.0 OR MIT |
| async-process | 2.5.0 | Apache-2.0 OR MIT |
| async-recursion | 1.2.0 | Apache-2.0 OR MIT |
| async-signal | 0.2.14 | Apache-2.0 OR MIT |
| async-task | 4.7.1 | Apache-2.0 OR MIT |
| async-trait | 0.1.92 | Apache-2.0 OR MIT |
| atk | 0.18.2 | MIT |
| atk-sys | 0.18.2 | MIT |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT |
| aws-lc-rs | 1.19.0 | (Apache-2.0 OR ISC) AND ISC |
| aws-lc-sys | 0.47.0 | (Apache-2.0 OR ISC OR MIT) AND (Apache-2.0 OR ISC OR MIT-0) AND (Apache-2.0 OR ISC) AND Apache-2.0 AND BSD-3-Clause AND ISC AND MIT |
| base64 | 0.21.7, 0.23.1 | Apache-2.0 OR MIT |
| bit-set | 0.11.1, 0.8.0 | Apache-2.0 OR MIT |
| bit-vec | 0.10.2, 0.8.0 | Apache-2.0 OR MIT |
| bitflags | 1.3.2, 2.13.2 | Apache-2.0 OR MIT |
| block-buffer | 0.10.4, 0.12.1 | Apache-2.0 OR MIT |
| block2 | 0.6.2 | MIT |
| blocking | 1.7.0 | Apache-2.0 OR MIT |
| brotli | 9.1.3 | BSD-3-Clause AND MIT |
| brotli-decompressor | 6.4.0 | BSD-3-Clause OR MIT |
| bs58 | 0.5.1 | Apache-2.0 OR MIT |
| bumpalo | 3.20.4 | Apache-2.0 OR MIT |
| bytemuck | 1.25.2 | Apache-2.0 OR MIT OR Zlib |
| byteorder | 1.5.0 | MIT OR Unlicense |
| bytes | 1.12.2 | MIT |
| cairo-rs | 0.18.5 | MIT |
| cairo-sys-rs | 0.18.2 | MIT |
| camino | 1.2.6 | Apache-2.0 OR MIT |
| cargo-platform | 0.1.9 | Apache-2.0 OR MIT |
| cargo_metadata | 0.19.2 | MIT |
| cesu8 | 1.1.0 | Apache-2.0 OR MIT |
| cfb | 0.14.0 | MIT |
| cfg-if | 1.0.5 | Apache-2.0 OR MIT |
| chacha20 | 0.10.2 | Apache-2.0 OR MIT |
| chrono | 0.4.45 | Apache-2.0 OR MIT |
| clap | 4.6.10 | Apache-2.0 OR MIT |
| clap_builder | 4.6.10 | Apache-2.0 OR MIT |
| clap_derive | 4.6.9 | Apache-2.0 OR MIT |
| clap_lex | 1.1.2 | Apache-2.0 OR MIT |
| colorchoice | 1.0.5 | Apache-2.0 OR MIT |
| combine | 4.6.9 | MIT |
| concurrent-queue | 2.5.0 | Apache-2.0 OR MIT |
| const-oid | 0.10.3 | Apache-2.0 OR MIT |
| cookie | 0.18.3 | Apache-2.0 OR MIT |
| core-foundation | 0.10.1 | Apache-2.0 OR MIT |
| core-foundation-sys | 0.8.7 | Apache-2.0 OR MIT |
| core-graphics | 0.25.0 | Apache-2.0 OR MIT |
| core-graphics-types | 0.2.0 | Apache-2.0 OR MIT |
| cpufeatures | 0.2.17, 0.3.1 | Apache-2.0 OR MIT |
| crc32fast | 1.5.6 | Apache-2.0 OR MIT |
| crossbeam-channel | 0.5.17 | Apache-2.0 OR MIT |
| crossbeam-deque | 0.8.8 | Apache-2.0 OR MIT |
| crossbeam-epoch | 0.9.21 | Apache-2.0 OR MIT |
| crossbeam-utils | 0.8.23 | Apache-2.0 OR MIT |
| crypto-common | 0.1.7, 0.2.2 | Apache-2.0 OR MIT |
| cssparser | 0.37.0, 0.38.0 | MPL-2.0 |
| cssparser-macros | 0.7.1 | MPL-2.0 |
| ctor | 1.0.14 | Apache-2.0 OR MIT |
| darling | 0.24.1 | MIT |
| darling_core | 0.24.1 | MIT |
| darling_macro | 0.24.1 | MIT |
| dbus | 0.9.12 | Apache-2.0 OR MIT |
| deranged | 0.5.9 | Apache-2.0 OR MIT |
| derive_arbitrary | 1.5.0 | Apache-2.0 OR MIT |
| derive_more | 2.1.1 | MIT |
| derive_more-impl | 2.1.1 | MIT |
| digest | 0.10.7, 0.11.3 | Apache-2.0 OR MIT |
| directories | 6.0.0 | Apache-2.0 OR MIT |
| dirs | 7.0.0 | Apache-2.0 OR MIT |
| dirs-sys | 0.5.0 | Apache-2.0 OR MIT |
| dispatch2 | 0.3.1 | Apache-2.0 OR MIT OR Zlib |
| displaydoc | 0.2.7 | Apache-2.0 OR MIT |
| dlopen2 | 0.8.2 | MIT |
| dlopen2_derive | 0.4.3 | MIT |
| dom_query | 0.28.0, 0.30.0 | MIT |
| dpi | 0.1.2 | Apache-2.0 AND MIT |
| dtoa | 1.0.11 | Apache-2.0 OR MIT |
| dtoa-short | 0.3.5 | MPL-2.0 |
| dunce | 1.0.5 | Apache-2.0 OR CC0-1.0 OR MIT-0 |
| dyn-clone | 1.0.20 | Apache-2.0 OR MIT |
| either | 1.19.0 | Apache-2.0 OR MIT |
| embed_plist | 1.2.2 | Apache-2.0 OR MIT |
| endi | 1.1.1 | MIT |
| enumflags2 | 0.7.12 | Apache-2.0 OR MIT |
| enumflags2_derive | 0.7.12 | Apache-2.0 OR MIT |
| equivalent | 1.0.3 | Apache-2.0 OR MIT |
| erased-serde | 0.4.11 | Apache-2.0 OR MIT |
| errno | 0.3.15 | Apache-2.0 OR MIT |
| event-listener | 5.4.2 | Apache-2.0 OR MIT |
| event-listener-strategy | 0.5.4 | Apache-2.0 OR MIT |
| fastrand | 2.5.0 | Apache-2.0 OR MIT |
| fdeflate | 0.3.7 | Apache-2.0 OR MIT |
| field-offset | 0.3.6 | Apache-2.0 OR MIT |
| flate2 | 1.1.11 | Apache-2.0 OR MIT |
| fnv | 1.0.7 | Apache-2.0 OR MIT |
| foldhash | 0.2.0 | Zlib |
| foreign-types | 0.5.0 | Apache-2.0 OR MIT |
| foreign-types-macros | 0.2.4 | Apache-2.0 OR MIT |
| foreign-types-shared | 0.3.1 | Apache-2.0 OR MIT |
| form_urlencoded | 1.2.3 | Apache-2.0 OR MIT |
| fs4 | 0.13.1 | Apache-2.0 OR MIT |
| futures-channel | 0.3.34 | Apache-2.0 OR MIT |
| futures-core | 0.3.34 | Apache-2.0 OR MIT |
| futures-executor | 0.3.34 | Apache-2.0 OR MIT |
| futures-io | 0.3.34 | Apache-2.0 OR MIT |
| futures-lite | 2.6.1 | Apache-2.0 OR MIT |
| futures-macro | 0.3.34 | Apache-2.0 OR MIT |
| futures-sink | 0.3.34 | Apache-2.0 OR MIT |
| futures-task | 0.3.34 | Apache-2.0 OR MIT |
| futures-util | 0.3.34 | Apache-2.0 OR MIT |
| gdk | 0.18.2 | MIT |
| gdk-pixbuf | 0.18.5 | MIT |
| gdk-pixbuf-sys | 0.18.0 | MIT |
| gdk-sys | 0.18.2 | MIT |
| gdkwayland-sys | 0.18.2 | MIT |
| gdkx11 | 0.18.2 | MIT |
| gdkx11-sys | 0.18.2 | MIT |
| generic-array | 0.14.7 | MIT |
| getrandom | 0.2.17, 0.3.4, 0.4.3 | Apache-2.0 OR MIT |
| gio | 0.18.4 | MIT |
| gio-sys | 0.18.1 | MIT |
| glib | 0.18.5 | MIT |
| glib-macros | 0.18.5 | MIT |
| glib-sys | 0.18.1 | MIT |
| glob | 0.3.4 | Apache-2.0 OR MIT |
| gobject-sys | 0.18.0 | MIT |
| gtk | 0.18.2 | MIT |
| gtk-sys | 0.18.2 | MIT |
| gtk3-macros | 0.18.2 | MIT |
| hashbrown | 0.12.3, 0.17.2 | Apache-2.0 OR MIT |
| heck | 0.4.1, 0.5.0 | Apache-2.0 OR MIT |
| hermit-abi | 0.5.3 | Apache-2.0 OR MIT |
| hex | 0.4.3 | Apache-2.0 OR MIT |
| html5ever | 0.39.0, 0.40.1 | Apache-2.0 OR MIT |
| http | 1.5.0 | Apache-2.0 OR MIT |
| http-body | 1.1.0 | MIT |
| http-body-util | 0.1.5 | MIT |
| http-range | 0.1.6 | MIT |
| httparse | 1.10.1 | Apache-2.0 OR MIT |
| hybrid-array | 0.4.15 | Apache-2.0 OR MIT |
| hyper | 1.13.0 | MIT |
| hyper-rustls | 0.27.10 | Apache-2.0 OR ISC OR MIT |
| hyper-util | 0.1.21 | MIT |
| iana-time-zone | 0.1.65 | Apache-2.0 OR MIT |
| iana-time-zone-haiku | 0.1.2 | Apache-2.0 OR MIT |
| ico | 0.6.0 | MIT |
| icu_collections | 2.4.0 | Unicode-3.0 |
| icu_locale_core | 2.4.0 | Unicode-3.0 |
| icu_normalizer | 2.4.0 | Unicode-3.0 |
| icu_normalizer_data | 2.4.0 | Unicode-3.0 |
| icu_properties | 2.4.0 | Unicode-3.0 |
| icu_properties_data | 2.4.0 | Unicode-3.0 |
| icu_provider | 2.4.0 | Unicode-3.0 |
| ident_case | 1.0.1 | Apache-2.0 OR MIT |
| idna | 1.1.0 | Apache-2.0 OR MIT |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT |
| indexmap | 1.9.3, 2.14.2 | Apache-2.0 OR MIT |
| infer | 0.22.0 | MIT |
| ipnet | 2.12.2 | Apache-2.0 OR MIT |
| is-docker | 0.2.0 | MIT |
| is-wsl | 0.4.0 | MIT |
| is_terminal_polyfill | 1.70.2 | Apache-2.0 OR MIT |
| itoa | 1.0.19 | Apache-2.0 OR MIT |
| javascriptcore-rs | 1.1.2 | MIT |
| javascriptcore-rs-sys | 1.1.1 | MIT |
| jiff | 0.2.46, 1.0.3 | MIT OR Unlicense |
| jiff-core | 0.1.8, 1.0.2 | MIT OR Unlicense |
| jiff-static | 0.2.46, 1.0.3 | MIT OR Unlicense |
| jiff-tzdb | 0.1.10, 1.0.0 | MIT OR Unlicense |
| jiff-tzdb-platform | 0.1.3, 1.0.0 | MIT OR Unlicense |
| jni | 0.21.1, 0.22.4 | Apache-2.0 OR MIT |
| jni-macros | 0.22.4 | Apache-2.0 OR MIT |
| jni-sys | 0.3.1, 0.4.1 | Apache-2.0 OR MIT |
| jni-sys-macros | 0.4.1 | Apache-2.0 OR MIT |
| js-sys | 0.3.107 | Apache-2.0 OR MIT |
| json-patch | 4.2.0 | Apache-2.0 OR MIT |
| jsonptr | 0.7.1 | Apache-2.0 OR MIT |
| keyboard-types | 0.8.3 | Apache-2.0 OR MIT |
| libappindicator | 0.9.0 | Apache-2.0 OR MIT |
| libappindicator-sys | 0.9.0 | Apache-2.0 OR MIT |
| libc | 0.2.191 | Apache-2.0 OR MIT |
| libdbus-sys | 0.2.7 | Apache-2.0 OR MIT |
| libloading | 0.7.4 | ISC |
| libredox | 0.1.25 | MIT |
| link-section | 0.19.4 | Apache-2.0 OR MIT |
| linux-raw-sys | 0.12.1 | Apache-2.0 OR Apache-2.0 WITH LLVM-exception OR MIT |
| litemap | 0.8.3 | Unicode-3.0 |
| lock_api | 0.4.14 | Apache-2.0 OR MIT |
| log | 0.4.35 | Apache-2.0 OR MIT |
| lru-slab | 0.1.3 | Apache-2.0 OR MIT OR Zlib |
| markup5ever | 0.39.0, 0.40.0 | Apache-2.0 OR MIT |
| memchr | 2.8.3 | MIT OR Unlicense |
| memoffset | 0.9.1 | MIT |
| mime | 0.3.18 | Apache-2.0 OR MIT |
| mime_guess | 2.0.5 | MIT |
| miniz_oxide | 0.8.9, 0.9.1 | Apache-2.0 OR MIT OR Zlib |
| mio | 1.2.5 | MIT |
| muda | 0.20.0 | Apache-2.0 OR MIT |
| ndk | 0.9.0 | Apache-2.0 OR MIT |
| ndk-context | 0.1.1 | Apache-2.0 OR MIT |
| ndk-sys | 0.6.0+11769913 | Apache-2.0 OR MIT |
| new_debug_unreachable | 1.0.6 | MIT |
| num | 0.3.1 | Apache-2.0 OR MIT |
| num-bigint | 0.3.3 | Apache-2.0 OR MIT |
| num-complex | 0.3.1 | Apache-2.0 OR MIT |
| num-conv | 0.2.2 | Apache-2.0 OR MIT |
| num-integer | 0.1.47 | Apache-2.0 OR MIT |
| num-iter | 0.1.46 | Apache-2.0 OR MIT |
| num-rational | 0.3.2 | Apache-2.0 OR MIT |
| num-traits | 0.2.19 | Apache-2.0 OR MIT |
| num_enum | 0.7.6 | Apache-2.0 OR BSD-3-Clause OR MIT |
| num_enum_derive | 0.7.6 | Apache-2.0 OR BSD-3-Clause OR MIT |
| objc2 | 0.6.5 | MIT |
| objc2-app-kit | 0.3.2 | Apache-2.0 OR MIT OR Zlib |
| objc2-cloud-kit | 0.3.2 | Apache-2.0 OR MIT OR Zlib |
| objc2-core-data | 0.3.2 | Apache-2.0 OR MIT OR Zlib |
| objc2-core-foundation | 0.3.2 | Apache-2.0 OR MIT OR Zlib |
| objc2-core-graphics | 0.3.2 | Apache-2.0 OR MIT OR Zlib |
| objc2-core-image | 0.3.2 | Apache-2.0 OR MIT OR Zlib |
| objc2-core-location | 0.3.2 | Apache-2.0 OR MIT OR Zlib |
| objc2-core-text | 0.3.2 | Apache-2.0 OR MIT OR Zlib |
| objc2-encode | 4.1.0 | MIT |
| objc2-exception-helper | 0.1.1 | Apache-2.0 OR MIT OR Zlib |
| objc2-foundation | 0.3.2 | MIT |
| objc2-io-surface | 0.3.2 | Apache-2.0 OR MIT OR Zlib |
| objc2-quartz-core | 0.3.2 | Apache-2.0 OR MIT OR Zlib |
| objc2-ui-kit | 0.3.2 | Apache-2.0 OR MIT OR Zlib |
| objc2-user-notifications | 0.3.2 | Apache-2.0 OR MIT OR Zlib |
| objc2-web-kit | 0.3.2 | Apache-2.0 OR MIT OR Zlib |
| once_cell | 1.21.4 | Apache-2.0 OR MIT |
| once_cell_polyfill | 1.70.2 | Apache-2.0 OR MIT |
| open | 5.4.5 | MIT |
| openssl-probe | 0.2.2 | Apache-2.0 OR MIT |
| option-ext | 0.2.0 | MPL-2.0 |
| ordered-stream | 0.2.0 | Apache-2.0 OR MIT |
| pango | 0.18.3 | MIT |
| pango-sys | 0.18.0 | MIT |
| parking | 2.2.1 | Apache-2.0 OR MIT |
| parking_lot | 0.12.5 | Apache-2.0 OR MIT |
| parking_lot_core | 0.9.12 | Apache-2.0 OR MIT |
| percent-encoding | 2.3.2 | Apache-2.0 OR MIT |
| phf | 0.13.1, 0.14.0 | MIT |
| phf_generator | 0.13.1, 0.14.0 | MIT |
| phf_macros | 0.13.1, 0.14.0 | MIT |
| phf_shared | 0.13.1, 0.14.0 | MIT |
| pin-project-lite | 0.2.18 | Apache-2.0 OR MIT |
| piper | 0.2.5 | Apache-2.0 OR MIT |
| plist | 1.10.4 | MIT |
| png | 0.18.1 | Apache-2.0 OR MIT |
| polling | 3.11.0 | Apache-2.0 OR MIT |
| portable-atomic | 1.15.0 | Apache-2.0 OR MIT |
| portable-atomic-util | 0.2.8 | Apache-2.0 OR MIT |
| potential_utf | 0.1.6 | Unicode-3.0 |
| powerfmt | 0.2.1 | Apache-2.0 OR MIT |
| precomputed-hash | 0.1.1 | MIT |
| proc-macro-crate | 1.3.1, 2.0.2, 3.5.0 | Apache-2.0 OR MIT |
| proc-macro-error | 1.0.4 | Apache-2.0 OR MIT |
| proc-macro-error-attr | 1.0.4 | Apache-2.0 OR MIT |
| proc-macro2 | 1.0.107 | Apache-2.0 OR MIT |
| quick-xml | 0.42.0 | MIT |
| quinn | 0.11.12 | Apache-2.0 OR MIT |
| quinn-proto | 0.11.19 | Apache-2.0 OR MIT |
| quinn-udp | 0.5.16 | Apache-2.0 OR MIT |
| quote | 1.0.47 | Apache-2.0 OR MIT |
| r-efi | 5.3.0, 6.0.0 | Apache-2.0 OR LGPL-2.1-or-later OR MIT |
| rand | 0.10.4 | Apache-2.0 OR MIT |
| rand_core | 0.10.1 | Apache-2.0 OR MIT |
| rand_pcg | 0.10.3 | Apache-2.0 OR MIT |
| raw-window-handle | 0.6.2 | Apache-2.0 OR MIT OR Zlib |
| rayon | 1.12.0 | Apache-2.0 OR MIT |
| rayon-core | 1.13.0 | Apache-2.0 OR MIT |
| redox_syscall | 0.5.18 | MIT |
| redox_users | 0.5.3 | MIT |
| ref-cast | 1.0.27 | Apache-2.0 OR MIT |
| ref-cast-impl | 1.0.27 | Apache-2.0 OR MIT |
| regex | 1.13.2 | Apache-2.0 OR MIT |
| regex-automata | 0.4.19 | Apache-2.0 OR MIT |
| regex-syntax | 0.8.12 | Apache-2.0 OR MIT |
| reqwest | 0.13.5 | Apache-2.0 OR MIT |
| rfd | 0.17.2 | MIT |
| ring | 0.17.14 | Apache-2.0 AND ISC |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT |
| rustface | 0.1.7 | BSD-2-Clause |
| rustix | 1.1.6 | Apache-2.0 OR Apache-2.0 WITH LLVM-exception OR MIT |
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT |
| rustls-native-certs | 0.8.5 | Apache-2.0 OR ISC OR MIT |
| rustls-pki-types | 1.16.0 | Apache-2.0 OR MIT |
| rustls-platform-verifier | 0.7.1 | Apache-2.0 OR MIT |
| rustls-platform-verifier-android | 0.2.0 | Apache-2.0 OR MIT |
| rustls-webpki | 0.103.17 | ISC |
| rustversion | 1.0.23 | Apache-2.0 OR MIT |
| same-file | 1.0.6 | MIT OR Unlicense |
| schannel | 0.1.30 | MIT |
| schemars | 0.8.22, 0.9.0, 1.2.3 | MIT |
| schemars_derive | 0.8.22 | MIT |
| scopeguard | 1.2.0 | Apache-2.0 OR MIT |
| security-framework | 3.7.0 | Apache-2.0 OR MIT |
| security-framework-sys | 2.17.0 | Apache-2.0 OR MIT |
| selectors | 0.38.0, 0.41.0 | MPL-2.0 |
| semver | 1.0.28 | Apache-2.0 OR MIT |
| serde | 1.0.229 | Apache-2.0 OR MIT |
| serde-untagged | 0.1.9 | Apache-2.0 OR MIT |
| serde_core | 1.0.229 | Apache-2.0 OR MIT |
| serde_derive | 1.0.229 | Apache-2.0 OR MIT |
| serde_derive_internals | 0.29.1 | Apache-2.0 OR MIT |
| serde_json | 1.0.154 | Apache-2.0 OR MIT |
| serde_repr | 0.1.21 | Apache-2.0 OR MIT |
| serde_spanned | 0.6.9, 1.1.3 | Apache-2.0 OR MIT |
| serde_with | 3.25.0 | Apache-2.0 OR MIT |
| serde_with_macros | 3.25.0 | Apache-2.0 OR MIT |
| serialize-to-javascript | 0.1.2 | Apache-2.0 OR MIT |
| serialize-to-javascript-impl | 0.1.2 | Apache-2.0 OR MIT |
| servo_arc | 0.4.3, 0.5.0 | Apache-2.0 OR MIT |
| sha2 | 0.10.9, 0.11.2 | Apache-2.0 OR MIT |
| signal-hook-registry | 1.4.8 | Apache-2.0 OR MIT |
| simd-adler32 | 0.3.10 | MIT |
| simd_cesu8 | 1.2.0 | Apache-2.0 OR MIT |
| simdutf8 | 0.1.5 | Apache-2.0 OR MIT |
| siphasher | 1.0.4 | Apache-2.0 OR MIT |
| slab | 0.4.12 | MIT |
| smallvec | 1.16.6 | Apache-2.0 OR MIT |
| socket2 | 0.6.5 | Apache-2.0 OR MIT |
| softbuffer | 0.4.8 | Apache-2.0 OR MIT |
| soup3 | 0.5.0 | MIT |
| soup3-sys | 0.5.0 | MIT |
| stable_deref_trait | 1.2.1 | Apache-2.0 OR MIT |
| string_cache | 0.11.0, 0.9.0 | Apache-2.0 OR MIT |
| strsim | 0.11.1 | MIT |
| subtle | 2.6.1 | BSD-3-Clause |
| swift-rs | 1.0.9 | Apache-2.0 OR MIT |
| syn | 1.0.109, 2.0.119, 3.0.10 | Apache-2.0 OR MIT |
| sync_wrapper | 1.0.2 | Apache-2.0 |
| synstructure | 0.14.0 | MIT |
| tao | 0.37.3 | Apache-2.0 |
| tao-macros | 0.1.4 | Apache-2.0 OR MIT |
| tauri | 2.12.7 | Apache-2.0 OR MIT |
| tauri-codegen | 2.7.6 | Apache-2.0 OR MIT |
| tauri-macros | 2.7.6 | Apache-2.0 OR MIT |
| tauri-plugin-dialog | 2.8.4 | Apache-2.0 OR MIT |
| tauri-plugin-fs | 2.7.1 | Apache-2.0 OR MIT |
| tauri-plugin-opener | 2.7.3 | Apache-2.0 OR MIT |
| tauri-runtime | 2.12.4 | Apache-2.0 OR MIT |
| tauri-runtime-wry | 2.12.5 | Apache-2.0 OR MIT |
| tauri-utils | 2.10.4 | Apache-2.0 OR MIT |
| tempfile | 3.27.0 | Apache-2.0 OR MIT |
| tendril | 0.5.1 | Apache-2.0 OR MIT |
| thiserror | 1.0.69, 2.0.21 | Apache-2.0 OR MIT |
| thiserror-impl | 1.0.69, 2.0.21 | Apache-2.0 OR MIT |
| time | 0.3.55 | Apache-2.0 OR MIT |
| time-core | 0.1.9 | Apache-2.0 OR MIT |
| time-macros | 0.2.32 | Apache-2.0 OR MIT |
| tinystr | 0.8.5 | Unicode-3.0 |
| tinyvec | 1.13.3 | Apache-2.0 OR MIT OR Zlib |
| tokio | 1.54.1 | MIT |
| tokio-rustls | 0.26.6 | Apache-2.0 OR MIT |
| tokio-util | 0.7.20 | MIT |
| toml | 1.2.3+spec-1.1.0 | Apache-2.0 OR MIT |
| toml_datetime | 0.6.3, 1.1.3+spec-1.1.0 | Apache-2.0 OR MIT |
| toml_edit | 0.19.15, 0.20.2, 0.25.21+spec-1.1.0 | Apache-2.0 OR MIT |
| toml_parser | 1.1.7+spec-1.1.0 | Apache-2.0 OR MIT |
| toml_writer | 1.2.0+spec-1.1.0 | Apache-2.0 OR MIT |
| tower | 0.5.3 | MIT |
| tower-http | 0.6.11 | MIT |
| tower-layer | 0.3.3 | MIT |
| tower-service | 0.3.3 | MIT |
| tracing | 0.1.44 | MIT |
| tracing-attributes | 0.1.31 | MIT |
| tracing-core | 0.1.36 | MIT |
| tray-icon | 0.25.1 | Apache-2.0 OR MIT |
| try-lock | 0.2.5 | MIT |
| typeid | 1.0.3 | Apache-2.0 OR MIT |
| typenum | 1.20.1 | Apache-2.0 OR MIT |
| uds_windows | 1.2.1 | MIT |
| unicase | 2.10.0 | Apache-2.0 OR MIT |
| unicode-ident | 1.0.27 | (Apache-2.0 OR MIT) AND Unicode-3.0 |
| unicode-segmentation | 1.13.3 | Apache-2.0 OR MIT |
| untrusted | 0.9.0 | ISC |
| url | 2.5.9 | Apache-2.0 OR MIT |
| urlpattern | 0.6.0 | MIT |
| utf8_iter | 1.1.0 | Apache-2.0 OR MIT |
| utf8parse | 0.2.2 | Apache-2.0 OR MIT |
| utf_types | 1.0.0 | Apache-2.0 OR MIT |
| uuid | 1.29.3 | Apache-2.0 OR MIT |
| walkdir | 2.5.1 | MIT OR Unlicense |
| want | 0.3.2 | MIT |
| wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 OR Apache-2.0 WITH LLVM-exception OR MIT |
| wasip2 | 1.0.4+wasi-0.2.12 | Apache-2.0 OR Apache-2.0 WITH LLVM-exception OR MIT |
| wasm-bindgen | 0.2.130 | Apache-2.0 OR MIT |
| wasm-bindgen-futures | 0.4.80 | Apache-2.0 OR MIT |
| wasm-bindgen-macro | 0.2.130 | Apache-2.0 OR MIT |
| wasm-bindgen-macro-support | 0.2.130 | Apache-2.0 OR MIT |
| wasm-bindgen-shared | 0.2.130 | Apache-2.0 OR MIT |
| wasm-streams | 0.5.0 | Apache-2.0 OR MIT |
| web-sys | 0.3.107 | Apache-2.0 OR MIT |
| web-time | 1.1.0 | Apache-2.0 OR MIT |
| web_atoms | 0.2.6, 0.3.0 | Apache-2.0 OR MIT |
| webkit2gtk | 2.0.2 | MIT |
| webkit2gtk-sys | 2.0.2 | MIT |
| webpki-root-certs | 1.0.12 | CDLA-Permissive-2.0 |
| webview2-com | 0.39.1 | MIT |
| webview2-com-macros | 0.8.1 | MIT |
| webview2-com-sys | 0.39.1 | MIT |
| winapi | 0.3.9 | Apache-2.0 OR MIT |
| winapi-i686-pc-windows-gnu | 0.4.0 | Apache-2.0 OR MIT |
| winapi-util | 0.1.11 | MIT OR Unlicense |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | Apache-2.0 OR MIT |
| window-vibrancy | 0.8.1 | Apache-2.0 OR MIT |
| windows | 0.62.2 | Apache-2.0 OR MIT |
| windows-collections | 0.3.2 | Apache-2.0 OR MIT |
| windows-core | 0.62.2 | Apache-2.0 OR MIT |
| windows-future | 0.3.2 | Apache-2.0 OR MIT |
| windows-implement | 0.60.2 | Apache-2.0 OR MIT |
| windows-interface | 0.59.3 | Apache-2.0 OR MIT |
| windows-link | 0.2.1 | Apache-2.0 OR MIT |
| windows-numerics | 0.3.1 | Apache-2.0 OR MIT |
| windows-result | 0.4.1 | Apache-2.0 OR MIT |
| windows-strings | 0.5.1 | Apache-2.0 OR MIT |
| windows-sys | 0.45.0, 0.52.0, 0.59.0, 0.61.2 | Apache-2.0 OR MIT |
| windows-targets | 0.42.2, 0.52.6 | Apache-2.0 OR MIT |
| windows-threading | 0.2.1 | Apache-2.0 OR MIT |
| windows-version | 0.1.7 | Apache-2.0 OR MIT |
| windows_aarch64_gnullvm | 0.42.2, 0.52.6 | Apache-2.0 OR MIT |
| windows_aarch64_msvc | 0.42.2, 0.52.6 | Apache-2.0 OR MIT |
| windows_i686_gnu | 0.42.2, 0.52.6 | Apache-2.0 OR MIT |
| windows_i686_gnullvm | 0.52.6 | Apache-2.0 OR MIT |
| windows_i686_msvc | 0.42.2, 0.52.6 | Apache-2.0 OR MIT |
| windows_x86_64_gnu | 0.42.2, 0.52.6 | Apache-2.0 OR MIT |
| windows_x86_64_gnullvm | 0.42.2, 0.52.6 | Apache-2.0 OR MIT |
| windows_x86_64_msvc | 0.42.2, 0.52.6 | Apache-2.0 OR MIT |
| winnow | 0.5.40, 1.0.5 | MIT |
| wit-bindgen | 0.57.1 | Apache-2.0 OR Apache-2.0 WITH LLVM-exception OR MIT |
| writeable | 0.6.4 | Unicode-3.0 |
| wry | 0.57.4 | Apache-2.0 OR MIT |
| x11 | 2.21.0 | MIT |
| x11-dl | 2.21.0 | MIT |
| yoke | 0.8.4 | Unicode-3.0 |
| yoke-derive | 0.8.4 | Unicode-3.0 |
| zbus | 5.19.0 | MIT |
| zbus_macros | 5.19.0 | MIT |
| zbus_names | 4.3.5 | MIT |
| zcheapstr | 1.4.0 | MIT |
| zerofrom | 0.1.8 | Unicode-3.0 |
| zerofrom-derive | 0.1.8 | Unicode-3.0 |
| zeroize | 1.9.2 | Apache-2.0 OR MIT |
| zerotrie | 0.2.5 | Unicode-3.0 |
| zerovec | 0.11.9 | Unicode-3.0 |
| zerovec-derive | 0.11.7 | Unicode-3.0 |
| zip | 2.6.1 | MIT |
| zlib-rs | 0.6.9 | Zlib |
| zmij | 1.0.23 | MIT |
| zopfli | 0.8.4 | Apache-2.0 |
| zvariant | 5.15.1 | MIT |
| zvariant_derive | 5.15.1 | MIT |
| zvariant_utils | 4.2.0 | MIT |
