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
    /// Para lives de jogos: webcam do streamer em cima, gameplay embaixo.
    Gameplay,
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
    pub italic: bool,
    /// Cor das palavras que ainda não foram faladas (vazio = mesma cor do texto).
    pub upcoming_color: String,
    /// Cor da caixa atrás do texto (vazio = sem caixa, só contorno).
    pub box_color: String,
    /// Estilo pronto escolhido na interface (só informativo).
    pub preset: String,
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
            italic: false,
            upcoming_color: String::new(),
            box_color: String::new(),
            preset: "viral".into(),
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
    /// Layout Gameplay: fração da altura ocupada pela webcam (0.25–0.55).
    pub cam_ratio: f64,
    /// Layout Gameplay: posição da webcam marcada à mão (vazio = detectar em cada corte).
    pub facecam: Option<crate::facecam::NormRect>,
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
            cam_ratio: 0.38,
            facecam: None,
        }
    }
}

/// Configurações globais do aplicativo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Pasta onde os projetos/cortes são salvos (vazio = padrão do app).
    pub output_dir: String,
    /// Pastas de saída já usadas, para a lista de Projetos continuar achando cortes antigos.
    pub output_history: Vec<String>,
    /// Pasta dos modelos de IA (vazio = dentro dos dados do app).
    pub models_dir: String,

    // Ferramentas externas (vazio = procurar automaticamente).
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub ytdlp_path: String,
    /// Arquivo cookies.txt exportado do navegador (avançado; vale para qualquer plataforma).
    pub youtube_cookies_file: String,
    /// Navegador de onde o yt-dlp lê os cookies: "", "firefox", "edge", "chrome", "brave"…
    pub cookies_browser: String,
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
    /// Usa a placa de vídeo (Vulkan: AMD, NVIDIA ou Intel) na IA local quando houver.
    pub llm_gpu: bool,
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
    /// O tour "Como funciona" já foi visto.
    pub tour_done: bool,

    /// Opções padrão para novos trabalhos.
    pub defaults: ClipOptions,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: String::new(),
            output_history: Vec::new(),
            models_dir: String::new(),
            ffmpeg_path: String::new(),
            ffprobe_path: String::new(),
            ytdlp_path: String::new(),
            youtube_cookies_file: String::new(),
            cookies_browser: String::new(),
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
            llm_gpu: true,
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
            tour_done: false,
            defaults: ClipOptions::default(),
        }
    }
}

/// Pasta de dados antiga/padrão: `%LOCALAPPDATA%\BWSmartClips` no Windows,
/// `~/.local/share/BWSmartClips` no Linux.
pub fn legacy_data_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.data_local_dir().join("BWSmartClips"))
        .unwrap_or_else(|| PathBuf::from("BWSmartClips"))
}

/// Arquivo (na pasta padrão) que aponta para a pasta de dados quando o app foi instalado
/// em outro lugar, para a versão de linha de comando achar os mesmos dados.
const DATA_POINTER: &str = "pasta-de-dados.txt";

static DATA_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Pasta onde o app foi instalado pelo instalador (tem o `uninstall.exe` ao lado).
pub fn installed_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_path_buf();
    dir.join("uninstall.exe").is_file().then_some(dir)
}

fn resolve_data_dir() -> PathBuf {
    if let Some(v) = std::env::var_os("BWSMARTCLIPS_DATA").filter(|v| !v.is_empty()) {
        return PathBuf::from(v);
    }
    // Edição Store: a pasta de dados do pacote (o Windows a leva junto se o app for
    // movido para outro disco em Configurações → Aplicativos → Mover).
    if cfg!(feature = "store") {
        return legacy_data_dir();
    }
    // Os dados ficam junto do programa: instalou no D:, tudo fica no D:.
    if let Some(dir) = installed_dir() {
        return dir;
    }
    if let Ok(text) = std::fs::read_to_string(legacy_data_dir().join(DATA_POINTER)) {
        let p = PathBuf::from(text.trim());
        if !text.trim().is_empty() && p.is_dir() {
            return p;
        }
    }
    legacy_data_dir()
}

