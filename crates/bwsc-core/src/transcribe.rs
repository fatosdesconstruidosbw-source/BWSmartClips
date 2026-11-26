//! Transcrição: whisper.cpp local ou API compatível com OpenAI.

use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::config::{Settings, TranscriptionEngine};
use crate::media;
use crate::process::{command, run_lines, Cancel, Stream};
use crate::tools::Tools;
use crate::transcript::{Transcript, Word};

/// Transcreve `video` usando o motor escolhido nas configurações.
/// `work` é uma pasta temporária para os arquivos de áudio.
#[allow(clippy::too_many_arguments)]
pub fn transcribe(
    settings: &Settings,
    tools: &Tools,
    video: &Path,
    duration: f64,
    language: &str,
    work: &Path,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(f32, &str),
) -> Result<Transcript> {
    std::fs::create_dir_all(work)?;
    let ffmpeg = tools.ffmpeg()?;
    match settings.transcription {
        TranscriptionEngine::Local => {
            let whisper = tools.whisper()?;
            let model = tools.whisper_model()?;
            on_progress(0.0, "Extraindo áudio…");
            let wav = work.join("audio.wav");
            media::extract_wav(ffmpeg, video, &wav, cancel)?;
            let vad = if settings.whisper_vad { tools.vad_model.as_deref() } else { None };
            match transcribe_local(whisper, model, vad, &wav, language, settings.whisper_threads, work, cancel, on_progress) {
                // Versões antigas do whisper.cpp não têm VAD: tenta de novo sem ele.
                Err(e) if vad.is_some() && !crate::process::is_cancelled_error(&e) => {
                    on_progress(0.0, "Transcrevendo sem detector de voz…");
                    transcribe_local(whisper, model, None, &wav, language, settings.whisper_threads, work, cancel, on_progress)
                }
                other => other,
            }
        }
        TranscriptionEngine::OpenAi => {
            transcribe_openai(settings, ffmpeg, video, duration, language, work, cancel, on_progress)
        }
    }
}

// ---------------------------------------------------------------- whisper.cpp

#[derive(Deserialize)]
struct WhisperJson {
    #[serde(default)]
    result: Option<WhisperResult>,
    transcription: Vec<WhisperItem>,
}
#[derive(Deserialize)]
struct WhisperResult {
    language: Option<String>,
}
#[derive(Deserialize)]
struct WhisperItem {
    offsets: WhisperOffsets,
    text: String,
}
#[derive(Deserialize)]
struct WhisperOffsets {
    from: i64,
    to: i64,
}

#[allow(clippy::too_many_arguments)]
pub fn transcribe_local(
    whisper: &Path,
    model: &Path,
    vad_model: Option<&Path>,
    wav: &Path,
    language: &str,
    threads: usize,
    work: &Path,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(f32, &str),
) -> Result<Transcript> {
    let out_base = work.join("whisper");
    let threads = if threads == 0 {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8)
    } else {
        threads
    };
    let lang = if language.trim().is_empty() { "auto" } else { language.trim() };
    let mut cmd = command(whisper);
    cmd.arg("-m")
        .arg(model)
        .arg("-f")
        .arg(wav)
        .args(["-l", lang, "-t", &threads.to_string()])
        // Uma palavra por segmento → tempos palavra a palavra.
        .args(["-ml", "1", "-sow", "-oj", "-pp"])
        .arg("-of")
        .arg(&out_base);
    if let Some(vad) = vad_model {
        // Pula silêncio e música (detector de voz Silero): bem mais rápido.
        cmd.arg("--vad").arg("-vm").arg(vad);
    }
    on_progress(0.02, "Transcrevendo com Whisper local…");
    run_lines(cmd, cancel, |stream, line| {
        if stream == Stream::Stderr {
            if let Some(p) = parse_whisper_progress(line) {
                on_progress(p, "Transcrevendo com Whisper local…");
            }
        }
    })
    .context("falha ao transcrever com whisper.cpp")?;
    let json_path = out_base.with_extension("json");
    let text = std::fs::read(&json_path)
        .with_context(|| format!("o whisper não gerou {}", json_path.display()))?;
    parse_whisper_json(&String::from_utf8_lossy(&text))
}

