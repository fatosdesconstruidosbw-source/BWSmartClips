//! Renderização dos cortes: reenquadramento (rosto, centro, desfoque), legendas e codificação.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{bail, Context, Result};

use crate::captions::{build_ass, EMBEDDED_FONT, EMBEDDED_FONT_FILE};
use crate::config::{CaptionStyle, ReframeMode};
use crate::face;
use crate::facecam::{self, Facecam};
use crate::media::MediaInfo;
use crate::process::{command, drain_stderr, kill, run_lines, Cancel, Cancelled, Stream};
use crate::transcript::Word;

/// Altura máxima de processamento (o vídeo é decodificado nesta resolução).
const MAX_PROCESS_HEIGHT: u32 = 1080;

pub struct RenderJob<'a> {
    pub ffmpeg: &'a Path,
    pub input: &'a Path,
    pub info: &'a MediaInfo,
    pub start: f64,
    pub end: f64,
    /// Palavras com tempo relativo ao início do corte.
    pub words: &'a [Word],
    pub reframe: ReframeMode,
    pub captions: &'a CaptionStyle,
    /// Altura do vídeo vertical (1920 ou 1280).
    pub output_height: u32,
    pub x264_preset: &'a str,
    pub x264_crf: u32,
    /// Layout Gameplay: fração da altura para a webcam.
    pub cam_ratio: f64,
    /// Layout Gameplay: webcam marcada à mão (senão, detecta neste corte).
    pub facecam: Option<crate::facecam::NormRect>,
    /// Pasta temporária deste corte.
    pub work_dir: &'a Path,
    pub output: &'a Path,
}

fn even(v: f64) -> u32 {
    ((v.round() as u32).max(2)) & !1
}

/// Geometria calculada para um corte.
#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    /// Resolução em que a fonte é decodificada.
    pub proc_w: u32,
    pub proc_h: u32,
    /// Largura do recorte vertical (dentro de `proc_w`).
    pub crop_w: u32,
    /// Resolução final.
    pub out_w: u32,
    pub out_h: u32,
    /// A fonte é mais larga que 9:16 (precisa recortar).
    pub needs_crop: bool,
}

pub fn geometry(info: &MediaInfo, mode: ReframeMode, output_height: u32) -> Geometry {
    let proc_h = even(info.height.min(MAX_PROCESS_HEIGHT) as f64);
    let proc_w = even(info.width as f64 * proc_h as f64 / info.height as f64);
    if mode == ReframeMode::Original {
        return Geometry { proc_w, proc_h, crop_w: proc_w, out_w: proc_w, out_h: proc_h, needs_crop: false };
    }
    let out_h = even(output_height.clamp(640, 3840) as f64);
    let out_w = even(out_h as f64 * 9.0 / 16.0);
    let crop_w = even(proc_h as f64 * 9.0 / 16.0).min(proc_w);
    let needs_crop = proc_w as f64 / proc_h as f64 > 9.0 / 16.0 + 0.01;
    Geometry { proc_w, proc_h, crop_w, out_w, out_h, needs_crop }
}

fn output_fps(info: &MediaInfo) -> f64 {
    if (23.0..=61.0).contains(&info.fps) {
        (info.fps * 1000.0).round() / 1000.0
    } else {
        30.0
    }
}