/// Pasta de dados do app (configurações, ferramentas, modelos, cache e registros):
/// a pasta onde o programa foi instalado ou, sem instalador, a pasta padrão.
pub fn app_data_dir() -> PathBuf {
    DATA_DIR.get_or_init(resolve_data_dir).clone()
}

/// Grava (ou apaga) o apontador na pasta padrão para a linha de comando achar os dados.
pub fn update_data_pointer() {
    if cfg!(feature = "store") {
        return;
    }
    let (cur, legacy) = (app_data_dir(), legacy_data_dir());
    let ptr = legacy.join(DATA_POINTER);
    if cur == legacy {
        let _ = std::fs::remove_file(ptr);
    } else if std::fs::read_to_string(&ptr).ok().as_deref().map(str::trim) != Some(&*cur.to_string_lossy()) {
        let _ = std::fs::create_dir_all(&legacy);
        let _ = std::fs::write(ptr, cur.to_string_lossy().as_bytes());
    }
}

/// Itens de dados do app que podem ser levados para outra pasta.
pub const DATA_ITEMS: &[&str] = &[
    "settings.json",
    "api-keys.dat",
    "tools",
    "models",
    "cache",
    "logs",
    "youtube-login.dat",
    "twitch-login.dat",
    "kick-login.dat",
    "youtube-cookies.txt",
    "twitch-cookies.txt",
    "kick-cookies.txt",
];

/// Onde ficam ffmpeg, yt-dlp, whisper-cli baixados pelo instalador do app.
pub fn tools_dir() -> PathBuf {
    app_data_dir().join("tools")
}

/// Pasta de modelos padrão, dentro dos dados do app.
pub fn default_models_dir() -> PathBuf {
    app_data_dir().join("models")
}

static MODELS_DIR_OVERRIDE: std::sync::RwLock<Option<PathBuf>> = std::sync::RwLock::new(None);

/// Define a pasta de modelos escolhida pelo usuário (vazio = padrão).
pub fn set_models_dir(dir: &str) {
    let dir = dir.trim();
    let v = (!dir.is_empty()).then(|| PathBuf::from(dir));
    if let Ok(mut g) = MODELS_DIR_OVERRIDE.write() {
        *g = v;
    }
}

/// Onde ficam os modelos (Whisper, detector de voz e IA local): a pasta escolhida
/// nas configurações ou a padrão.
pub fn models_dir() -> PathBuf {
    MODELS_DIR_OVERRIDE.read().ok().and_then(|g| g.clone()).unwrap_or_else(default_models_dir)
}

/// Pastas onde procurar modelos: a atual e, se for outra, a padrão
/// (para os modelos antigos continuarem funcionando até serem movidos).
pub fn models_search_dirs() -> Vec<PathBuf> {
    let cur = models_dir();
    let def = default_models_dir();
    if cur == def { vec![cur] } else { vec![cur, def] }
}

/// Procura um arquivo de modelo pelo nome nas pastas de modelos.
pub fn find_model_file(name: &str) -> Option<PathBuf> {
    models_search_dirs().into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}


/// Cache de transcrições, análises e downloads.
pub fn cache_dir() -> PathBuf {
    app_data_dir().join("cache")
}

pub fn default_output_dir() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|u| u.video_dir().map(Path::to_path_buf))
        .unwrap_or_else(app_data_dir)
        .join("BWSmartClips")
}

/// Arquivo protegido com as chaves de API, ao lado do `settings.json`.
fn keys_path(settings_path: &Path) -> PathBuf {
    settings_path.with_file_name("api-keys.dat")
}

impl Settings {
    /// Pasta onde os cortes serão salvos: a escolhida pelo usuário ou a padrão.
    pub fn output_base(&self) -> PathBuf {
        let s = self.output_dir.trim();
        if s.is_empty() { default_output_dir() } else { PathBuf::from(s) }
    }

    /// Todas as pastas onde pode haver projetos (atual, padrão e já usadas), sem repetir.
    pub fn project_dirs(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        let all = std::iter::once(self.output_base())
            .chain(std::iter::once(default_output_dir()))
            .chain(self.output_history.iter().filter(|s| !s.trim().is_empty()).map(PathBuf::from));
        for d in all {
            if !out.iter().any(|o| o == &d) {
                out.push(d);
            }
        }
        out
    }

