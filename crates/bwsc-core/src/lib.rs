//! # bwsc-core
//!
//! Núcleo do BWSmartClips, um gerador de cortes virais no estilo Opus Clip:
//!
//! 1. baixa o vídeo (yt-dlp) ou usa um arquivo local;
//! 2. transcreve palavra a palavra (whisper.cpp local ou API OpenAI);
//! 3. escolhe os momentos com mais potencial viral — por padrão com IA local
//!    (llama.cpp, 100% offline), ou opcionalmente com Claude/OpenAI;
//! 4. ajusta os cortes às frases e às durações desejadas;
//! 5. renderiza em 9:16 seguindo o rosto, com legendas animadas.

pub mod cache;
pub mod captions;
pub mod clips;
pub mod config;
pub mod download;
pub mod face;
pub mod facecam;
pub mod installer;
pub mod llm;
pub mod local_ai;
pub mod media;
pub mod pipeline;
pub mod process;
pub mod render;
pub mod selector;
pub mod tools;
pub mod transcribe;
pub mod transcript;

pub use config::{ClipOptions, Settings};
pub use pipeline::{run_job, JobResult, Progress, Source, Stage};
pub use process::Cancel;
