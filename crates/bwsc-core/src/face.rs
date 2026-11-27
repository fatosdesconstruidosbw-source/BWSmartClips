//! Rastreamento de rosto para o reenquadramento 9:16.
//!
//! Amostramos alguns quadros por segundo, detectamos rostos com o rustface
//! (SeetaFace, 100% Rust) e calculamos um caminho de câmera suave com
//! zona morta e cortes secos quando a cena muda.

use std::io::Read;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use rustface::{Detector, ImageData};

use crate::process::{command, drain_stderr, kill, Cancel, Cancelled};

static MODEL: &[u8] = include_bytes!("../assets/seeta_fd_frontal_v1.0.bin");

/// Quadros por segundo analisados.
pub const SAMPLE_FPS: f64 = 4.0;
/// Largura usada para a detecção (menor = mais rápido).
const DETECT_WIDTH: u32 = 640;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Face {
    /// Centro horizontal normalizado (0–1).
    pub cx: f64,
    /// Centro vertical normalizado (0–1).
    pub cy: f64,
    /// Largura normalizada (0–1).
    pub w: f64,
    pub score: f64,
}

pub fn new_detector() -> Result<Box<dyn Detector>> {
    let model = rustface::read_model(std::io::Cursor::new(MODEL)).context("modelo de rosto inválido")?;
    let mut det = rustface::create_detector_with_model(model);
    det.set_min_face_size(24);
    det.set_score_thresh(2.0);
    det.set_pyramid_scale_factor(0.8);
    det.set_slide_window_step(4, 4);
    Ok(det)
}

/// Detecta rostos numa imagem em tons de cinza.
pub fn detect(det: &mut dyn Detector, gray: &[u8], width: u32, height: u32) -> Vec<Face> {
    let img = ImageData::new(gray, width, height);
    det.detect(&img)
        .into_iter()
        .map(|f| {
            let b = f.bbox();
            Face {
                cx: (b.x() as f64 + b.width() as f64 / 2.0) / width as f64,
                cy: (b.y() as f64 + b.height() as f64 / 2.0) / height as f64,
                w: b.width() as f64 / width as f64,
                score: f.score(),
            }
        })
        .collect()
}

