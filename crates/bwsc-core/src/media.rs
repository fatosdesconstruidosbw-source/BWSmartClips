//! Leitura de informações do vídeo e extração de áudio com ffmpeg/ffprobe.

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::process::{command, run, Cancel};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub duration: f64,
    /// Largura já considerando rotação (como o ffmpeg decodifica).
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub has_audio: bool,
}

pub fn probe(ffprobe: &Path, input: &Path, cancel: &Cancel) -> Result<MediaInfo> {
    let mut cmd = command(ffprobe);
    cmd.args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
        .arg(input);
    let out = run(cmd, cancel).context("ffprobe não conseguiu ler o vídeo")?;
    parse_probe(&out.stdout.join("\n"))
}

pub fn parse_probe(json: &str) -> Result<MediaInfo> {
    let v: serde_json::Value = serde_json::from_str(json).context("saída inválida do ffprobe")?;
    let streams = v["streams"].as_array().cloned().unwrap_or_default();
    let video = streams
        .iter()
        .find(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"].as_i64() != Some(1));
    let Some(video) = video else {
        bail!("o arquivo não tem trilha de vídeo")
    };
    let has_audio = streams.iter().any(|s| s["codec_type"] == "audio");
    let mut width = video["width"].as_u64().unwrap_or(0) as u32;
    let mut height = video["height"].as_u64().unwrap_or(0) as u32;
    if width == 0 || height == 0 {
        bail!("não foi possível descobrir a resolução do vídeo");
    }
    let rotation = rotation_of(video);
    if rotation.rem_euclid(180) == 90 {
        std::mem::swap(&mut width, &mut height);
    }
    let fps = parse_rate(video["avg_frame_rate"].as_str())
        .or_else(|| parse_rate(video["r_frame_rate"].as_str()))
        .unwrap_or(30.0);
    let duration = v["format"]["duration"]
        .as_str()
        .and_then(|d| d.parse::<f64>().ok())
        .or_else(|| video["duration"].as_str().and_then(|d| d.parse().ok()))
        .unwrap_or(0.0);
    if duration <= 0.0 {
        bail!("não foi possível descobrir a duração do vídeo");
    }
    Ok(MediaInfo { duration, width, height, fps, has_audio })
}

fn rotation_of(stream: &serde_json::Value) -> i64 {
    if let Some(r) = stream["tags"]["rotate"].as_str().and_then(|r| r.parse::<i64>().ok()) {
        return r;
    }
    stream["side_data_list"]
        .as_array()
        .and_then(|l| l.iter().find_map(|sd| sd["rotation"].as_f64()))
        .map(|r| r.round() as i64)
        .unwrap_or(0)
}

fn parse_rate(s: Option<&str>) -> Option<f64> {
    let s = s?;
    let (n, d) = s.split_once('/').unwrap_or((s, "1"));
    let n: f64 = n.parse().ok()?;
    let d: f64 = d.parse().ok()?;
    (d > 0.0 && n > 0.0).then(|| n / d).filter(|f| f.is_finite() && *f < 1000.0)
}

/// Extrai o áudio em WAV 16 kHz mono (formato exigido pelo whisper.cpp).
pub fn extract_wav(ffmpeg: &Path, input: &Path, out: &Path, cancel: &Cancel) -> Result<()> {
    let mut cmd = command(ffmpeg);
    cmd.args(["-y", "-v", "error", "-i"])
        .arg(input)
        .args(["-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le"])
        .arg(out);
    run(cmd, cancel).context("falha ao extrair o áudio")?;
    Ok(())
}

/// Extrai um trecho do áudio em MP3 mono de baixa taxa (para enviar à API).
pub fn extract_mp3_chunk(
    ffmpeg: &Path,
    input: &Path,
    out: &Path,
    start: f64,
    duration: f64,
    cancel: &Cancel,
) -> Result<()> {
    let mut cmd = command(ffmpeg);
    cmd.args(["-y", "-v", "error", "-ss", &format!("{start:.3}"), "-t", &format!("{duration:.3}"), "-i"])
        .arg(input)
        .args(["-vn", "-ac", "1", "-ar", "16000", "-c:a", "libmp3lame", "-b:a", "48k"])
        .arg(out);
    run(cmd, cancel).context("falha ao extrair o trecho de áudio")?;
    Ok(())
}

/// Gera uma miniatura JPG do vídeo.
pub fn thumbnail(ffmpeg: &Path, video: &Path, out: &Path, at: f64, cancel: &Cancel) -> Result<()> {
    let mut cmd = command(ffmpeg);
    cmd.args(["-y", "-v", "error", "-ss", &format!("{at:.3}"), "-i"])
        .arg(video)
        .args(["-frames:v", "1", "-vf", "scale=360:-2", "-q:v", "4"])
        .arg(out);
    run(cmd, cancel)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rotated_phone_video() {
        let json = r#"{"streams":[{"codec_type":"video","width":1920,"height":1080,
            "avg_frame_rate":"30000/1001","side_data_list":[{"rotation":-90}]},
            {"codec_type":"audio"}],"format":{"duration":"62.5"}}"#;
        let m = parse_probe(json).unwrap();
        assert_eq!((m.width, m.height), (1080, 1920));
        assert!((m.fps - 29.97).abs() < 0.01);
        assert!(m.has_audio);
        assert_eq!(m.duration, 62.5);
    }
}
