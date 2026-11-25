# BWSmartClips

App desktop em **Rust** que transforma vídeos longos em **cortes virais verticais**, no estilo do Opus Clip:

- 🔗 Cole um link do **YouTube** (via `yt-dlp`) ou escolha um arquivo do computador
- 🎙️ **Transcrição palavra por palavra** com Whisper **local** (whisper.cpp, grátis e offline) **ou** pela **API OpenAI** (ou compatível, como Groq)
- 🧠 Uma **IA (Claude ou OpenAI)** lê a transcrição e escolhe os trechos com mais potencial viral, com nota 0–100, título, motivo e hashtags
- ✂️ Os cortes são ajustados para começar e terminar em frases completas, dentro da duração escolhida
- 📱 **Reenquadramento 9:16** que **segue o rosto** de quem fala (detecção em Rust puro, câmera suave, corte seco em troca de cena), ou centro / fundo desfocado / formato original
- 🔤 **Legendas animadas** palavra por palavra com destaque colorido e efeito "pulo" (fonte Montserrat ExtraBold embutida)

## Como usar (Windows)

1. Baixe o instalador na aba **Actions** (artefato `BWSmartClips-windows`) ou em **Releases**, e instale.
2. Abra o app → **⚙ Configurações → Ferramentas → Instalar o que falta**. Ele baixa para `%LOCALAPPDATA%\BWSmartClips`:
   ffmpeg, yt-dlp, Deno (exigido pelo yt-dlp para o YouTube), whisper.cpp e o modelo Whisper escolhido.
3. Em **Configurações → IA**, coloque a chave da **Anthropic** (Claude) ou da **OpenAI**.
4. Cole o link ou escolha o vídeo, ajuste as opções e clique em **✨ Gerar cortes**.

Os cortes ficam em `Vídeos\BWSmartClips\<data>-<título>\` com o `.mp4`, uma miniatura `.jpg`,
`transcricao.json` e `cortes.json` (títulos, notas, motivos e hashtags). O botão **Projetos** reabre trabalhos anteriores.

### Qual modelo do Whisper?

| Modelo | Tamanho | Quando usar |
|---|---|---|
| `small` | 466 MB | padrão — bom equilíbrio em CPU |
| `medium` | 1,5 GB | mais preciso, bem mais lento sem GPU |
| `large-v3-turbo-q5_0` | 547 MB | muito preciso; precisa de um PC forte |
| `base` / `tiny` | 142 / 75 MB | testes rápidos |

Se a transcrição local estiver lenta, use **Transcrição → API OpenAI** (ou o Groq, que é barato e rápido:
URL `https://api.groq.com/openai/v1`, modelo `whisper-large-v3`).

## Desenvolvimento

Pré-requisitos: [Rust](https://rustup.rs) estável, e no Windows o WebView2 (já vem no Windows 10/11).

```bash
cargo install tauri-cli --version "^2" --locked

cargo tauri dev            # abre o app em modo desenvolvimento
cargo tauri build          # gera o instalador (.exe NSIS e .msi) em target/release/bundle
cargo test -p bwsc-core    # testes do núcleo
```

No Linux, instale antes: `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`,
além de `ffmpeg`, `yt-dlp` e `whisper-cli` pelo gerenciador de pacotes.

### Linha de comando

Também há uma CLI, útil para testes e automação:

```bash
cargo run --release -p bwsc-cli -- run "https://youtu.be/..." -n 5 --min 20 --max 60 --focus "momentos engraçados"
cargo run --release -p bwsc-cli -- run video.mp4 --reframe blur
cargo run --release -p bwsc-cli -- run video.mp4 --clip 12-48 --clip 1:30-2:10   # cortes manuais, sem IA
cargo run --release -p bwsc-cli -- tools     # mostra as ferramentas encontradas
cargo run --release -p bwsc-cli -- install --model small   # instala as ferramentas (Windows)
```

A CLI usa as mesmas configurações do app (`%LOCALAPPDATA%\BWSmartClips\settings.json`).

## Arquitetura

```
crates/bwsc-core     núcleo (biblioteca)
  config.rs          configurações (JSON)
  download.rs        yt-dlp
  media.rs           ffprobe / extração de áudio / miniaturas
  transcribe.rs      whisper.cpp local ou API OpenAI → palavras com tempo
  transcript.rs      palavras → frases
  llm.rs             prompt + chamada ao Claude/OpenAI → candidatos
  clips.rs           encaixe em frases, limites de duração, sem sobreposição
  face.rs            detecção de rosto (rustface) + caminho de câmera suave
  captions.rs        legendas ASS palavra por palavra
  render.rs          ffmpeg decodifica → Rust recorta seguindo o rosto → ffmpeg codifica com legendas
  pipeline.rs        orquestra tudo, com progresso e cancelamento
  installer.rs       baixa ffmpeg, yt-dlp, Deno, whisper.cpp e modelo (Windows)
crates/bwsc-cli      CLI `bwsmartclips`
src-tauri            app desktop (Tauri 2): comandos e eventos
ui                   interface (HTML/CSS/JS puro, sem etapa de build)
```

## Licenças de terceiros

- Modelo de detecção de rosto SeetaFace (`seeta_fd_frontal_v1.0.bin`) — BSD 2-Clause
- Fonte Montserrat — SIL Open Font License 1.1
- ffmpeg, yt-dlp, Deno e whisper.cpp são baixados à parte e seguem suas próprias licenças.
