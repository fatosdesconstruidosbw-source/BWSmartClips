//! Legendas animadas no estilo "palavra por palavra" geradas em ASS (libass).

use std::fmt::Write as _;

use crate::config::{CaptionPosition, CaptionStyle};
use crate::transcript::Word;

/// Fonte embutida (Montserrat ExtraBold, licença OFL).
pub static EMBEDDED_FONT: &[u8] = include_bytes!("../assets/Montserrat-ExtraBold.ttf");
pub const EMBEDDED_FONT_FILE: &str = "Montserrat-ExtraBold.ttf";

/// Converte `#RRGGBB` para a cor ASS `&H00BBGGRR`.
pub fn ass_color(hex: &str) -> String {
    let h = hex.trim().trim_start_matches('#');
    let (r, g, b) = if h.len() >= 6 {
        (&h[0..2], &h[2..4], &h[4..6])
    } else {
        ("FF", "FF", "FF")
    };
    let ok = |s: &str| u8::from_str_radix(s, 16).is_ok();
    if !(ok(r) && ok(g) && ok(b)) {
        return "&H00FFFFFF".into();
    }
    format!("&H00{}{}{}", b.to_uppercase(), g.to_uppercase(), r.to_uppercase())
}

fn ass_time(t: f64) -> String {
    let cs = (t.max(0.0) * 100.0).round() as u64;
    format!("{}:{:02}:{:02}.{:02}", cs / 360_000, (cs / 6000) % 60, (cs / 100) % 60, cs % 100)
}

fn clean(text: &str, upper: bool) -> String {
    let t: String = text.chars().filter(|c| !matches!(c, '{' | '}' | '\\')).collect();
    if upper {
        t.to_uppercase()
    } else {
        t
    }
}

/// Agrupa palavras em blocos curtos para exibição.
pub fn chunk_words(words: &[Word], style: &CaptionStyle) -> Vec<Vec<Word>> {
    let max_words = style.max_words.max(1);
    let max_chars = style.max_chars.max(6);
    let mut chunks: Vec<Vec<Word>> = Vec::new();
    let mut cur: Vec<Word> = Vec::new();
    let mut chars = 0;
    for w in words {
        let len = w.text.chars().count();
        let gap = cur.last().map(|l: &Word| w.start - l.end).unwrap_or(0.0);
        if !cur.is_empty() && (cur.len() >= max_words || chars + 1 + len > max_chars || gap > 0.6) {
            chunks.push(std::mem::take(&mut cur));
            chars = 0;
        }
        chars += len + usize::from(!cur.is_empty());
        cur.push(w.clone());
        if w.text.ends_with(['.', '!', '?', ',', ';', ':', '…']) {
            chunks.push(std::mem::take(&mut cur));
            chars = 0;
        }
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }
    chunks
}

