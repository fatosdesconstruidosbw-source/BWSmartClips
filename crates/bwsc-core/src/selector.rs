//! Seleção de cortes em duas etapas, pensada para IA local:
//!
//! 1. **Regras (Rust, segundos):** monta trechos candidatos já encaixados em
//!    frases e dá uma nota por sinais simples (gancho, ritmo, volume, palavras de impacto…).
//! 2. **IA local:** avalia só os melhores, um por vez, em prompts curtos com
//!    resposta presa a um JSON Schema — e pode aparar frases fracas no início/fim.
//!
//! Sem IA disponível, a etapa 1 sozinha já produz os cortes.

use anyhow::Result;
use serde_json::{json, Value};

use crate::clips::{refine, ClipPlan};
use crate::config::ClipOptions;
use crate::llm::Candidate;
use crate::local_ai::JsonChat;
use crate::media::LOUDNESS_STEP;
use crate::process::{is_cancelled_error, Cancel};
use crate::transcribe::truncate;
use crate::transcript::{Segment, Transcript};

/// Trecho candidato vindo das regras.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    /// Primeiro e último segmento (inclusive).
    pub si: usize,
    pub ei: usize,
    pub start: f64,
    pub end: f64,
    /// Nota das regras, 0–100.
    pub score: f64,
    pub signals: Vec<&'static str>,
}

// Palavras sem acento e em minúsculas.
const HOOK_WORDS: &[&str] = &[
    "voce sabia", "ninguem", "nunca", "segredo", "verdade", "por que", "porque ninguem", "o maior", "o pior",
    "o melhor", "erro", "cuidado", "pare de", "se voce", "imagina", "olha so", "sabe o que", "a real", "eu descobri",
    "o problema", "presta atencao", "ja pensou", "vou te contar", "a maioria", "ninguem fala",
    "the truth", "nobody", "never", "secret", "why", "biggest", "worst", "mistake", "stop", "imagine", "here's",
    "most people", "nadie", "secreto", "verdad", "el peor", "el mejor",
];
const IMPACT_WORDS: &[&str] = &[
    "incrivel", "absurdo", "chocante", "dinheiro", "milhao", "milhoes", "mil reais", "morreu", "morte", "medo",
    "odio", "amor", "louco", "loucura", "insano", "surreal", "bizarro", "impossivel", "mentira", "polemica",
    "briga", "chorei", "chorando", "demitido", "rico", "pobre", "falencia", "sucesso", "fracasso", "perdi", "ganhei",
    "crazy", "insane", "money", "million", "died", "fear", "love", "hate", "impossible", "lie", "fired", "rich",
    "increible", "dinero", "miedo", "loco",
];
const LAUGH_WORDS: &[&str] = &["kkk", "haha", "hahaha", "rsrs", "risos", "risadas", "laughs", "laughter", "jaja"];
const CONNECTORS: &[&str] = &[
    "e", "mas", "entao", "tipo", "ne", "ai", "ah", "eh", "uh", "hum", "bom", "so", "and", "but", "um", "uh", "y",
    "pero", "porque", "que", "isso", "dai", "enfim",
];
const FILLERS: &[&str] = &["tipo", "ne", "uh", "hum", "eh", "um", "uhm", "like", "basicamente", "assim"];

pub fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            'ñ' | 'Ñ' => 'n',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

/// Palavras "limpas" (sem pontuação) do texto já normalizado.
fn tokens(folded: &str) -> Vec<&str> {
    folded.split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|t| !t.is_empty()).collect()
}

fn count_phrases(folded: &str, list: &[&str]) -> usize {
    let padded = format!(" {} ", tokens(folded).join(" "));
    list.iter().filter(|p| padded.contains(&format!(" {p} "))).count()
}

/// Sinais pré-calculados por segmento.
struct SegStats {
    words: usize,
    question: bool,
    exclaim: bool,
    hooks: usize,
    impact: usize,
    laugh: bool,
    fillers: usize,
    digits: bool,
    starts_with_connector: bool,
    ends_clean: bool,
}

