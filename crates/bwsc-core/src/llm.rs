//! Seleção dos momentos virais com um modelo de linguagem (Claude ou OpenAI).

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Deserializer, Serialize};

use crate::config::{ClipOptions, LlmProvider, Settings};
use crate::process::Cancel;
use crate::transcribe::truncate;
use crate::transcript::Transcript;

/// Corte sugerido pela IA (antes do ajuste fino).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Candidate {
    #[serde(deserialize_with = "de_time")]
    pub start: f64,
    #[serde(deserialize_with = "de_time")]
    pub end: f64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub hook: String,
    #[serde(default, deserialize_with = "de_score")]
    pub score: f64,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub hashtags: Vec<String>,
}

#[derive(Deserialize)]
struct CandidateList {
    #[serde(default)]
    clips: Vec<Candidate>,
}

/// Janela de transcrição enviada por chamada (vídeos longos são divididos).
const WINDOW_SECONDS: f64 = 1500.0;
const WINDOW_OVERLAP: f64 = 120.0;

const SYSTEM_PROMPT: &str = "You are a world-class short-form video editor who finds the most viral \
moments in long videos for TikTok, Instagram Reels and YouTube Shorts. You think like the best \
clipping tools (e.g. Opus Clip): every clip must be self-contained, start with a strong hook in \
the first 3 seconds, deliver a complete idea or punchline, and end on a satisfying beat. \
You always answer with strict JSON only, no markdown.";

pub fn build_prompt(transcript_lines: &str, opts: &ClipOptions, wanted: usize) -> String {
    let focus = if opts.focus.trim().is_empty() {
        String::new()
    } else {
        format!("\nEXTRA INSTRUCTIONS FROM THE CREATOR (high priority): {}\n", opts.focus.trim())
    };
    format!(
        "Below is a timestamped transcript. Each line is `[start-end] text` with times in seconds.\n\
         \n\
         Select up to {wanted} of the BEST clips for vertical short videos.\n\
         Rules:\n\
         - Each clip must last between {min:.0} and {max:.0} seconds.\n\
         - Start exactly at the beginning of a line and end exactly at the end of a line, using the timestamps given.\n\
         - The first sentence must hook the viewer (a bold claim, question, conflict, surprise or emotion). Skip greetings, intros and filler.\n\
         - The clip must make sense without the rest of the video and must not end mid-thought.\n\
         - Clips must not overlap.\n\
         - Prefer: strong opinions, stories with a payoff, surprising facts, humor, emotional moments, practical tips, controversy.\n\
         - score: 0-100 virality potential (be honest and discriminating).\n\
         - title: catchy title, max 70 characters, in the SAME language as the transcript.\n\
         - hook: the opening sentence of the clip.\n\
         - reason: one short sentence on why it can go viral, in the SAME language as the transcript.\n\
         - hashtags: 3 to 5 relevant hashtags.\n\
         {focus}\n\
         Answer ONLY with JSON in this exact shape:\n\
         {{\"clips\":[{{\"start\":12.3,\"end\":58.1,\"title\":\"...\",\"hook\":\"...\",\"score\":87,\"reason\":\"...\",\"hashtags\":[\"#tag\"]}}]}}\n\
         \n\
         TRANSCRIPT:\n{transcript_lines}",
        min = opts.min_duration,
        max = opts.max_duration,
    )
}

/// Pede à IA os melhores momentos de toda a transcrição.
pub fn find_candidates(
    settings: &Settings,
    transcript: &Transcript,
    opts: &ClipOptions,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(f32, &str),
) -> Result<Vec<Candidate>> {
    let total = transcript.duration();
    if transcript.segments.is_empty() {
        bail!("a transcrição está vazia — o vídeo tem fala?");
    }
    let mut windows = Vec::new();
    let mut s = 0.0;
    loop {
        let e = (s + WINDOW_SECONDS).min(total + 1.0);
        windows.push((s, e));
        if e >= total {
            break;
        }
        s = e - WINDOW_OVERLAP;
    }
    let n = windows.len();
    let per_window = if n == 1 {
        opts.clip_count + 3
    } else {
        ((opts.clip_count as f64 * 1.5 / n as f64).ceil() as usize).max(3)
    };
    let mut all = Vec::new();
    for (i, (ws, we)) in windows.into_iter().enumerate() {
        cancel.check()?;
        on_progress(i as f32 / n as f32, &format!("IA analisando o conteúdo ({}/{n})…", i + 1));
        let lines = transcript.prompt_lines(ws, we);
        if lines.trim().is_empty() {
            continue;
        }
        let prompt = build_prompt(&lines, opts, per_window);
        let reply = complete(settings, SYSTEM_PROMPT, &prompt)?;
        let mut found = parse_candidates(&reply)
            .with_context(|| format!("a IA respondeu em formato inesperado: {}", truncate(&reply, 300)))?;
        all.append(&mut found);
    }
    on_progress(1.0, "Momentos encontrados");
    Ok(all)
}

