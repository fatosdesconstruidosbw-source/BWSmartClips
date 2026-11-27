//! Detecção da webcam do streamer ("facecam") para o layout Gameplay.
//!
//! Ideia: numa transmissão, a webcam é um retângulo que fica **parado** no mesmo
//! lugar enquanto o jogo muda. Então:
//! 1. amostramos vários quadros e detectamos rostos; o rosto que aparece quase
//!    sempre no mesmo ponto é o do streamer (rostos de personagens se movem);
//! 2. calculamos as bordas "persistentes" (mediana do gradiente entre os quadros);
//!    as bordas da moldura da webcam aparecem em todos os quadros;
//! 3. a partir do rosto, procuramos a borda persistente mais forte em cada lado.
//!    Se não houver moldura (webcam sem borda ou recortada), usamos uma caixa
//!    proporcional ao tamanho do rosto.

use std::io::Read;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::face::{detect, new_detector, Face};
use crate::process::{command, drain_stderr, kill, Cancel, Cancelled};

/// Largura usada na análise (a webcam costuma ser pequena; 960 px dá rostos de ~40 px).
const ANALYSIS_WIDTH: u32 = 960;
/// Quantos quadros analisar.
const SAMPLES: usize = 24;

/// Retângulo normalizado (0–1) em relação ao quadro inteiro.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NormRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl NormRect {
    pub fn clamp(self) -> Self {
        let w = self.w.clamp(0.02, 1.0);
        let h = self.h.clamp(0.02, 1.0);
        Self { x: self.x.clamp(0.0, 1.0 - w), y: self.y.clamp(0.0, 1.0 - h), w, h }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Facecam {
    /// Caixa da webcam.
    pub rect: NormRect,
    /// Centro do rosto (normalizado), para centralizar o recorte.
    pub face_x: f64,
    pub face_y: f64,
    /// `true` se as bordas da moldura foram encontradas (senão a caixa foi estimada pelo rosto).
    pub framed: bool,
}

/// Quadros em tons de cinza, todos do mesmo tamanho.
pub struct Frames {
    pub w: usize,
    pub h: usize,
    pub data: Vec<Vec<u8>>,
}

/// Tempo máximo da detecção inteira; depois disso usamos o que já foi analisado.
const DEADLINE: std::time::Duration = std::time::Duration::from_secs(120);

/// Lê `SAMPLES` quadros espalhados em `[start, start+duration)`.
#[allow(clippy::too_many_arguments)]
pub fn sample_frames(
    ffmpeg: &Path,
    input: &Path,
    src_w: u32,
    src_h: u32,
    start: f64,
    duration: f64,
    cancel: &Cancel,
    on_frame: &mut dyn FnMut(usize),
) -> Result<Frames> {
    let w = ANALYSIS_WIDTH.min(src_w) & !1;
    let h = ((src_h as f64 * w as f64 / src_w as f64).round() as u32).max(2) & !1;
    let fps = (SAMPLES as f64 / duration.max(0.5)).max(0.05);
    let mut cmd = command(ffmpeg);
    cmd.args(["-v", "error", "-ss", &format!("{start:.3}"), "-t", &format!("{duration:.3}"), "-i"])
        .arg(input)
        .args(["-an", "-sn", "-vf", &format!("fps={fps:.5},scale={w}:{h},format=gray"), "-frames:v", &SAMPLES.to_string()])
        .args(["-f", "rawvideo", "-pix_fmt", "gray", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().context("não foi possível executar o ffmpeg")?;
    let err = drain_stderr(&mut child);
    let mut out = child.stdout.take().unwrap();
    let len = (w * h) as usize;
    // Leitura numa thread, para podermos desistir se o ffmpeg travar.
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || loop {
        let mut buf = vec![0u8; len];
        if out.read_exact(&mut buf).is_err() || tx.send(buf).is_err() {
            break;
        }
    });
    let started = std::time::Instant::now();
    let mut data = Vec::new();
    loop {
        if cancel.is_cancelled() {
            kill(&mut child);
            anyhow::bail!(Cancelled);
        }
        match rx.recv_timeout(std::time::Duration::from_millis(200)) {
            Ok(f) => {
                data.push(f);
                on_frame(data.len());
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if started.elapsed() > DEADLINE / 2 {
                    kill(&mut child);
                    break;
                }
            }
        }
    }
    let _ = child.wait();
    if data.is_empty() {
        anyhow::bail!("não foi possível ler quadros do vídeo:\n{}", err.join().unwrap_or_default());
    }
    Ok(Frames { w: w as usize, h: h as usize, data })
}

/// Detecta a webcam num trecho do vídeo. `on_progress` recebe 0.0–1.0.
#[allow(clippy::too_many_arguments)]
pub fn detect_in_video(
    ffmpeg: &Path,
    input: &Path,
    src_w: u32,
    src_h: u32,
    start: f64,
    duration: f64,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(f32),
) -> Result<Option<Facecam>> {
    let started = std::time::Instant::now();
    let frames = sample_frames(ffmpeg, input, src_w, src_h, start, duration, cancel, &mut |n| {
        on_progress(0.3 * n as f32 / SAMPLES as f32)
    })?;
    let mut det = new_detector()?;
    det.set_min_face_size(20);
    let mut faces = Vec::with_capacity(frames.data.len());
    for (i, f) in frames.data.iter().enumerate() {
        cancel.check()?;
        // Garante que a detecção nunca prende o trabalho: usa o que já analisou.
        if started.elapsed() > DEADLINE && faces.len() >= 6 {
            break;
        }
        faces.push(detect(det.as_mut(), f, frames.w as u32, frames.h as u32));
        on_progress(0.3 + 0.6 * (i + 1) as f32 / frames.data.len() as f32);
    }
    let analysed = Frames { w: frames.w, h: frames.h, data: frames.data[..faces.len()].to_vec() };
    let cam = detect_facecam(&analysed, &faces);
    on_progress(1.0);
    Ok(cam)
}

/// Núcleo da detecção (separado para testes).
pub fn detect_facecam(frames: &Frames, faces_per_frame: &[Vec<Face>]) -> Option<Facecam> {
    let (fx, fy, fw) = stable_face(faces_per_frame)?;
    let (w, h) = (frames.w, frames.h);
    // Rosto em pixels da análise.
    let px = fx * w as f64;
    let py = fy * h as f64;
    let pw = (fw * w as f64).max(8.0);

    let (vmed, hmed) = persistent_edges(frames);
    let v_at = |x: usize, y: usize| vmed[y * w + x] as f64;
    let h_at = |x: usize, y: usize| hmed[y * w + x] as f64;

    let band = |c: f64, r: f64, max: usize| -> (usize, usize) {
        let a = (c - r).max(1.0) as usize;
        let b = ((c + r) as usize).min(max - 2);
        (a, b.max(a + 1))
    };
    // Faixas onde procurar as linhas (em volta do rosto).
    let (y0, y1) = band(py, pw * 1.3, h);
    let (x0, x1) = band(px, pw * 1.6, w);
    let col = |x: usize| (y0..y1).map(|y| v_at(x, y)).sum::<f64>() / (y1 - y0) as f64;
    let row = |y: usize| (x0..x1).map(|x| h_at(x, y)).sum::<f64>() / (x1 - x0) as f64;

    // Procura a linha mais forte num intervalo; aceita se se destacar da média.
    let best = |range: Vec<usize>, score: &dyn Fn(usize) -> f64| -> Option<usize> {
        if range.is_empty() {
            return None;
        }
        let scores: Vec<(usize, f64)> = range.iter().map(|&i| (i, score(i))).collect();
        let mean = scores.iter().map(|s| s.1).sum::<f64>() / scores.len() as f64;
        let (i, s) = scores.into_iter().max_by(|a, b| a.1.total_cmp(&b.1))?;
        (s >= 14.0 && s >= mean * 3.0).then_some(i)
    };
    let lim = |v: f64, max: usize| v.clamp(1.0, (max - 2) as f64) as usize;

    let left_range: Vec<usize> = (lim(px - pw * 6.0, w)..lim(px - pw * 0.9, w)).collect();
    let right_range: Vec<usize> = (lim(px + pw * 0.9, w)..lim(px + pw * 6.0, w)).collect();
    let top_range: Vec<usize> = (lim(py - pw * 4.5, h)..lim(py - pw * 0.9, h)).collect();
    let bottom_range: Vec<usize> = (lim(py + pw * 1.2, h)..lim(py + pw * 5.5, h)).collect();

    let near_edge = |v: f64, max: f64| v <= 1.5 || v >= max - 1.5;
    // Se o intervalo encosta na borda do quadro e não achou linha, a webcam encosta na borda.
    let side = |found: Option<usize>, range: &Vec<usize>, edge: f64, max: usize| -> Option<f64> {
        found.map(|v| v as f64).or_else(|| {
            let touches = range.first().is_some_and(|&a| near_edge(a as f64 - 1.0, max as f64))
                || range.last().is_some_and(|&b| near_edge(b as f64 + 2.0, max as f64));
            touches.then_some(edge)
        })
    };
    let l = side(best(left_range.clone(), &col), &left_range, 0.0, w);
    let r = side(best(right_range.clone(), &col), &right_range, w as f64, w);
    let t = side(best(top_range.clone(), &row), &top_range, 0.0, h);
    let b = side(best(bottom_range.clone(), &row), &bottom_range, h as f64, h);

    let framed_rect = match (l, r, t, b) {
        (Some(l), Some(r), Some(t), Some(b)) => {
            let (bw, bh) = (r - l, b - t);
            let aspect = bw / bh.max(1.0);
            let face_ratio = pw / bw.max(1.0);
            ((0.7..=2.6).contains(&aspect) && (0.08..=0.6).contains(&face_ratio)).then_some((l, t, bw, bh))
        }
        _ => None,
    };
    let (rect, framed) = match framed_rect {
        Some((l, t, bw, bh)) => (NormRect { x: l / w as f64, y: t / h as f64, w: bw / w as f64, h: bh / h as f64 }, true),
        None => {
            // Sem moldura: caixa 16:9 em volta do rosto (cabeça e ombros).
            let bw = pw * 5.0;
            let bh = bw * 9.0 / 16.0;
            let x = px - bw / 2.0;
            let y = py - bh * 0.42;
            (NormRect { x: x / w as f64, y: y / h as f64, w: bw / w as f64, h: bh / h as f64 }.clamp(), false)
        }
    };
    Some(Facecam { rect: rect.clamp(), face_x: fx, face_y: fy, framed })
}

/// O rosto que aparece no mesmo lugar na maioria dos quadros: (cx, cy, largura), normalizados.
pub fn stable_face(faces_per_frame: &[Vec<Face>]) -> Option<(f64, f64, f64)> {
    let n = faces_per_frame.len();
    if n == 0 {
        return None;
    }
    let all: Vec<&Face> = faces_per_frame.iter().flatten().collect();
    let mut best: Option<(usize, f64, (f64, f64, f64))> = None;
    for cand in &all {
        let mut frames_hit = 0;
        let (mut sx, mut sy, mut sw) = (0.0, 0.0, 0.0);
        for frame in faces_per_frame {
            if let Some(m) = frame.iter().find(|f| {
                (f.cx - cand.cx).abs() < cand.w * 0.6 && (f.cy - cand.cy).abs() < cand.w * 0.9 && (0.7..1.4).contains(&(f.w / cand.w))
            }) {
                frames_hit += 1;
                sx += m.cx;
                sy += m.cy;
                sw += m.w;
            }
        }
        let k = frames_hit as f64;
        let avg = (sx / k, sy / k, sw / k);
        let better = match &best {
            None => true,
            Some((h, w, _)) => frames_hit > *h || (frames_hit == *h && avg.2 > *w),
        };
        if frames_hit > 0 && better {
            best = Some((frames_hit, avg.2, avg));
        }
    }
    let (hits, _, avg) = best?;
    let needed = 3.max((n as f64 * 0.35).ceil() as usize).min(n);
    (hits >= needed).then_some(avg)
}

/// Mediana, entre os quadros, do gradiente vertical e horizontal de cada pixel.
fn persistent_edges(frames: &Frames) -> (Vec<u8>, Vec<u8>) {
    let (w, h, n) = (frames.w, frames.h, frames.data.len());
    let mut v = vec![0u8; w * h];
    let mut hz = vec![0u8; w * h];
    let mut tmp_v = vec![0u8; n];
    let mut tmp_h = vec![0u8; n];
    for y in 1..h {
        for x in 1..w {
            let i = y * w + x;
            for (k, f) in frames.data.iter().enumerate() {
                tmp_v[k] = (f[i] as i16 - f[i - 1] as i16).unsigned_abs().min(255) as u8;
                tmp_h[k] = (f[i] as i16 - f[i - w] as i16).unsigned_abs().min(255) as u8;
            }
            tmp_v.sort_unstable();
            tmp_h.sort_unstable();
            v[i] = tmp_v[n / 2];
            hz[i] = tmp_h[n / 2];
        }
    }
    (v, hz)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quadros sintéticos: "jogo" com ruído que muda a cada quadro e uma "webcam"
    /// com moldura fixa no canto superior direito.
    fn synthetic(frames: usize, framed: bool) -> (Frames, Vec<Vec<Face>>, NormRect) {
        let (w, h) = (480usize, 270usize);
        let cam = (330usize, 20usize, 130usize, 74usize); // x, y, largura, altura
        let mut data = Vec::new();
        let mut faces = Vec::new();
        let mut seed = 12345u32;
        let mut rnd = || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (seed >> 16) as u8
        };
        for k in 0..frames {
            let mut f = vec![0u8; w * h];
            for p in f.iter_mut() {
                *p = 60 + rnd() % 120; // jogo: muda sempre
            }
            for y in cam.1..cam.1 + cam.3 {
                for x in cam.0..cam.0 + cam.2 {
                    let border = x == cam.0 || x == cam.0 + cam.2 - 1 || y == cam.1 || y == cam.1 + cam.3 - 1;
                    f[y * w + x] = if border && framed { 255 } else { 40 + ((x + y + k) % 7) as u8 };
                }
            }
            data.push(f);
            let mut fr = vec![Face { cx: 395.0 / w as f64, cy: 52.0 / h as f64, w: 22.0 / w as f64, score: 5.0 }];
            // Um "personagem" que passeia pela tela.
            fr.push(Face { cx: 0.1 + 0.03 * k as f64, cy: 0.6, w: 0.05, score: 3.0 });
            faces.push(fr);
        }
        let truth = NormRect { x: cam.0 as f64 / w as f64, y: cam.1 as f64 / h as f64, w: cam.2 as f64 / w as f64, h: cam.3 as f64 / h as f64 };
        (Frames { w, h, data }, faces, truth)
    }

    #[test]
    fn finds_framed_webcam_and_ignores_moving_faces() {
        let (frames, faces, truth) = synthetic(16, true);
        let cam = detect_facecam(&frames, &faces).expect("webcam");
        assert!(cam.framed, "deveria achar a moldura");
        let err = |a: f64, b: f64| (a - b).abs();
        assert!(err(cam.rect.x, truth.x) < 0.01 && err(cam.rect.y, truth.y) < 0.02, "{:?} vs {:?}", cam.rect, truth);
        assert!(err(cam.rect.w, truth.w) < 0.015 && err(cam.rect.h, truth.h) < 0.03, "{:?} vs {:?}", cam.rect, truth);
        assert!((cam.face_x - 395.0 / 480.0).abs() < 0.01);
    }

    #[test]
    fn estimates_box_without_frame() {
        let (frames, faces, _) = synthetic(16, false);
        let cam = detect_facecam(&frames, &faces).expect("webcam");
        // Sem moldura nítida ainda deve conter o rosto.
        let fx = 395.0 / 480.0;
        assert!(cam.rect.x < fx && cam.rect.x + cam.rect.w > fx);
    }

    #[test]
    fn no_stable_face_means_no_webcam() {
        let faces: Vec<Vec<Face>> = (0..12).map(|k| vec![Face { cx: 0.05 * k as f64, cy: 0.5, w: 0.05, score: 3.0 }]).collect();
        assert!(stable_face(&faces).is_none());
    }
}