fn seg_stats(s: &Segment) -> SegStats {
    let folded = fold(&s.text);
    let toks = tokens(&folded);
    SegStats {
        words: s.end_word - s.first_word,
        question: s.text.contains('?'),
        exclaim: s.text.contains('!'),
        hooks: count_phrases(&folded, HOOK_WORDS),
        impact: count_phrases(&folded, IMPACT_WORDS),
        laugh: toks.iter().any(|t| LAUGH_WORDS.iter().any(|l| t.starts_with(l))),
        fillers: toks.iter().filter(|t| FILLERS.contains(t)).count(),
        digits: s.text.chars().any(|c| c.is_ascii_digit()),
        starts_with_connector: toks.first().is_some_and(|t| CONNECTORS.contains(t)),
        ends_clean: s.text.trim_end().ends_with(['.', '!', '?', '…']),
    }
}

fn mean_std(v: &[f64]) -> (f64, f64) {
    if v.is_empty() {
        return (0.0, 1.0);
    }
    let m = v.iter().sum::<f64>() / v.len() as f64;
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64;
    (m, var.sqrt().max(1e-6))
}

/// Etapa 1: gera e pontua candidatos, devolvendo os `k` melhores sem sobreposição.
pub fn heuristic_candidates(t: &Transcript, levels: Option<&[f32]>, opts: &ClipOptions, k: usize) -> Vec<Window> {
    let segs = &t.segments;
    if segs.is_empty() {
        return Vec::new();
    }
    let stats: Vec<SegStats> = segs.iter().map(seg_stats).collect();
    let rates: Vec<f64> = segs
        .iter()
        .zip(&stats)
        .filter(|(s, _)| s.end - s.start > 1.0)
        .map(|(s, st)| st.words as f64 / (s.end - s.start))
        .collect();
    let (rate_m, rate_sd) = mean_std(&rates);

    // Volume: médias com somas prefixadas.
    let (lvl_prefix, lvl_m, lvl_sd) = match levels {
        Some(l) if !l.is_empty() => {
            let mut p = Vec::with_capacity(l.len() + 1);
            p.push(0.0f64);
            for v in l {
                p.push(p.last().unwrap() + *v as f64);
            }
            let speech: Vec<f64> = l.iter().map(|v| *v as f64).filter(|v| *v > -50.0).collect();
            let (m, sd) = mean_std(&speech);
            (Some(p), m, sd)
        }
        _ => (None, 0.0, 1.0),
    };
    let level_avg = |a: f64, b: f64| -> Option<f64> {
        let p = lvl_prefix.as_ref()?;
        let n = p.len() - 1;
        let i = ((a / LOUDNESS_STEP) as usize).min(n);
        let j = ((b / LOUDNESS_STEP).ceil() as usize).clamp(i + 1, n.max(i + 1)).min(n);
        (j > i).then(|| (p[j] - p[i]) / (j - i) as f64)
    };

    let min = opts.min_duration.max(1.0);
    let max = opts.max_duration.max(min + 1.0);
    let target = crate::render::safe_clamp(35.0, min, max);

    let mut best_per_start: Vec<Window> = Vec::new();
    for si in 0..segs.len() {
        let mut best: Option<Window> = None;
        let mut words = 0usize;
        let mut impact = 0usize;
        let mut fillers = 0usize;
        let mut laugh = false;
        for ei in si..segs.len() {
            let st = &stats[ei];
            words += st.words;
            impact += st.impact;
            fillers += st.fillers;
            laugh |= st.laugh;
            let dur = segs[ei].end - segs[si].start;
            if dur > max {
                break;
            }
            if dur < min {
                continue;
            }
            let mut raw = 0.0;
            let mut signals = Vec::new();
            // Gancho (primeira frase, ou as duas primeiras se a primeira for curta).
            let first = &stats[si];
            let second = stats.get(si + 1).filter(|_| first.words < 6 && si < ei);
            if first.question || second.is_some_and(|s| s.question) {
                raw += 1.0;
                signals.push("começa com uma pergunta");
            } else if first.exclaim {
                raw += 0.5;
                signals.push("começa com uma exclamação");
            }
            let hooks = first.hooks + second.map(|s| s.hooks).unwrap_or(0);
            if hooks > 0 {
                raw += (0.5 * hooks as f64).min(1.5);
                signals.push("frase de gancho forte");
            }
            if first.digits {
                raw += 0.3;
                signals.push("número logo no início");
            }
            if first.starts_with_connector {
                raw -= 0.8;
            }
            let gap_before = if si == 0 { 1.0 } else { segs[si].start - segs[si - 1].end };
            if gap_before >= 0.5 {
                raw += 0.3;
            }
            // Final.
            raw += if st.ends_clean { 0.4 } else { -0.3 };
            // Ritmo de fala.
            let rate = words as f64 / dur.max(1.0);
            let rz = ((rate - rate_m) / rate_sd).clamp(-2.0, 2.0);
            raw += 0.4 * rz;
            if rz > 0.8 {
                signals.push("fala acelerada e enérgica");
            }
            // Volume.
            if let Some(avg) = level_avg(segs[si].start, segs[ei].end) {
                let ez = ((avg - lvl_m) / lvl_sd).clamp(-2.0, 2.0);
                raw += 0.5 * ez;
                if ez > 0.8 {
                    signals.push("voz mais alta/animada");
                }
            }
            // Palavras de impacto, risadas e vícios de linguagem.
            let density = impact as f64 / (words as f64 / 20.0).max(1.0);
            if impact > 0 {
                raw += 0.6 * density.min(1.5);
                signals.push("palavras de impacto");
            }
            if laugh {
                raw += 0.5;
                signals.push("tem risadas");
            }
            raw -= 0.6 * (fillers as f64 / (words as f64 / 15.0).max(1.0)).min(1.0);
            // Preferência de duração.
            raw -= 0.4 * (dur - target).abs() / target;

            let score = 100.0 / (1.0 + (-(raw - 0.8)).exp());
            if best.as_ref().is_none_or(|b| score > b.score) {
                best = Some(Window { si, ei, start: segs[si].start, end: segs[ei].end, score, signals });
            }
        }
        if let Some(b) = best {
            best_per_start.push(b);
        }
    }

    best_per_start.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Window> = Vec::new();
    for w in best_per_start {
        let clash = kept.iter().any(|k| {
            let inter = (w.end.min(k.end) - w.start.max(k.start)).max(0.0);
            inter > 0.25 * (w.end - w.start).min(k.end - k.start)
        });
        if !clash {
            kept.push(w);
        }
        if kept.len() >= k {
            break;
        }
    }
    kept
}

