//! Configurações persistentes do aplicativo (salvas em JSON).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Motor de transcrição.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionEngine {
    /// whisper.cpp rodando localmente (`whisper-cli`).
    #[default]
    Local,
    /// API compatível com OpenAI (`/audio/transcriptions`).
    OpenAi,
}

/// Provedor do modelo de linguagem que escolhe os cortes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LlmProvider {
    /// IA local com llama.cpp (`llama-server`) — padrão, 100% offline.
    #[default]
    Local,
    /// IA local servida por um Ollama já instalado.
    Ollama,
    /// API da Anthropic (Claude) — opcional, precisa de internet.
    Anthropic,
    /// API da OpenAI ou compatível — opcional, precisa de internet.
    OpenAi,
}

impl LlmProvider {
    /// Provedores que rodam no próprio computador.
    pub fn is_local(self) -> bool {
        matches!(self, LlmProvider::Local | LlmProvider::Ollama)
    }
}

/// Como transformar o vídeo em vertical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReframeMode {
    /// 9:16 seguindo o rosto de quem fala.
    #[default]
    Face,
    /// 9:16 cortando o centro.
    Center,
    /// 9:16 com o vídeo inteiro no meio e fundo desfocado.
    Blur,
    /// Mantém o formato original.
    Original,
}

/// Posição vertical das legendas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CaptionPosition {
    Top,
    Middle,
    #[default]
    Bottom,
}

/// Estilo das legendas animadas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CaptionStyle {
    pub enabled: bool,
    /// Nome da fonte. "Montserrat ExtraBold" vem embutida no app.
    pub font: String,
    /// Tamanho da fonte para um vídeo de 1080 px de largura.
    pub font_size: u32,
    /// Cor do texto, formato `#RRGGBB`.
    pub color: String,
    /// Cor da palavra falada no momento, formato `#RRGGBB`.
    pub highlight_color: String,
    /// Cor do contorno, formato `#RRGGBB`.
    pub outline_color: String,
    pub outline: f32,
    pub shadow: f32,
    pub uppercase: bool,
    /// Máximo de palavras visíveis ao mesmo tempo.
    pub max_words: usize,
    /// Máximo de caracteres por bloco de legenda.
    pub max_chars: usize,
    pub position: CaptionPosition,
    /// Efeito de "pulo" na palavra ativa.
    pub pop: bool,
}

impl Default for CaptionStyle {
    fn default() -> Self {
        Self {
            enabled: true,
            font: "Montserrat ExtraBold".into(),
            font_size: 82,
            color: "#FFFFFF".into(),
            highlight_color: "#FFE600".into(),
            outline_color: "#000000".into(),
            outline: 6.0,
            shadow: 2.0,
            uppercase: true,
            max_words: 3,
            max_chars: 22,
            position: CaptionPosition::Bottom,
            pop: true,
        }
    }
}

/// Opções de um trabalho de cortes (podem mudar a cada vídeo).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClipOptions {
    /// Quantos cortes gerar (no máximo).
    pub clip_count: usize,
    pub min_duration: f64,
    pub max_duration: f64,
    /// Código do idioma (`pt`, `en`...) ou vazio para detectar.
    pub language: String,
    /// Instrução extra para a IA, ex.: "momentos engraçados".
    pub focus: String,
    pub reframe: ReframeMode,
    pub captions: CaptionStyle,
    /// Ignora o cache e refaz transcrição e análise.
    pub reanalyze: bool,
}

impl Default for ClipOptions {
    fn default() -> Self {
        Self {
            clip_count: 8,
            min_duration: 20.0,
            max_duration: 75.0,
            language: "pt".into(),
            focus: String::new(),
            reframe: ReframeMode::Face,
            captions: CaptionStyle::default(),
            reanalyze: false,
        }
    }
}

