//! Transcrição com marcação de tempo palavra a palavra.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Frase (ou trecho) formada por palavras consecutivas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
    /// Índice da primeira palavra em `Transcript::words`.
    pub first_word: usize,
    /// Índice (exclusivo) da última palavra.
    pub end_word: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Transcript {
    pub language: Option<String>,
    pub words: Vec<Word>,
    pub segments: Vec<Segment>,
}

const MAX_SEGMENT_SECONDS: f64 = 18.0;
const MAX_SEGMENT_WORDS: usize = 45;
const PAUSE_BREAK: f64 = 0.9;

impl Transcript {
    /// Monta frases a partir das palavras (quebra em pontuação, pausas ou tamanho).
    pub fn from_words(mut words: Vec<Word>, language: Option<String>) -> Self {
        words.retain(|w| !w.text.trim().is_empty() && w.end >= w.start);
        for w in &mut words {
            w.text = w.text.trim().to_string();
        }
        words.sort_by(|a, b| a.start.total_cmp(&b.start));
        let mut segments = Vec::new();
        let mut first = 0;
        for i in 0..words.len() {
            let w = &words[i];
            let next_gap = words.get(i + 1).map(|n| n.start - w.end).unwrap_or(f64::MAX);
            let ends_sentence = w.text.ends_with(['.', '!', '?', '…']);
            let too_long = w.end - words[first].start >= MAX_SEGMENT_SECONDS
                || i + 1 - first >= MAX_SEGMENT_WORDS;
            if ends_sentence || next_gap > PAUSE_BREAK || too_long || i + 1 == words.len() {
                segments.push(Segment {
                    start: words[first].start,
                    end: w.end,
                    text: join_words(&words[first..=i]),
                    first_word: first,
                    end_word: i + 1,
                });
                first = i + 1;
            }
        }
        Self { language, words, segments }
    }

    pub fn duration(&self) -> f64 {
        self.words.last().map(|w| w.end).unwrap_or(0.0)
    }

    /// Palavras que começam dentro de `[start, end)`.
    pub fn words_between(&self, start: f64, end: f64) -> &[Word] {
        let a = self.words.partition_point(|w| w.start < start - 0.05);
        let b = self.words.partition_point(|w| w.start < end);
        &self.words[a..b.max(a)]
    }

    pub fn text_between(&self, start: f64, end: f64) -> String {
        join_words(self.words_between(start, end))
    }

    /// Texto no formato usado no prompt: `[12.3-18.9] frase`.
    pub fn prompt_lines(&self, start: f64, end: f64) -> String {
        let mut out = String::new();
        for s in self.segments.iter().filter(|s| s.end > start && s.start < end) {
            out.push_str(&format!("[{:.1}-{:.1}] {}\n", s.start, s.end, s.text));
        }
        out
    }
}

pub fn join_words(words: &[Word]) -> String {
    let mut s = String::new();
    for w in words {
        let attach = w.text.starts_with([',', '.', '!', '?', ';', ':', '…', ')', '%'])
            || w.text.starts_with('\'') && !s.is_empty();
        if !s.is_empty() && !attach {
            s.push(' ');
        }
        s.push_str(&w.text);
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
    fn splits_sentences_and_pauses() {
        let t = Transcript::from_words(
            vec![
                w(0.0, 0.3, "Olá"),
                w(0.3, 0.6, "pessoal."),
                w(0.7, 1.0, "Hoje"),
                w(1.0, 1.2, "vamos"),
                w(3.0, 3.3, "falar"),
            ],
            None,
        );
        assert_eq!(t.segments.len(), 3);
        assert_eq!(t.segments[0].text, "Olá pessoal.");
        assert_eq!(t.segments[1].text, "Hoje vamos");
        assert_eq!(t.segments[2].first_word, 4);
        assert_eq!(t.text_between(0.7, 3.1), "Hoje vamos falar");
    }
}