const SYSTEM_PROMPT: &str = "Você é um editor experiente de vídeos curtos virais (TikTok, Reels, YouTube Shorts). \
Você recebe um trecho de transcrição dividido em frases numeradas e avalia se ele funciona como um corte viral \
que faz sentido sozinho. Critérios: gancho forte nas primeiras frases, ideia completa, emoção, surpresa, humor, \
utilidade ou polêmica, e um final satisfatório. Seja exigente: trechos comuns recebem de 30 a 50; só trechos \
realmente fortes passam de 80. Você pode cortar frases fracas do começo (start_sentence) ou do fim (end_sentence) \
para melhorar o gancho e o final. Escreva reason, title e hashtags no MESMO idioma do trecho. \
reason: uma frase curta explicando a nota. title: chamativo, no máximo 70 caracteres. hashtags: de 3 a 5.";

fn schema(n: usize) -> Value {
    json!({
        "type": "object",
        "properties": {
            "reason": {"type": "string", "maxLength": 160},
            "score": {"type": "integer", "minimum": 0, "maximum": 100},
            "start_sentence": {"type": "integer", "minimum": 1, "maximum": n},
            "end_sentence": {"type": "integer", "minimum": 1, "maximum": n},
            "title": {"type": "string", "maxLength": 80},
            "hashtags": {"type": "array", "items": {"type": "string", "maxLength": 30}, "maxItems": 5}
        },
        "required": ["reason", "score", "start_sentence", "end_sentence", "title", "hashtags"]
    })
}