    /// Lembra a pasta atual na lista de pastas usadas. Devolve `true` se mudou algo.
    pub fn remember_output(&mut self) -> bool {
        let cur = self.output_dir.trim().to_string();
        if cur.is_empty() || self.output_history.iter().any(|h| h == &cur) {
            return false;
        }
        self.output_history.push(cur);
        if self.output_history.len() > 20 {
            self.output_history.remove(0);
        }
        true
    }

    pub fn default_path() -> PathBuf {
        app_data_dir().join("settings.json")
    }

    /// Carrega as configurações; se não existir arquivo, usa o padrão.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            let s = Self::default();
            s.apply();
            return Ok(s);
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("não foi possível ler {}", path.display()))?;
        let mut s: Self = serde_json::from_str(&text).with_context(|| format!("configuração inválida em {}", path.display()))?;
        let had_plain_keys = !s.openai_api_key.is_empty() || !s.anthropic_api_key.is_empty();
        if let Ok(keys) = crate::secret::read(&keys_path(path)) {
            if let Ok((openai, anthropic)) = serde_json::from_slice::<(String, String)>(&keys) {
                if s.openai_api_key.is_empty() {
                    s.openai_api_key = openai;
                }
                if s.anthropic_api_key.is_empty() {
                    s.anthropic_api_key = anthropic;
                }
            }
        }
        // Chaves antigas em texto puro passam a ser guardadas protegidas.
        if had_plain_keys {
            let _ = s.save(path);
        }
        s.apply();
        Ok(s)
    }

    /// Aplica as configurações que valem para o processo todo (pasta de modelos).
    pub fn apply(&self) {
        set_models_dir(&self.models_dir);
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // As chaves de API ficam fora do JSON, protegidas pelo Windows (DPAPI).
        let keys = keys_path(path);
        if self.openai_api_key.is_empty() && self.anthropic_api_key.is_empty() {
            let _ = std::fs::remove_file(&keys);
        } else {
            crate::secret::write(&keys, &serde_json::to_vec(&(&self.openai_api_key, &self.anthropic_api_key))?)?;
        }
        let mut public = self.clone();
        public.openai_api_key.clear();
        public.anthropic_api_key.clear();
        let text = serde_json::to_string_pretty(&public)?;
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

#[cfg(test)]
mod output_tests {
    use super::*;

    #[test]
    fn empty_output_uses_default_and_history_is_deduped() {
        let mut s = Settings::default();
        assert_eq!(s.output_base(), default_output_dir());
        assert!(!s.remember_output());
        s.output_dir = "/tmp/meus-cortes".into();
        assert!(s.remember_output());
        assert!(!s.remember_output());
        let dirs = s.project_dirs();
        assert_eq!(dirs[0], PathBuf::from("/tmp/meus-cortes"));
        assert!(dirs.contains(&default_output_dir()));
        assert_eq!(dirs.len(), 2);
    }
}

#[cfg(test)]
mod keys_tests {
    use super::*;

    #[test]
    fn api_keys_are_not_written_in_plain_json() {
        let dir = std::env::temp_dir().join(format!("bwsc-keys-{}", std::process::id()));
        let f = dir.join("settings.json");
        let s = Settings { openai_api_key: "sk-segredo-123".into(), ..Settings::default() };
        s.save(&f).unwrap();
        let json = std::fs::read_to_string(&f).unwrap();
        assert!(!json.contains("sk-segredo-123"), "chave vazou no JSON");
        let back = Settings::load(&f).unwrap();
        assert_eq!(back.openai_api_key, "sk-segredo-123");
        // JSON antigo com a chave em texto puro é convertido ao carregar.
        std::fs::write(&f, r#"{"anthropic_api_key": "sk-ant-velha"}"#).unwrap();
        let old = Settings::load(&f).unwrap();
        assert_eq!(old.anthropic_api_key, "sk-ant-velha");
        assert!(!std::fs::read_to_string(&f).unwrap().contains("sk-ant-velha"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