/// Extrai a lista de cortes de uma resposta (tolerando texto ou ``` ao redor).
pub fn parse_candidates(reply: &str) -> Result<Vec<Candidate>> {
    let start = reply.find('{').context("nenhum JSON na resposta")?;
    let end = reply.rfind('}').context("JSON incompleto")?;
    let json = &reply[start..=end];
    if let Ok(list) = serde_json::from_str::<CandidateList>(json) {
        return Ok(list.clips);
    }
    // Alguns modelos devolvem a lista direto.
    if let (Some(a), Some(b)) = (reply.find('['), reply.rfind(']')) {
        if let Ok(v) = serde_json::from_str::<Vec<Candidate>>(&reply[a..=b]) {
            return Ok(v);
        }
    }
    let list: CandidateList = serde_json::from_str(json)?;
    Ok(list.clips)
}

/// Envia um prompt e devolve o texto da resposta.
pub fn complete(settings: &Settings, system: &str, prompt: &str) -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?;
    match settings.llm_provider {
        LlmProvider::Local | LlmProvider::Ollama => {
            bail!("IA local usa o módulo selector/local_ai, não a API de nuvem")
        }
        LlmProvider::Anthropic => {
            let key = settings.anthropic_api_key.trim();
            if key.is_empty() {
                bail!("Configure a chave da API da Anthropic (Claude) nas configurações.");
            }
            let url = format!("{}/v1/messages", settings.anthropic_base_url.trim_end_matches('/'));
            let body = serde_json::json!({
                "model": settings.anthropic_model,
                "max_tokens": 8000,
                "system": system,
                "messages": [{"role": "user", "content": prompt}],
            });
            let resp = client
                .post(url)
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .json(&body)
                .send()
                .context("não foi possível conectar à API da Anthropic")?;
            let status = resp.status();
            let text = resp.text()?;
            if !status.is_success() {
                bail!("API da Anthropic respondeu {status}: {}", truncate(&text, 400));
            }
            let v: serde_json::Value = serde_json::from_str(&text)?;
            let out: String = v["content"]
                .as_array()
                .map(|parts| {
                    parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("")
                })
                .unwrap_or_default();
            if out.is_empty() {
                bail!("resposta vazia da Anthropic: {}", truncate(&text, 300));
            }
            Ok(out)
        }
        LlmProvider::OpenAi => {
            let key = settings.openai_api_key.trim();
            if key.is_empty() {
                bail!("Configure a chave da API OpenAI nas configurações.");
            }
            let url = format!("{}/chat/completions", settings.openai_base_url.trim_end_matches('/'));
            let body = serde_json::json!({
                "model": settings.openai_llm_model,
                "messages": [
                    {"role": "system", "content": system},
                    {"role": "user", "content": prompt}
                ],
                "response_format": {"type": "json_object"},
            });
            let resp = client
                .post(url)
                .bearer_auth(key)
                .json(&body)
                .send()
                .context("não foi possível conectar à API OpenAI")?;
            let status = resp.status();
            let text = resp.text()?;
            if !status.is_success() {
                bail!("API OpenAI respondeu {status}: {}", truncate(&text, 400));
            }
            let v: serde_json::Value = serde_json::from_str(&text)?;
            v["choices"][0]["message"]["content"]
                .as_str()
                .map(str::to_string)
                .context("resposta vazia da OpenAI")
        }
    }
}

/// Aceita `12.5`, `"12.5"`, `"01:23"` ou `"1:02:03.5"`.
fn de_time<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<f64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum T {
        N(f64),
        S(String),
    }
    match T::deserialize(d)? {
        T::N(n) => Ok(n),
        T::S(s) => parse_clock(&s).ok_or_else(|| serde::de::Error::custom(format!("tempo inválido: {s}"))),
    }
}

fn de_score<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<f64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum T {
        N(f64),
        S(String),
    }
    Ok(match T::deserialize(d)? {
        T::N(n) => n,
        T::S(s) => s.trim().trim_end_matches('%').parse().unwrap_or(0.0),
    })
}

pub fn parse_clock(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches('s');
    let mut total = 0.0;
    for part in s.split(':') {
        total = total * 60.0 + part.trim().replace(',', ".").parse::<f64>().ok()?;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_reply_with_clock_times() {
        let reply = "Aqui está:\n```json\n{\"clips\":[{\"start\":\"01:05\",\"end\":95.5,\"title\":\"T\",\"score\":\"88\"}]}\n```";
        let c = parse_candidates(reply).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].start, 65.0);
        assert_eq!(c[0].end, 95.5);
        assert_eq!(c[0].score, 88.0);
        assert!(c[0].hashtags.is_empty());
    }

    #[test]
    fn clock() {
        assert_eq!(parse_clock("1:02:03.5"), Some(3723.5));
        assert_eq!(parse_clock("12,5"), Some(12.5));
    }
}