fn parse_whisper_progress(line: &str) -> Option<f32> {
    let idx = line.find("progress =")?;
    let pct = line[idx + 10..].trim().trim_end_matches('%').trim();
    pct.parse::<f32>().ok().map(|p| (p / 100.0).clamp(0.0, 1.0))
}

pub fn parse_whisper_json(text: &str) -> Result<Transcript> {
    let parsed: WhisperJson = serde_json::from_str(text).context("JSON do whisper inválido")?;
    let mut words: Vec<Word> = Vec::with_capacity(parsed.transcription.len());
    for item in parsed.transcription {
        let t = item.text.trim();
        if t.is_empty() || is_annotation(t) {
            continue;
        }
        let start = item.offsets.from as f64 / 1000.0;
        let end = item.offsets.to as f64 / 1000.0;
        // Com -sow, pedaços sem espaço inicial são continuação da palavra anterior.
        let continues = !item.text.starts_with(' ') && !words.is_empty();
        if continues {
            let last = words.last_mut().unwrap();
            last.text.push_str(t);
            last.end = end.max(last.end);
        } else {
            words.push(Word { start, end: end.max(start), text: t.to_string() });
        }
    }
    let language = parsed.result.and_then(|r| r.language);
    Ok(Transcript::from_words(words, language))
}

fn is_annotation(t: &str) -> bool {
    (t.starts_with('[') && t.ends_with(']')) || (t.starts_with('(') && t.ends_with(')')) || t == "♪"
}

// ---------------------------------------------------------------- API OpenAI

#[derive(Deserialize)]
struct ApiResponse {
    language: Option<String>,
    #[serde(default)]
    words: Vec<ApiWord>,
    #[serde(default)]
    segments: Vec<ApiSegment>,
}
#[derive(Deserialize)]
struct ApiWord {
    word: String,
    start: f64,
    end: f64,
}
#[derive(Deserialize)]
struct ApiSegment {
    start: f64,
    end: f64,
    text: String,
}

/// Duração de cada pedaço enviado (a API aceita arquivos de até 25 MB).
const CHUNK_SECONDS: f64 = 600.0;