fn user_prompt(t: &Transcript, w: &Window, focus: &str) -> String {
    let mut s = String::new();
    if !focus.trim().is_empty() {
        s.push_str(&format!(
            "O criador quer cortes sobre: \"{}\". Dê nota baixa se o trecho não tiver relação com isso.\n\n",
            focus.trim()
        ));
    }
    s.push_str("Frases do trecho:\n");
    for (i, seg) in t.segments[w.si..=w.ei].iter().enumerate() {
        s.push_str(&format!("[{}] {}\n", i + 1, seg.text));
    }
    s
}

/// Etapa 2: avalia um candidato com a IA. Devolve o candidato ajustado.
fn judge(ai: &dyn JsonChat, t: &Transcript, w: &Window, opts: &ClipOptions) -> Result<Candidate> {
    let n = w.ei - w.si + 1;
    let v = ai.chat_json(SYSTEM_PROMPT, &user_prompt(t, w, &opts.focus), &schema(n), 400)?;
    let llm_score = v["score"].as_f64().unwrap_or(50.0).clamp(0.0, 100.0);
    let s = (v["start_sentence"].as_u64().unwrap_or(1) as usize).clamp(1, n);
    let e = (v["end_sentence"].as_u64().unwrap_or(n as u64) as usize).clamp(1, n);
    let (mut start, mut end) = (w.start, w.end);
    if e >= s {
        let a = t.segments[w.si + s - 1].start;
        let b = t.segments[w.si + e - 1].end;
        if b - a >= opts.min_duration * 0.8 && b - a <= opts.max_duration {
            start = a;
            end = b;
        }
    }
    let str_field = |k: &str| v[k].as_str().unwrap_or_default().trim().to_string();
    Ok(Candidate {
        start,
        end,
        title: truncate(&str_field("title"), 80),
        hook: t.text_between(start, start + 4.0),
        score: 0.7 * llm_score + 0.3 * w.score,
        reason: truncate(&str_field("reason"), 200),
        hashtags: v["hashtags"]
            .as_array()
            .map(|a| a.iter().filter_map(|h| h.as_str().map(str::to_string)).collect())
            .unwrap_or_default(),
    })
}

fn rules_only(t: &Transcript, w: &Window) -> Candidate {
    let first = &t.segments[w.si].text;
    let reason = if w.signals.is_empty() {
        "Seleção automática por regras".to_string()
    } else {
        let mut sig = w.signals.clone();
        sig.dedup();
        format!("Seleção automática: {}", sig.join(", "))
    };
    Candidate {
        start: w.start,
        end: w.end,
        title: truncate(first.trim_end_matches(['.', ',', ';']), 70),
        hook: first.clone(),
        score: w.score,
        reason,
        hashtags: Vec::new(),
    }
}

/// Quantas avaliações da IA deram certo.
#[derive(Debug, Default, Clone)]
pub struct AiReport {
    pub ok: usize,
    pub failed: usize,
    pub first_error: Option<String>,
}