/// Amostra o trecho `[start, start+duration)` e devolve, para cada amostra,
/// o centro horizontal (0–1) do rosto principal, se houver.
#[allow(clippy::too_many_arguments)]
pub fn track(
    ffmpeg: &Path,
    input: &Path,
    src_w: u32,
    src_h: u32,
    start: f64,
    duration: f64,
    cancel: &Cancel,
    mut on_progress: impl FnMut(f32),
) -> Result<Vec<Option<f64>>> {
    let dw = DETECT_WIDTH.min(src_w) & !1;
    let dh = ((src_h as f64 * dw as f64 / src_w as f64).round() as u32).max(2) & !1;
    let mut cmd = command(ffmpeg);
    cmd.args(["-v", "error", "-ss", &format!("{start:.3}"), "-t", &format!("{duration:.3}"), "-i"])
        .arg(input)
        .args([
            "-an",
            "-vf",
            &format!("fps={SAMPLE_FPS},scale={dw}:{dh},format=gray"),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "gray",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().context("não foi possível executar o ffmpeg")?;
    let err = drain_stderr(&mut child);
    let mut stdout = child.stdout.take().unwrap();
    let mut det = new_detector()?;
    let frame_len = (dw * dh) as usize;
    let mut buf = vec![0u8; frame_len];
    let expected = (duration * SAMPLE_FPS).ceil().max(1.0) as usize;
    let mut out = Vec::with_capacity(expected);
    let mut prev: Option<f64> = None;
    loop {
        if cancel.is_cancelled() {
            kill(&mut child);
            anyhow::bail!(Cancelled);
        }
        match stdout.read_exact(&mut buf) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }
        let faces = detect(det.as_mut(), &buf, dw, dh);
        let pick = choose_face(&faces, prev);
        if let Some(cx) = pick {
            prev = Some(cx);
        }
        out.push(pick);
        on_progress((out.len() as f32 / expected as f32).min(1.0));
    }
    let status = child.wait()?;
    let stderr = err.join().unwrap_or_default();
    if !status.success() && out.is_empty() {
        anyhow::bail!("ffmpeg falhou ao ler quadros para rastrear rosto:\n{stderr}");
    }
    Ok(out)
}

/// Escolhe o rosto principal: o maior, mas mantém o anterior se ele ainda
/// estiver lá e não for muito menor (evita a câmera "pular" entre pessoas).
pub fn choose_face(faces: &[Face], prev: Option<f64>) -> Option<f64> {
    let largest = faces.iter().max_by(|a, b| (a.w * a.score.min(20.0)).total_cmp(&(b.w * b.score.min(20.0))))?;
    if let Some(p) = prev {
        if let Some(same) = faces
            .iter()
            .filter(|f| (f.cx - p).abs() < 0.08 && f.w >= largest.w * 0.6)
            .min_by(|a, b| (a.cx - p).abs().total_cmp(&(b.cx - p).abs()))
        {
            return Some(same.cx);
        }
    }
    Some(largest.cx)
}

/// Converte as amostras num caminho de câmera: para cada quadro de saída,
/// a posição X (em pixels da fonte) do canto esquerdo do recorte.
pub fn camera_path(samples: &[Option<f64>], frames: usize, fps: f64, src_w: u32, crop_w: u32) -> Vec<u32> {
    let max_x = src_w.saturating_sub(crop_w) as f64;
    if frames == 0 {
        return Vec::new();
    }
    if max_x <= 0.0 {
        return vec![0; frames];
    }
    // 1) Preenche buracos (sem rosto) com o último valor conhecido.
    let mut filled: Vec<f64> = Vec::with_capacity(samples.len());
    let first = samples.iter().flatten().next().copied().unwrap_or(0.5);
    let mut last = first;
    for s in samples {
        if let Some(v) = s {
            last = *v;
        }
        filled.push(last);
    }
    if filled.is_empty() {
        filled.push(0.5);
    }
    // 2) Mediana de 5 para remover detecções falsas isoladas.
    let med: Vec<f64> = (0..filled.len())
        .map(|i| {
            let a = i.saturating_sub(2);
            let b = (i + 3).min(filled.len());
            let mut w: Vec<f64> = filled[a..b].to_vec();
            w.sort_by(f64::total_cmp);
            w[w.len() / 2]
        })
        .collect();

    let cw = crop_w as f64 / src_w as f64; // largura do recorte, normalizada
    let dead = cw * 0.12;
    let cut = cw * 0.55;
    let alpha = (1.0 - (-1.0 / (fps * 0.35)).exp()).clamp(0.01, 1.0);

    // 3) Alvo por quadro (interpolação linear, mas degrau em mudanças bruscas).
    let target_at = |t: f64| -> f64 {
        let pos = t * SAMPLE_FPS;
        let i = (pos.floor() as usize).min(med.len() - 1);
        let j = (i + 1).min(med.len() - 1);
        let (a, b) = (med[i], med[j]);
        if (b - a).abs() > cut {
            a
        } else {
            a + (b - a) * (pos - i as f64).clamp(0.0, 1.0)
        }
    };

    // 4) Câmera suave com zona morta; cortes secos quando o alvo salta.
    let mut cam = target_at(0.0);
    let mut prev_target = cam;
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        let target = target_at(f as f64 / fps);
        if (target - prev_target).abs() > cut || (target - cam).abs() > cw * 0.9 {
            cam = target;
        } else {
            let err = target - cam;
            if err.abs() > dead {
                cam += (err - dead * err.signum()) * alpha;
            }
        }
        prev_target = target;
        let x = crate::render::safe_clamp(cam * src_w as f64 - crop_w as f64 / 2.0, 0.0, max_x);
        out.push(x.round() as u32);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_stays_inside_and_snaps_on_cut() {
        // Rosto à esquerda por 2 s, depois à direita (mudança de cena).
        let mut samples = vec![Some(0.25); 8];
        samples.extend(vec![Some(0.8); 8]);
        let path = camera_path(&samples, 120, 30.0, 1920, 606);
        assert_eq!(path.len(), 120);
        assert!(path.iter().all(|&x| x <= 1920 - 606));
        let left = (0.25 * 1920.0 - 303.0) as i64;
        assert!((path[10] as i64 - left).abs() < 5);
        // Logo após a troca a câmera já está no novo rosto (corte seco).
        let right = (0.8 * 1920.0 - 303.0) as i64;
        assert!((path[66] as i64 - right).abs() < 5, "x={}", path[66]);
    }

    #[test]
    fn small_jitter_is_ignored() {
        let samples: Vec<Option<f64>> = (0..20).map(|i| Some(0.5 + if i % 2 == 0 { 0.01 } else { -0.01 })).collect();
        let path = camera_path(&samples, 150, 30.0, 1920, 606);
        let min = *path.iter().min().unwrap();
        let max = *path.iter().max().unwrap();
        assert!(max - min <= 2, "câmera tremendo: {min}..{max}");
    }

    #[test]
    fn keeps_previous_speaker() {
        let faces = [
            Face { cx: 0.2, cy: 0.4, w: 0.10, score: 5.0 },
            Face { cx: 0.7, cy: 0.4, w: 0.12, score: 5.0 },
        ];
        assert_eq!(choose_face(&faces, None), Some(0.7));
        assert_eq!(choose_face(&faces, Some(0.21)), Some(0.2));
        assert_eq!(choose_face(&[], Some(0.3)), None);
    }
}