/// Renderiza o corte. `on_progress` recebe 0.0–1.0.
pub fn render(job: &RenderJob, cancel: &Cancel, on_progress: &mut dyn FnMut(f32, &str)) -> Result<()> {
    std::fs::create_dir_all(job.work_dir)?;
    let g = geometry(job.info, job.reframe, job.output_height);
    let duration = job.end - job.start;

    if job.reframe == ReframeMode::Gameplay {
        on_progress(0.0, "Procurando a webcam…");
        let cam = match job.facecam {
            Some(rect) => Some(Facecam { rect: rect.clamp(), face_x: rect.x + rect.w / 2.0, face_y: rect.y + rect.h * 0.45, framed: true }),
            None => facecam::detect_in_video(job.ffmpeg, job.input, job.info.width, job.info.height, job.start, duration, cancel)?,
        };
        match cam {
            Some(cam) => {
                let plan = gameplay_plan(&g, &cam, job.cam_ratio);
                let subs = prepare_captions(job, &g, duration, Some(plan.top_h))?;
                return render_filter(job, &g, ReframeMode::Gameplay, Some(&plan), subs.as_deref(), cancel, &mut |p| {
                    on_progress(0.1 + p * 0.9, "Renderizando (gameplay)…")
                });
            }
            None => {
                // Trecho sem webcam: corta o centro, como em "Preencher".
                on_progress(0.1, "Webcam não encontrada neste corte — usando Preencher");
                let subs = prepare_captions(job, &g, duration, None)?;
                return render_filter(job, &g, ReframeMode::Center, None, subs.as_deref(), cancel, &mut |p| {
                    on_progress(0.1 + p * 0.9, "Renderizando…")
                });
            }
        }
    }

    let subtitle_filter = prepare_captions(job, &g, duration, None)?;

    let tracked = job.reframe == ReframeMode::Face && g.needs_crop;
    if tracked {
        on_progress(0.0, "Rastreando rosto…");
        let samples = face::track(
            job.ffmpeg,
            job.input,
            job.info.width,
            job.info.height,
            job.start,
            duration,
            cancel,
            |p| on_progress(p * 0.3, "Rastreando rosto…"),
        )?;
        let fps = output_fps(job.info);
        let frames = (duration * fps).ceil() as usize + 2;
        let path = face::camera_path(&samples, frames, fps, g.proc_w, g.crop_w);
        render_tracked(job, &g, fps, &path, subtitle_filter.as_deref(), cancel, &mut |p| {
            on_progress(0.3 + p * 0.7, "Renderizando…")
        })
    } else {
        render_filter(job, &g, job.reframe, None, subtitle_filter.as_deref(), cancel, &mut |p| on_progress(p, "Renderizando…"))
    }
}

/// Grava `captions.ass` e a fonte na pasta do corte. Devolve o filtro do ffmpeg.
fn prepare_captions(job: &RenderJob, g: &Geometry, duration: f64, divider: Option<u32>) -> Result<Option<String>> {
    if !job.captions.enabled || job.words.is_empty() {
        return Ok(None);
    }
    let fonts = job.work_dir.join("fonts");
    std::fs::create_dir_all(&fonts)?;
    let font_path = fonts.join(EMBEDDED_FONT_FILE);
    if !font_path.exists() {
        std::fs::write(&font_path, EMBEDDED_FONT)?;
    }
    let ass = build_ass(job.words, job.captions, g.out_w, g.out_h, duration, divider);
    std::fs::write(job.work_dir.join("captions.ass"), ass)?;
    // Caminhos relativos (o ffmpeg roda dentro de work_dir) evitam problemas
    // de escape com "C:\" no filtro de legendas no Windows.
    Ok(Some("subtitles=captions.ass:fontsdir=fonts".into()))
}