/// Seleção completa com IA local (ou só regras, se `ai` for `None`).
#[allow(clippy::too_many_arguments)]
pub fn select(
    ai: Option<&dyn JsonChat>,
    t: &Transcript,
    levels: Option<&[f32]>,
    opts: &ClipOptions,
    candidates: usize,
    media_duration: f64,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(f32, &str),
) -> Result<(Vec<ClipPlan>, AiReport)> {
    let k = candidates.max(opts.clip_count + 2);
    let mut report = AiReport::default();
    let windows = heuristic_candidates(t, levels, opts, k);
    let mut out = Vec::with_capacity(windows.len());
    let total = windows.len().max(1);
    for (i, w) in windows.iter().enumerate() {
        cancel.check()?;
        let cand = match ai {
            Some(ai) => {
                on_progress(i as f32 / total as f32, &format!("{} avaliando trecho {}/{total}…", ai.label(), i + 1));
                match judge(ai, t, w, opts) {
                    Ok(c) => {
                        report.ok += 1;
                        c
                    }
                    Err(e) if is_cancelled_error(&e) => return Err(e),
                    // Uma falha pontual não derruba o trabalho: fica a nota das regras.
                    Err(e) => {
                        report.failed += 1;
                        report.first_error.get_or_insert_with(|| format!("{e:#}"));
                        rules_only(t, w)
                    }
                }
            }
            None => rules_only(t, w),
        };
        out.push(cand);
    }
    on_progress(1.0, "Análise concluída");
    Ok((refine(out, t, opts, media_duration), report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Word;
    use std::cell::RefCell;

    fn transcript(sentences: &[&str], secs_per_word: f64) -> Transcript {
        let mut words = Vec::new();
        let mut t = 0.0;
        for s in sentences {
            for w in s.split_whitespace() {
                words.push(Word { start: t, end: t + secs_per_word * 0.9, text: w.into() });
                t += secs_per_word;
            }
            t += 0.6;
        }
        Transcript::from_words(words, Some("pt".into()))
    }

    fn sample() -> Transcript {
        let mut s = vec!["então tipo é isso aí né pessoal vamos lá começar o vídeo de hoje."; 12];
        s.push("Você sabia que ninguém fala desse erro que custa milhões de reais?");
        s.push("Eu descobri isso depois de perder dinheiro e foi absurdo.");
        s.push("O segredo é simples e vou te contar agora mesmo sem enrolação.");
        s.push("Primeiro pare de fazer isso todo dia porque é o pior hábito.");
        s.push("Depois anota tudo e revisa no fim da semana com calma.");
        s.extend(vec!["e aí tipo né a gente vai falando umas coisas aqui assim."; 12]);
        transcript(&s, 0.35)
    }

    #[test]
    fn hook_section_wins() {
        let t = sample();
        let opts = ClipOptions { min_duration: 12.0, max_duration: 30.0, ..Default::default() };
        let w = heuristic_candidates(&t, None, &opts, 3);
        assert!(!w.is_empty());
        let best = &w[0];
        assert!(t.segments[best.si].text.starts_with("Você sabia"), "melhor: {}", t.segments[best.si].text);
        assert!(best.signals.contains(&"começa com uma pergunta"));
        for x in &w {
            let d = x.end - x.start;
            assert!((12.0..=30.0).contains(&d));
        }
    }

    struct FakeAi {
        calls: RefCell<usize>,
    }
    impl JsonChat for FakeAi {
        fn chat_json(&self, _s: &str, user: &str, schema: &Value, _m: u32) -> Result<Value> {
            *self.calls.borrow_mut() += 1;
            assert!(user.contains("[1]"));
            let n = schema["properties"]["end_sentence"]["maximum"].as_u64().unwrap();
            Ok(json!({"reason":"Gancho forte.","score":90,"start_sentence":1,"end_sentence":n,
                      "title":"O erro que custa milhões","hashtags":["dinheiro","#dicas"]}))
        }
        fn label(&self) -> String {
            "teste".into()
        }
    }

    #[test]
    fn two_stage_uses_ai_scores_and_titles() {
        let t = sample();
        let opts = ClipOptions { min_duration: 12.0, max_duration: 30.0, clip_count: 2, ..Default::default() };
        let ai = FakeAi { calls: RefCell::new(0) };
        let (plans, rep) = select(Some(&ai), &t, None, &opts, 4, t.duration(), &Cancel::new(), &mut |_, _| {}).unwrap();
        assert_eq!(*ai.calls.borrow(), 4);
        assert_eq!((rep.ok, rep.failed), (4, 0));
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].title, "O erro que custa milhões");
        assert_eq!(plans[0].hashtags, vec!["#dinheiro", "#dicas"]);
        assert!(plans[0].score > 60.0);
    }

    #[test]
    fn rules_only_without_ai() {
        let t = sample();
        let opts = ClipOptions { min_duration: 12.0, max_duration: 30.0, clip_count: 2, ..Default::default() };
        let (plans, _) = select(None, &t, None, &opts, 4, t.duration(), &Cancel::new(), &mut |_, _| {}).unwrap();
        assert!(!plans.is_empty());
        assert!(plans[0].reason.starts_with("Seleção automática"));
        assert!(plans[0].title.starts_with("Você sabia"));
    }

    #[test]
    fn folding() {
        assert_eq!(fold("Ninguém SABIA, né?"), "ninguem sabia, ne?");
        assert_eq!(count_phrases(&fold("Você sabia que NINGUÉM fala?"), HOOK_WORDS), 3);
    }
}