#[allow(clippy::too_many_arguments)]
fn transcribe_openai(
    settings: &Settings,
    ffmpeg: &Path,
    video: &Path,
    duration: f64,
    language: &str,
    work: &Path,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(f32, &str),
) -> Result<Transcript> {
    if settings.openai_api_key.trim().is_empty() {
        bail!("Configure a chave da API OpenAI para usar a transcrição pela API.");
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()?;
    let url = format!("{}/audio/transcriptions", settings.openai_base_url.trim_end_matches('/'));
    let chunks = (duration / CHUNK_SECONDS).ceil().max(1.0) as usize;
    let mut words = Vec::new();
    let mut detected = None;
    for i in 0..chunks {
        cancel.check()?;
        let start = i as f64 * CHUNK_SECONDS;
        let len = CHUNK_SECONDS.min(duration - start + 0.5);
        let msg = format!("Transcrevendo pela API ({}/{chunks})…", i + 1);
        on_progress(i as f32 / chunks as f32, &msg);
        let mp3 = work.join(format!("chunk_{i:03}.mp3"));
        media::extract_mp3_chunk(ffmpeg, video, &mp3, start, len, cancel)?;
        let bytes = std::fs::read(&mp3)?;
        let part = reqwest::blocking::multipart::Part::bytes(bytes)
            .file_name(format!("chunk_{i:03}.mp3"))
            .mime_str("audio/mpeg")?;
        let mut form = reqwest::blocking::multipart::Form::new()
            .part("file", part)
            .text("model", settings.openai_transcription_model.clone())
            .text("response_format", "verbose_json")
            .text("timestamp_granularities[]", "word")
            .text("timestamp_granularities[]", "segment");
        if !language.trim().is_empty() {
            form = form.text("language", language.trim().to_string());
        }
        let resp = client
            .post(&url)
            .bearer_auth(settings.openai_api_key.trim())
            .multipart(form)
            .send()
            .context("não foi possível conectar à API de transcrição")?;
        let status = resp.status();
        let body = resp.text()?;
        if !status.is_success() {
            bail!("API de transcrição respondeu {status}: {}", truncate(&body, 400));
        }
        let parsed: ApiResponse =
            serde_json::from_str(&body).context("resposta inesperada da API de transcrição")?;
        detected = detected.or(parsed.language);
        let chunk_words = api_words(parsed.words, parsed.segments);
        words.extend(chunk_words.into_iter().map(|w| Word { start: w.start + start, end: w.end + start, text: w.text }));
        let _ = std::fs::remove_file(&mp3);
    }
    on_progress(1.0, "Transcrição concluída");
    Ok(Transcript::from_words(words, detected))
}

/// Usa as palavras da API; se ela não mandar, divide os segmentos proporcionalmente.
fn api_words(words: Vec<ApiWord>, segments: Vec<ApiSegment>) -> Vec<Word> {
    if !words.is_empty() {
        let mut out: Vec<Word> = words
            .into_iter()
            .map(|w| Word { start: w.start, end: w.end, text: w.word.trim().to_string() })
            .collect();
        attach_punctuation(&mut out, &segments);
        return out;
    }
    let mut out = Vec::new();
    for s in segments {
        let parts: Vec<&str> = s.text.split_whitespace().collect();
        let total: usize = parts.iter().map(|p| p.chars().count()).sum::<usize>().max(1);
        let mut t = s.start;
        for p in parts {
            let d = (s.end - s.start) * p.chars().count() as f64 / total as f64;
            out.push(Word { start: t, end: t + d, text: p.to_string() });
            t += d;
        }
    }
    out
}

/// A API devolve palavras sem pontuação; recuperamos a pontuação a partir do texto dos segmentos.
fn attach_punctuation(words: &mut [Word], segments: &[ApiSegment]) {
    let mut wi = 0;
    for seg in segments {
        for token in seg.text.split_whitespace() {
            if wi >= words.len() {
                return;
            }
            let bare: String = token.chars().filter(|c| c.is_alphanumeric()).collect();
            let wbare: String = words[wi].text.chars().filter(|c| c.is_alphanumeric()).collect();
            if !bare.is_empty() && bare.to_lowercase() == wbare.to_lowercase() {
                words[wi].text = token.to_string();
                wi += 1;
            } else if !bare.is_empty() {
                // Tenta realinhar olhando algumas palavras à frente.
                if let Some(off) = (1..4).find(|&o| {
                    words.get(wi + o).is_some_and(|w| {
                        let b: String = w.text.chars().filter(|c| c.is_alphanumeric()).collect();
                        b.to_lowercase() == bare.to_lowercase()
                    })
                }) {
                    wi += off;
                    words[wi].text = token.to_string();
                    wi += 1;
                }
            }
        }
    }
}

pub(crate) fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_whisper_cpp_word_json() {
        let json = r#"{"result":{"language":"pt"},"transcription":[
            {"timestamps":{"from":"00:00:00,000","to":"00:00:00,400"},"offsets":{"from":0,"to":400},"text":" Olá"},
            {"offsets":{"from":400,"to":800},"text":" pesso"},
            {"offsets":{"from":800,"to":1000},"text":"al."},
            {"offsets":{"from":1000,"to":1500},"text":" [Música]"},
            {"offsets":{"from":1500,"to":1900},"text":" Tudo"},
            {"offsets":{"from":1900,"to":2300},"text":" bem?"}]}"#;
        let t = parse_whisper_json(json).unwrap();
        assert_eq!(t.language.as_deref(), Some("pt"));
        let texts: Vec<&str> = t.words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["Olá", "pessoal.", "Tudo", "bem?"]);
        assert_eq!(t.words[1].end, 1.0);
        assert_eq!(t.segments.len(), 2);
    }

    #[test]
    fn whisper_progress() {
        assert_eq!(parse_whisper_progress("whisper_print_progress_callback: progress =  40%"), Some(0.4));
    }

    #[test]
    fn api_punctuation_recovered() {
        let words = vec![
            ApiWord { word: "Oi".into(), start: 0.0, end: 0.2 },
            ApiWord { word: "gente".into(), start: 0.2, end: 0.5 },
            ApiWord { word: "bora".into(), start: 0.6, end: 0.9 },
        ];
        let segs = vec![ApiSegment { start: 0.0, end: 0.9, text: " Oi, gente! Bora.".into() }];
        let out = api_words(words, segs);
        let texts: Vec<&str> = out.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["Oi,", "gente!", "Bora."]);
    }
}