/// Gera o arquivo .ass. `words` têm tempos relativos ao início do corte.
/// `divider`: no layout Gameplay, a altura (px) da divisa webcam/jogo — com a posição
/// "Embaixo", a legenda fica centralizada sobre essa divisa.
pub fn build_ass(words: &[Word], style: &CaptionStyle, width: u32, height: u32, clip_len: f64, divider: Option<u32>) -> String {
    let scale = width as f64 / 1080.0;
    let font_size = (style.font_size as f64 * scale).round() as u32;
    let outline = style.outline as f64 * scale;
    let shadow = style.shadow as f64 * scale;
    let (alignment, margin_v) = match (style.position, divider) {
        (CaptionPosition::Bottom, Some(d)) => (8, d.saturating_sub((font_size as f64 * 0.62) as u32)),
        (CaptionPosition::Top, _) => (8, (height as f64 * 0.16) as u32),
        (CaptionPosition::Middle, _) => (5, 0),
        (CaptionPosition::Bottom, None) => (2, (height as f64 * 0.24) as u32),
    };
    let margin_h = (60.0 * scale) as u32;
    let primary = ass_color(&style.color);
    let highlight = ass_color(&style.highlight_color);
    let outline_c = ass_color(&style.outline_color);
    let upcoming = (!style.upcoming_color.trim().is_empty()).then(|| ass_color(&style.upcoming_color));
    let boxed = !style.box_color.trim().is_empty();
    // Com caixa: BorderStyle 3 (caixa opaca); o "contorno" vira o respiro da caixa.
    let (border_style, back, outline_c, shadow) = if boxed {
        (3, ass_color(&style.box_color), ass_color(&style.box_color), 0.0)
    } else {
        (1, "&H80000000".to_string(), outline_c, shadow)
    };
    let italic = if style.italic { -1 } else { 0 };

    let mut s = String::new();
    let _ = writeln!(s, "[Script Info]\nScriptType: v4.00+\nPlayResX: {width}\nPlayResY: {height}\nWrapStyle: 0\nScaledBorderAndShadow: yes\nYCbCr Matrix: TV.709\n");
    let _ = writeln!(s, "[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding");
    let _ = writeln!(
        s,
        "Style: Default,{},{font_size},{primary},{primary},{outline_c},{back},-1,{italic},0,0,100,100,0,0,{border_style},{outline:.1},{shadow:.1},{alignment},{margin_h},{margin_h},{margin_v},1\n",
        style.font
    );
    let _ = writeln!(s, "[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text");

    let chunks = chunk_words(words, style);
    for (ci, chunk) in chunks.iter().enumerate() {
        let chunk_start = chunk[0].start.max(0.0);
        // Mantém o bloco na tela até o próximo começar (se a pausa for curta).
        let next_start = chunks.get(ci + 1).map(|n| n[0].start).unwrap_or(f64::MAX);
        let last_end = chunk.last().unwrap().end;
        let chunk_end = if next_start - last_end < 0.4 { next_start } else { last_end + 0.15 }.min(clip_len);
        for (wi, w) in chunk.iter().enumerate() {
            let ev_start = if wi == 0 { chunk_start } else { w.start };
            let ev_end = chunk.get(wi + 1).map(|n| n.start).unwrap_or(chunk_end).max(ev_start + 0.01);
            if ev_start >= clip_len {
                continue;
            }
            let mut text = String::new();
            if wi == 0 {
                text.push_str("{\\fad(70,0)\\fscx92\\fscy92\\t(0,90,\\fscx100\\fscy100)}");
            }
            for (k, other) in chunk.iter().enumerate() {
                if k > 0 {
                    text.push(' ');
                }
                let word = clean(&other.text, style.uppercase);
                if k == wi {
                    if style.pop {
                        text.push_str(&format!(
                            "{{\\c{highlight}\\fscx108\\fscy108\\t(0,80,\\fscx116\\fscy116)\\t(80,160,\\fscx108\\fscy108)}}{word}{{\\c{primary}\\fscx100\\fscy100}}"
                        ));
                    } else {
                        text.push_str(&format!("{{\\c{highlight}}}{word}{{\\c{primary}}}"));
                    }
                } else if k > wi && upcoming.is_some() {
                    let up = upcoming.as_deref().unwrap();
                    text.push_str(&format!("{{\\c{up}}}{word}{{\\c{primary}}}"));
                } else {
                    text.push_str(&word);
                }
            }
            let _ = writeln!(
                s,
                "Dialogue: 0,{},{},Default,,0,0,0,,{}",
                ass_time(ev_start),
                ass_time(ev_end.min(clip_len)),
                text
            );
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(start: f64, end: f64, t: &str) -> Word {
        Word { start, end, text: t.into() }
    }

    #[test]
    fn colors_and_time() {
        assert_eq!(ass_color("#FFE600"), "&H0000E6FF");
        assert_eq!(ass_color("lixo"), "&H00FFFFFF");
        assert_eq!(ass_time(3725.456), "1:02:05.46");
    }

    #[test]
    fn chunks_break_on_punctuation_and_limits() {
        let style = CaptionStyle { max_words: 3, ..Default::default() };
        let words = vec![w(0.0, 0.2, "isso"), w(0.2, 0.4, "é"), w(0.4, 0.6, "muito"), w(0.6, 0.8, "bom!"), w(0.8, 1.0, "agora")];
        let c = chunk_words(&words, &style);
        let sizes: Vec<usize> = c.iter().map(Vec::len).collect();
        assert_eq!(sizes, [3, 1, 1]);
    }

    #[test]
    fn one_event_per_word_with_highlight() {
        let style = CaptionStyle::default();
        let words = vec![w(0.0, 0.3, "olá"), w(0.3, 0.7, "{mundo}")];
        let ass = build_ass(&words, &style, 1080, 1920, 5.0, None);
        let events: Vec<&str> = ass.lines().filter(|l| l.starts_with("Dialogue:")).collect();
        assert_eq!(events.len(), 2);
        assert!(events[0].contains("OLÁ") && events[1].contains("MUNDO"));
        assert!(!events[1].contains("{mundo}"));
        assert!(events[1].starts_with("Dialogue: 0,0:00:00.30,0:00:00.85"));
        assert!(ass.contains("PlayResY: 1920"));
    }

    #[test]
    fn boxed_italic_with_upcoming_words() {
        let style = CaptionStyle {
            italic: true,
            box_color: "#FFFFFF".into(),
            upcoming_color: "#9A9A9A".into(),
            color: "#111111".into(),
            ..Default::default()
        };
        let words = vec![w(0.0, 0.3, "um"), w(0.3, 0.6, "dois"), w(0.6, 0.9, "tres")];
        let ass = build_ass(&words, &style, 1080, 1920, 5.0, None);
        let style_line = ass.lines().find(|l| l.starts_with("Style:")).unwrap();
        let f: Vec<&str> = style_line.split(',').collect();
        assert_eq!(f[7], "-1"); // negrito
        assert_eq!(f[8], "-1"); // itálico
        assert_eq!(f[15], "3"); // caixa opaca
        assert_eq!(f[6], "&H00FFFFFF"); // cor da caixa
        let first = ass.lines().find(|l| l.starts_with("Dialogue:")).unwrap();
        // Na 1ª palavra, as duas seguintes estão em cinza (ainda não faladas).
        assert_eq!(first.matches("&H009A9A9A").count(), 2);
    }

    #[test]
    fn gameplay_caption_sits_on_divider() {
        let style = CaptionStyle::default();
        let ass = build_ass(&[w(0.0, 0.3, "oi")], &style, 1080, 1920, 5.0, Some(730));
        let f: Vec<String> = ass.lines().find(|l| l.starts_with("Style:")).unwrap().split(',').map(String::from).collect();
        assert_eq!(f[18], "8"); // alinhado pelo topo
        assert_eq!(f[21], (730 - (82.0f64 * 0.62) as u32).to_string());
    }
}
