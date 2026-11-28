# BWSmartClips

App desktop em **Rust** que transforma vídeos longos em **cortes virais verticais**, no estilo do Opus Clip —
e que funciona **100% offline por padrão**, com IA local. APIs online (Claude/OpenAI) são opcionais.

- 🔗 Cole um link do **YouTube** (via `yt-dlp`) ou escolha um arquivo do computador
- 🎙️ **Transcrição palavra por palavra** com Whisper local (whisper.cpp), com **detector de voz** que pula silêncio e música
- 🧠 **IA local** (llama.cpp + Qwen 3.5) escolhe os trechos com mais potencial viral, com nota 0–100, título, motivo e hashtags
- ✂️ Os cortes começam e terminam em frases completas, dentro da duração escolhida
- 📱 **Reenquadramento 9:16** que **segue o rosto** de quem fala, ou centro / fundo desfocado / formato original
- 🔤 **Legendas animadas** palavra por palavra com destaque colorido e efeito "pulo"
- ♻️ **Cache**: transcrição, análise e download são reaproveitados — mudar só legenda/formato e gerar de novo é rápido

## Como usar (Windows)

1. Baixe o instalador mais recente em **[Releases](https://github.com/fatosdesconstruidosbw-source/BWSmartClips/releases/latest)**
   (`BWSmartClips_X.Y.Z_x64-setup.exe`), execute e abra o **BWSmartClips** pelo menu Iniciar.
2. Na primeira abertura, o **assistente** baixa tudo de uma vez para `%LOCALAPPDATA%\BWSmartClips`
   (ffmpeg, yt-dlp, Deno, whisper.cpp, llama.cpp, modelo Whisper, detector de voz e modelo de IA — cerca de 4 GB
   com as opções recomendadas). Depois disso, nada precisa de internet (só links do YouTube, claro).
3. Cole o link ou escolha o vídeo, ajuste as opções e clique em **✨ Gerar cortes**.

**PC sem internet?** Baixe os arquivos de modelo em outro computador (links abaixo), copie para um pendrive e use
**Importar modelos** no assistente ou em Configurações → Ferramentas e modelos.

Os cortes ficam em `Vídeos\BWSmartClips\<data>-<título>\` com o `.mp4`, uma miniatura `.jpg`,
`transcricao.json` e `cortes.json` (títulos, notas, motivos e hashtags). O botão **Projetos** reabre trabalhos anteriores.

### Como a IA local escolhe os cortes

Modelos pequenos rodando no processador são lentos para ler um vídeo inteiro e erram minutagens.
Por isso a escolha acontece em duas etapas:

1. **Regras em Rust (segundos):** monta dezenas de trechos já encaixados em frases e dá uma nota por sinais
   como pergunta/afirmação forte no início, ritmo de fala, volume da voz, palavras de impacto, risadas e vícios de linguagem.
2. **IA local:** avalia só os melhores (16 por padrão), um por vez, com resposta presa a um JSON Schema
   (não tem como "quebrar" o formato). Ela dá a nota, o título, o motivo e as hashtags, e pode aparar frases fracas no começo ou no fim.

Se a IA local não estiver instalada, o app ainda funciona só com as regras e avisa no resultado.

### Modelos

| Uso | Modelo | Tamanho | Observação |
|---|---|---|---|
| Transcrição | `ggml-small.bin` | 466 MB | padrão — bom equilíbrio em CPU |
| Transcrição | `ggml-medium.bin` / `ggml-large-v3-turbo-q5_0.bin` | 1,5 GB / 547 MB | mais precisos, mais lentos |
| Detector de voz | `ggml-silero-v5.1.2.bin` | 1 MB | pula silêncio e música |
| IA | `Qwen_Qwen3.5-2B-Q4_K_M.gguf` | 1,4 GB | o mais rápido |
| IA | `Qwen_Qwen3.5-4B-Q4_K_M.gguf` | 3,0 GB | **padrão** — bom equilíbrio |
| IA | `Qwen_Qwen3.5-9B-Q4_K_M.gguf` | 6,2 GB | mais esperto, bem mais lento em CPU |

Links: Whisper em `huggingface.co/ggerganov/whisper.cpp`, detector de voz em `huggingface.co/ggml-org/whisper-vad`,
IA em `huggingface.co/bartowski/Qwen_Qwen3.5-4B-GGUF` (e as versões 2B/9B). Qualquer outro `.gguf` de chat também funciona.

### Opcional: APIs online e Ollama

Em **Configurações → IA** dá para trocar a IA local por **Claude (Anthropic)**, **OpenAI** (ou compatível) ou um
**Ollama** já instalado. Em **Transcrição** dá para usar a API da OpenAI ou do Groq
(URL `https://api.groq.com/openai/v1`, modelo `whisper-large-v3`) no lugar do Whisper local.

## Desenvolvimento

Pré-requisitos: [Rust](https://rustup.rs) estável, e no Windows o WebView2 (já vem no Windows 10/11).

```bash
cargo install tauri-cli --version "^2" --locked

cargo tauri dev            # abre o app em modo desenvolvimento
cargo tauri build          # gera o instalador (.exe NSIS e .msi) em target/release/bundle
cargo test -p bwsc-core    # testes do núcleo

# teste de integração com um llama-server e um modelo .gguf reais
BWSC_LLAMA_SERVER=/caminho/llama-server BWSC_TEST_GGUF=/caminho/modelo.gguf cargo test -p bwsc-core -- --ignored
```

No Linux, instale antes: `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`,
além de `ffmpeg`, `yt-dlp`, `whisper-cli` e `llama-server` pelo gerenciador de pacotes
(o instalador do app baixa só os modelos fora do Windows).

### Linha de comando

Também há uma CLI, útil para testes e automação:

```bash
cargo run --release -p bwsc-cli -- run "https://youtu.be/..." -n 5 --min 20 --max 60 --focus "momentos engraçados"
cargo run --release -p bwsc-cli -- run video.mp4 --reframe blur
cargo run --release -p bwsc-cli -- run video.mp4 --clip 12-48 --clip 1:30-2:10   # cortes manuais, sem IA
cargo run --release -p bwsc-cli -- tools     # mostra as ferramentas encontradas
cargo run --release -p bwsc-cli -- install --whisper small --llm qwen3.5-4b   # ferramentas (Windows) + modelos
cargo run --release -p bwsc-cli -- import E:\modelos       # importa .gguf/.bin de uma pasta
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
  selector.rs        seleção em duas etapas: regras em Rust + avaliação pela IA local
  local_ai.rs        llama-server sob demanda (ou Ollama), respostas presas a JSON Schema
  llm.rs             modo online opcional: Claude/OpenAI leem a transcrição inteira
  cache.rs           cache de transcrições, análises e downloads
  clips.rs           encaixe em frases, limites de duração, sem sobreposição
  face.rs            detecção de rosto (rustface) + caminho de câmera suave
  captions.rs        legendas ASS palavra por palavra
  render.rs          ffmpeg decodifica → Rust recorta seguindo o rosto → ffmpeg codifica com legendas
  pipeline.rs        orquestra tudo, com progresso e cancelamento
  installer.rs       baixa ferramentas (Windows) e modelos; importa modelos de uma pasta
crates/bwsc-cli      CLI `bwsmartclips`
src-tauri            app desktop (Tauri 2): comandos e eventos
ui                   interface (HTML/CSS/JS puro, sem etapa de build)
```

## Publicar uma nova versão

1. Anote as mudanças em `CHANGELOG.md`, numa seção `## [X.Y.Z] - AAAA-MM-DD`.
2. Atualize a versão em `Cargo.toml` (`[workspace.package]`) e em `src-tauri/tauri.conf.json`.
3. Faça o commit e envie para o `main`.

Quando chega ao `main` um número de versão que ainda não existe em Releases (e com seção no CHANGELOG),
o GitHub Actions compila, cria a tag `vX.Y.Z` e publica a página com o instalador, o `.msi`, a linha de
comando e o texto do CHANGELOG. Commits sem mudança de versão só geram o instalador de teste em Actions.
Versões com hífen (ex.: `1.1.0-beta.1`) saem como pré-lançamento.

Versões: `X.Y.Z` → correção aumenta o Z, recurso novo aumenta o Y, mudança grande aumenta o X.

## Licença

Software proprietário — todos os direitos reservados (veja `LICENSE`). As versões 1.0.0 a 1.3.0 foram
publicadas antes sob a licença MIT.

Componentes de terceiros: `docs/legal/pt-BR/avisos-de-terceiros.md` (gerado por `scripts/gerar-documentos.py`).
Documentos para a venda (privacidade, termos, EULA) em `docs/legal/` e o guia da Microsoft Store em `docs/loja/`.