fn encoder_args(job: &RenderJob) -> Vec<String> {
    [
        "-c:v", "libx264", "-preset", job.x264_preset, "-crf", &job.x264_crf.to_string(),
        "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k", "-ar", "48000", "-movflags", "+faststart",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn absolute(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Recortes do layout Gameplay (em pixels da resolução de processamento).
#[derive(Debug, Clone, PartialEq)]
pub struct GameplayPlan {
    /// Recorte da webcam: x, y, largura, altura.
    pub cam: (u32, u32, u32, u32),
    /// Recorte do jogo: x, y, largura, altura.
    pub game: (u32, u32, u32, u32),
    /// Altura da webcam no vídeo final (o jogo ocupa o resto).
    pub top_h: u32,
}

pub fn gameplay_plan(g: &Geometry, cam: &Facecam, ratio: f64) -> GameplayPlan {
    let (pw, ph, ow, oh) = (g.proc_w as f64, g.proc_h as f64, g.out_w as f64, g.out_h as f64);
    let top_h = even(oh * ratio.clamp(0.25, 0.55));
    let bot_h = g.out_h - top_h;

    // Webcam: ajusta a caixa à proporção do espaço de cima, sem sair da caixa.
    let r = cam.rect.clamp();
    let (bx, by, bw, bh) = (r.x * pw, r.y * ph, (r.w * pw).max(4.0), (r.h * ph).max(4.0));
    let target = ow / top_h as f64;
    let (cw, ch) = if bw / bh > target { (bh * target, bh) } else { (bw, bw / target) };
    let cx = (cam.face_x * pw - cw / 2.0).clamp(bx, bx + bw - cw);
    let cy = (cam.face_y * ph - ch * 0.45).clamp(by, by + bh - ch);
    let (cw, ch) = (even(cw).min(g.proc_w), even(ch).min(g.proc_h));
    let cx = (cx.round() as u32).min(g.proc_w - cw);
    let cy = (cy.round() as u32).min(g.proc_h - ch);

    // Jogo: recorte central na proporção do espaço de baixo.
    let target = ow / bot_h as f64;
    let (gw, gh) = if pw / ph > target { (even(ph * target), g.proc_h) } else { (g.proc_w, even(pw / target)) };
    let (gw, gh) = (gw.min(g.proc_w), gh.min(g.proc_h));
    GameplayPlan { cam: (cw, ch, cx, cy), game: (gw, gh, (g.proc_w - gw) / 2, (g.proc_h - gh) / 2), top_h }
}

/// Caminho só com filtros do ffmpeg (centro, desfoque, original, gameplay ou vídeo já vertical).
#[allow(clippy::too_many_arguments)]
fn render_filter(
    job: &RenderJob,
    g: &Geometry,
    mode: ReframeMode,
    gameplay: Option<&GameplayPlan>,
    subs: Option<&str>,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(f32),
) -> Result<()> {
    let fps = output_fps(job.info);
    let (pw, ph, cw, ow, oh) = (g.proc_w, g.proc_h, g.crop_w, g.out_w, g.out_h);
    let base = format!("fps={fps},scale={pw}:{ph}");
    let mut chain = match mode {
        ReframeMode::Gameplay if gameplay.is_some() => {
            let p = gameplay.unwrap();
            let (cw, ch, cx, cy) = p.cam;
            let (gw, gh, gx, gy) = p.game;
            let (top, bot) = (p.top_h, oh - p.top_h);
            format!(
                "[0:v]{base},split[a][b];[a]crop={cw}:{ch}:{cx}:{cy},scale={ow}:{top}:flags=lanczos[top];\
                 [b]crop={gw}:{gh}:{gx}:{gy},scale={ow}:{bot}:flags=lanczos[bot];[top][bot]vstack"
            )
        }
        ReframeMode::Original => format!("[0:v]{base}"),
        ReframeMode::Blur => format!(
            "[0:v]{base},split[a][b];[a]scale={ow}:{oh}:force_original_aspect_ratio=increase,crop={ow}:{oh},boxblur=24:3,eq=brightness=-0.08[bg];\
             [b]scale={ow}:{oh}:force_original_aspect_ratio=decrease[fg];[bg][fg]overlay=(W-w)/2:(H-h)/2"
        ),
        _ if g.needs_crop => format!("[0:v]{base},crop={cw}:{ph}:{}:0,scale={ow}:{oh}:flags=lanczos", (pw - cw) / 2),
        _ => format!("[0:v]{base},scale={ow}:{oh}:force_original_aspect_ratio=increase:flags=lanczos,crop={ow}:{oh}"),
    };
    if let Some(s) = subs {
        chain.push(',');
        chain.push_str(s);
    }
    chain.push_str(",setsar=1[v]");

    let duration = job.end - job.start;
    let mut cmd = command(job.ffmpeg);
    cmd.current_dir(job.work_dir)
        .args(["-y", "-v", "error", "-nostats", "-progress", "pipe:1"])
        .args(["-ss", &format!("{:.3}", job.start), "-t", &format!("{duration:.3}"), "-i"])
        .arg(absolute(job.input))
        .args(["-filter_complex", &chain, "-map", "[v]", "-map", "0:a:0?"])
        .args(encoder_args(job))
        .arg(absolute(job.output));
    run_lines(cmd, cancel, |stream, line| {
        if stream == Stream::Stdout {
            if let Some(us) = line.strip_prefix("out_time_us=").or_else(|| line.strip_prefix("out_time_ms=")) {
                if let Ok(v) = us.trim().parse::<f64>() {
                    on_progress(((v / 1e6) / duration).clamp(0.0, 1.0) as f32);
                }
            }
        }
    })
    .context("falha ao renderizar o corte")?;
    Ok(())
}

/// Caminho com rastreamento: decodifica quadros, recorta em Rust seguindo o
/// caminho da câmera e envia ao codificador (que aplica escala e legendas).
fn render_tracked(
    job: &RenderJob,
    g: &Geometry,
    fps: f64,
    path: &[u32],
    subs: Option<&str>,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(f32),
) -> Result<()> {
    let duration = job.end - job.start;
    let (pw, ph, cw) = (g.proc_w as usize, g.proc_h as usize, g.crop_w as usize);
    let ss = format!("{:.3}", job.start);
    let tt = format!("{duration:.3}");

    let mut dec = command(job.ffmpeg);
    dec.args(["-v", "error", "-ss", &ss, "-t", &tt, "-i"])
        .arg(absolute(job.input))
        .args(["-an", "-vf", &format!("fps={fps},scale={pw}:{ph}"), "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut filter = format!("[0:v]scale={}:{}:flags=lanczos", g.out_w, g.out_h);
    if let Some(s) = subs {
        filter.push(',');
        filter.push_str(s);
    }
    filter.push_str(",setsar=1[v]");
    let mut enc = command(job.ffmpeg);
    enc.current_dir(job.work_dir)
        .args(["-y", "-v", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", &format!("{cw}x{ph}")])
        .args(["-r", &fps.to_string(), "-i", "-", "-ss", &ss, "-t", &tt, "-i"])
        .arg(absolute(job.input))
        .args(["-filter_complex", &filter, "-map", "[v]", "-map", "1:a:0?", "-shortest"])
        .args(encoder_args(job))
        .arg(absolute(job.output))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut decoder = dec.spawn().context("não foi possível iniciar o ffmpeg (decodificação)")?;
    let dec_err = drain_stderr(&mut decoder);
    let mut encoder = match enc.spawn() {
        Ok(e) => e,
        Err(e) => {
            kill(&mut decoder);
            return Err(e).context("não foi possível iniciar o ffmpeg (codificação)");
        }
    };
    let enc_err = drain_stderr(&mut encoder);
    let mut src = decoder.stdout.take().unwrap();
    let mut sink = encoder.stdin.take().unwrap();

    let frame_len = pw * ph * 3;
    let mut frame = vec![0u8; frame_len];
    let mut cropped = vec![0u8; cw * ph * 3];
    let expected = (duration * fps).ceil().max(1.0) as usize;
    let mut n = 0usize;
    let mut write_failed = false;
    loop {
        if cancel.is_cancelled() {
            kill(&mut decoder);
            kill(&mut encoder);
            bail!(Cancelled);
        }
        match src.read_exact(&mut frame) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => {
                kill(&mut decoder);
                kill(&mut encoder);
                return Err(e).context("erro lendo quadros do vídeo");
            }
        }
        let x = path.get(n).or(path.last()).copied().unwrap_or(0) as usize;
        let x = x.min(pw - cw);
        crop_rgb(&frame, pw, ph, x, cw, &mut cropped);
        if sink.write_all(&cropped).is_err() {
            write_failed = true;
            break;
        }
        n += 1;
        if n.is_multiple_of(5) {
            on_progress((n as f32 / expected as f32).min(0.99));
        }
    }
    drop(sink);
    if write_failed {
        kill(&mut decoder);
    }
    let dec_status = decoder.wait()?;
    let enc_status = encoder.wait()?;
    let dec_log = dec_err.join().unwrap_or_default();
    let enc_log = enc_err.join().unwrap_or_default();
    if !enc_status.success() || write_failed {
        bail!("o ffmpeg falhou ao codificar o corte:\n{enc_log}");
    }
    if n == 0 {
        bail!("nenhum quadro foi lido do vídeo ({dec_status}):\n{dec_log}");
    }
    on_progress(1.0);
    Ok(())
}

/// Copia a faixa vertical `[x, x+cw)` de um quadro RGB.
pub fn crop_rgb(frame: &[u8], w: usize, h: usize, x: usize, cw: usize, out: &mut [u8]) {
    let row = w * 3;
    let crow = cw * 3;
    for y in 0..h {
        let s = y * row + x * 3;
        out[y * crow..(y + 1) * crow].copy_from_slice(&frame[s..s + crow]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(w: u32, h: u32) -> MediaInfo {
        MediaInfo { duration: 60.0, width: w, height: h, fps: 29.97, has_audio: true }
    }

    #[test]
    fn geometry_landscape_4k() {
        let g = geometry(&info(3840, 2160), ReframeMode::Face, 1920);
        assert_eq!((g.proc_w, g.proc_h, g.crop_w, g.out_w, g.out_h), (1920, 1080, 608, 1080, 1920));
        assert!(g.needs_crop);
    }

    #[test]
    fn geometry_vertical_source() {
        let g = geometry(&info(1080, 1920), ReframeMode::Face, 1280);
        assert!(!g.needs_crop);
        assert_eq!((g.out_w, g.out_h), (720, 1280));
    }

    #[test]
    fn gameplay_plan_fits_webcam_and_game() {
        let info = info(1920, 1080);
        let g = geometry(&info, ReframeMode::Gameplay, 1920);
        // Webcam 16:9 no canto superior direito, rosto no meio dela.
        let cam = Facecam {
            rect: crate::facecam::NormRect { x: 0.78, y: 0.19, w: 0.21, h: 0.23 },
            face_x: 0.885,
            face_y: 0.29,
            framed: true,
        };
        let p = gameplay_plan(&g, &cam, 0.38);
        assert_eq!(p.top_h, 730);
        let (cw, ch, cx, cy) = p.cam;
        // Proporção do recorte da webcam ≈ proporção do espaço de cima (1080:730).
        assert!(((cw as f64 / ch as f64) - 1080.0 / 730.0).abs() < 0.03, "{cw}x{ch}");
        // Dentro da caixa da webcam.
        assert!(cx as f64 >= 0.78 * 1920.0 - 1.0 && (cx + cw) as f64 <= 0.99 * 1920.0 + 1.0);
        assert!(cy as f64 >= 0.19 * 1080.0 - 1.0 && (cy + ch) as f64 <= 0.42 * 1080.0 + 1.0);
        // Jogo: altura total, proporção do espaço de baixo, centralizado.
        let (gw, gh, gx, _) = p.game;
        assert_eq!(gh, 1080);
        assert!(((gw as f64 / gh as f64) - 1080.0 / 1190.0).abs() < 0.01);
        assert_eq!(gx, (1920 - gw) / 2);
    }

    #[test]
    fn crop_copies_columns() {
        // 4x2 pixels, valor = índice da coluna.
        let frame: Vec<u8> = (0..2).flat_map(|_| (0..4u8).flat_map(|c| [c, c, c])).collect();
        let mut out = vec![0u8; 2 * 2 * 3];
        crop_rgb(&frame, 4, 2, 1, 2, &mut out);
        assert_eq!(out, vec![1, 1, 1, 2, 2, 2, 1, 1, 1, 2, 2, 2]);
    }
}
