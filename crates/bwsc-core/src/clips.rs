//! Ajuste fino dos cortes sugeridos: encaixe em frases, limites de duração e remoção de sobreposições.

use serde::{Deserialize, Serialize};

use crate::config::ClipOptions;
use crate::llm::Candidate;
use crate::transcript::Transcript;

/// Corte final, pronto para renderizar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClipPlan {
    pub start: f64,
    pub end: f64,
    pub title: String,
    pub hook: String,
    pub score: f64,
    pub reason: String,
    pub hashtags: Vec<String>,
}

impl ClipPlan {
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
}

const SNAP_WINDOW: f64 = 4.0;
const PAD_START: f64 = 0.12;
const PAD_END: f64 = 0.35;

pub fn refine(candidates: Vec<Candidate>, t: &Transcript, opts: &ClipOptions, media_duration: f64) -> Vec<ClipPlan> {
    let mut plans: Vec<ClipPlan> = candidates
        .into_iter()
        .filter_map(|c| refine_one(c, t, opts, media_duration))
        .collect();
    plans.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<ClipPlan> = Vec::new();
    for p in plans {
        let overlaps = kept.iter().any(|k| {
            let inter = (p.end.min(k.end) - p.start.max(k.start)).max(0.0);
            inter > 0.3 * p.duration().min(k.duration())
        });
        if !overlaps {
            kept.push(p);
        }
        if kept.len() >= opts.clip_count {
            break;
        }
    }
    kept
}

fn refine_one(c: Candidate, t: &Transcript, opts: &ClipOptions, media_duration: f64) -> Option<ClipPlan> {
    if !(c.start.is_finite() && c.end.is_finite()) || c.end <= c.start || t.segments.is_empty() {
        return None;
    }
    let segs = &t.segments;
    // Segmento inicial: o que contém o início ou o que começa mais perto dele.
    let si = segs
        .iter()
        .position(|s| s.start <= c.start + 0.25 && s.end > c.start + 0.25)
        .or_else(|| nearest(segs.iter().map(|s| s.start), c.start))?;
    if (segs[si].start - c.start).abs() > SNAP_WINDOW && !(segs[si].start <= c.start && segs[si].end > c.start) {
        return None;
    }
    // Segmento final: o que termina mais perto do fim sugerido (sem vir antes do inicial).
    let mut ei = nearest(segs.iter().map(|s| s.end), c.end)?.max(si);

    let min = opts.min_duration.max(1.0);
    let max = opts.max_duration.max(min + 1.0);
    let dur = |a: usize, b: usize| segs[b].end - segs[a].start;

    while ei > si && dur(si, ei) > max {
        ei -= 1;
    }
    while ei + 1 < segs.len() && dur(si, ei) < min && dur(si, ei + 1) <= max {
        ei += 1;
    }

    let mut start = segs[si].start;
    let mut end = segs[ei].end;
    if end - start > max {
        // Uma frase única longa demais: corta na última palavra que cabe.
        let words = t.words_between(start, start + max);
        end = words.iter().rev().find(|w| w.end - start <= max).map(|w| w.end).unwrap_or(start + max);
    }
    if end - start < min * 0.6 {
        return None;
    }
    start = (start - PAD_START).max(0.0);
    end = (end + PAD_END).min(media_duration);
    let hook = if c.hook.trim().is_empty() { segs[si].text.clone() } else { c.hook.trim().to_string() };
    let title = if c.title.trim().is_empty() { crate::transcribe::truncate(&hook, 60) } else { c.title.trim().to_string() };
    Some(ClipPlan {
        start,
        end,
        title,
        hook,
        score: c.score.clamp(0.0, 100.0),
        reason: c.reason.trim().to_string(),
        hashtags: c
            .hashtags
            .into_iter()
            .map(|h| {
                let h = h.trim().replace(' ', "");
                if h.starts_with('#') { h } else { format!("#{h}") }
            })
            .filter(|h| h.len() > 1)
            .collect(),
    })
}

fn nearest(values: impl Iterator<Item = f64>, target: f64) -> Option<usize> {
    values
        .enumerate()
        .min_by(|a, b| (a.1 - target).abs().total_cmp(&(b.1 - target).abs()))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Word;

    /// Frases de 5 s: "f0." de 0–5, "f1." de 5–10, ...
    fn transcript(n: usize) -> Transcript {
        let words = (0..n)
            .flat_map(|i| {
                let s = i as f64 * 5.0;
                vec![
                    Word { start: s, end: s + 2.4, text: format!("frase{i}") },
                    Word { start: s + 2.5, end: s + 4.9, text: format!("fim{i}.") },
                ]
            })
            .collect();
        Transcript::from_words(words, None)
    }

    fn cand(start: f64, end: f64, score: f64) -> Candidate {
        Candidate { start, end, title: String::new(), hook: String::new(), score, reason: String::new(), hashtags: vec!["viral".into()] }
    }

    #[test]
    fn snaps_to_sentences_and_respects_limits() {
        let t = transcript(40);
        let opts = ClipOptions { min_duration: 20.0, max_duration: 30.0, clip_count: 5, ..Default::default() };
        let plans = refine(
            vec![cand(11.0, 34.0, 90.0), cand(12.0, 30.0, 50.0), cand(100.0, 190.0, 70.0), cand(150.0, 155.0, 60.0)],
            &t,
            &opts,
            200.0,
        );
        // O segundo se sobrepõe ao primeiro e é descartado; o quarto é estendido até 20 s.
        assert_eq!(plans.len(), 3);
        let p = &plans[0];
        assert!((p.start - (10.0 - PAD_START)).abs() < 1e-9, "start {}", p.start);
        assert!((p.end - (34.9 + PAD_END)).abs() < 1e-9, "end {}", p.end);
        assert_eq!(p.hashtags, vec!["#viral"]);
        assert_eq!(p.title, "frase2 fim2.");
        for p in &plans {
            let core = p.duration() - PAD_START - PAD_END;
            assert!((19.0..=30.0 + 1e-9).contains(&core), "dur {core}");
        }
    }
}