/// Configurações globais do aplicativo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Pasta onde os projetos/cortes são salvos.
    pub output_dir: String,

    // Ferramentas externas (vazio = procurar automaticamente).
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub ytdlp_path: String,
    pub whisper_path: String,
    /// Arquivo do modelo ggml do whisper.cpp (vazio = procurar na pasta de modelos).
    pub whisper_model: String,
    /// Threads do whisper.cpp (0 = automático).
    pub whisper_threads: usize,
    /// Pula silêncio e música antes de transcrever (detector de voz Silero).
    pub whisper_vad: bool,

    pub transcription: TranscriptionEngine,
    pub openai_api_key: String,
    pub openai_base_url: String,
    pub openai_transcription_model: String,

    pub llm_provider: LlmProvider,

    // IA local (llama.cpp)
    pub llama_path: String,
    /// Arquivo .gguf do modelo (vazio = procurar na pasta de modelos).
    pub llm_model: String,
    /// Threads da IA local (0 = automático).
    pub llm_threads: usize,
    /// Tamanho do contexto da IA local, em tokens.
    pub llm_context: u32,
    /// Quantos trechos candidatos a IA local avalia.
    pub llm_candidates: usize,

    // Ollama (opcional)
    pub ollama_base_url: String,
    pub ollama_model: String,

    pub anthropic_api_key: String,
    pub anthropic_base_url: String,
    pub anthropic_model: String,
    pub openai_llm_model: String,

    /// Preset do x264 (ultrafast … slow).
    pub x264_preset: String,
    pub x264_crf: u32,
    /// Altura do vídeo final em modo vertical (1920 ou 1280).
    pub output_height: u32,

    /// Reaproveita transcrições, análises e downloads já feitos.
    pub use_cache: bool,
    /// O assistente de primeira abertura já foi concluído.
    pub setup_done: bool,

    /// Opções padrão para novos trabalhos.
    pub defaults: ClipOptions,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir().to_string_lossy().into_owned(),
            ffmpeg_path: String::new(),
            ffprobe_path: String::new(),
            ytdlp_path: String::new(),
            whisper_path: String::new(),
            whisper_model: String::new(),
            whisper_threads: 0,
            whisper_vad: true,
            transcription: TranscriptionEngine::Local,
            openai_api_key: String::new(),
            openai_base_url: "https://api.openai.com/v1".into(),
            openai_transcription_model: "whisper-1".into(),
            llm_provider: LlmProvider::Local,
            llama_path: String::new(),
            llm_model: String::new(),
            llm_threads: 0,
            llm_context: 8192,
            llm_candidates: 16,
            ollama_base_url: "http://127.0.0.1:11434".into(),
            ollama_model: "qwen3.5:4b".into(),
            anthropic_api_key: String::new(),
            anthropic_base_url: "https://api.anthropic.com".into(),
            anthropic_model: "claude-sonnet-5-5".into(),
            openai_llm_model: "gpt-4.1-mini".into(),
            x264_preset: "veryfast".into(),
            x264_crf: 20,
            output_height: 1920,
            use_cache: true,
            setup_done: false,
            defaults: ClipOptions::default(),
        }
    }
}

/// Pasta de dados do app: `%LOCALAPPDATA%\BWSmartClips` no Windows,
/// `~/.local/share/BWSmartClips` no Linux.
pub fn app_data_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.data_local_dir().join("BWSmartClips"))
        .unwrap_or_else(|| PathBuf::from("BWSmartClips"))
}

/// Onde ficam ffmpeg, yt-dlp, whisper-cli baixados pelo instalador do app.
pub fn tools_dir() -> PathBuf {
    app_data_dir().join("tools")
}

/// Onde ficam os modelos (Whisper, detector de voz e IA local).
pub fn models_dir() -> PathBuf {
    app_data_dir().join("models")
}

/// Cache de transcrições, análises e downloads.
pub fn cache_dir() -> PathBuf {
    app_data_dir().join("cache")
}

fn default_output_dir() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|u| u.video_dir().map(Path::to_path_buf))
        .unwrap_or_else(app_data_dir)
        .join("BWSmartClips")
}

impl Settings {
    pub fn default_path() -> PathBuf {
        app_data_dir().join("settings.json")
    }

    /// Carrega as configurações; se não existir arquivo, usa o padrão.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("não foi possível ler {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("configuração inválida em {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text).with_context(|| format!("não foi possível salvar {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_json_fills_defaults() {
        let s: Settings = serde_json::from_str(r#"{"llm_provider":"open_ai","defaults":{"clip_count":3}}"#).unwrap();
        assert_eq!(s.llm_provider, LlmProvider::OpenAi);
        assert_eq!(s.defaults.clip_count, 3);
        assert_eq!(s.defaults.max_duration, 75.0);
        assert!(s.defaults.captions.enabled);
    }

    #[test]
    fn offline_by_default() {
        let s = Settings::default();
        assert_eq!(s.transcription, TranscriptionEngine::Local);
        assert!(s.llm_provider.is_local());
        assert!(s.whisper_vad && s.use_cache && !s.setup_done);
    }
}
