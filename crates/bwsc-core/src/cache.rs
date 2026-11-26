//! Cache de transcrições, análises e downloads, para não refazer trabalho pesado.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::config::cache_dir;

/// Hash FNV-1a de 64 bits (estável entre versões, ao contrário do `DefaultHasher`).
pub fn fnv(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Identifica um arquivo de vídeo pelo caminho, tamanho e data de modificação.
pub fn source_key(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let id = format!("{}|{}|{}", abs.display(), meta.len(), mtime);
    Some(format!("{:016x}", fnv(id.as_bytes())))
}

/// Chave curta para um conjunto de parâmetros serializáveis.
pub fn params_key<T: Serialize>(params: &T) -> String {
    let json = serde_json::to_vec(params).unwrap_or_default();
    format!("{:016x}", fnv(&json))
}

#[derive(Clone)]
pub struct Cache {
    root: PathBuf,
    enabled: bool,
}

impl Cache {
    pub fn new(enabled: bool) -> Self {
        Self { root: cache_dir(), enabled }
    }

    pub fn at(root: PathBuf, enabled: bool) -> Self {
        Self { root, enabled }
    }

    fn file(&self, kind: &str, key: &str) -> PathBuf {
        self.root.join(kind).join(format!("{key}.json"))
    }

    pub fn get<T: DeserializeOwned>(&self, kind: &str, key: &str) -> Option<T> {
        if !self.enabled {
            return None;
        }
        let text = std::fs::read_to_string(self.file(kind, key)).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn put<T: Serialize>(&self, kind: &str, key: &str, value: &T) {
        if !self.enabled {
            return;
        }
        let path = self.file(kind, key);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string(value) {
            let _ = std::fs::write(path, text);
        }
    }

    /// Vídeo já baixado deste link, se ainda existir.
    pub fn downloaded(&self, url: &str) -> Option<(PathBuf, String)> {
        let map: HashMap<String, (PathBuf, String)> = self.get("downloads", "index")?;
        map.get(url.trim()).filter(|(p, _)| p.is_file()).cloned()
    }

    pub fn remember_download(&self, url: &str, path: &Path, title: &str) {
        let mut map: HashMap<String, (PathBuf, String)> = self.get("downloads", "index").unwrap_or_default();
        map.retain(|_, (p, _)| p.is_file());
        map.insert(url.trim().to_string(), (path.to_path_buf(), title.to_string()));
        self.put("downloads", "index", &map);
    }

    /// Apaga todo o cache.
    pub fn clear(&self) -> std::io::Result<()> {
        if self.root.exists() {
            std::fs::remove_dir_all(&self.root)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_disabled() {
        let dir = std::env::temp_dir().join(format!("bwsc-cache-test-{}", std::process::id()));
        let c = Cache::at(dir.clone(), true);
        c.put("x", "k", &vec![1, 2, 3]);
        assert_eq!(c.get::<Vec<i32>>("x", "k"), Some(vec![1, 2, 3]));
        let off = Cache::at(dir.clone(), false);
        assert_eq!(off.get::<Vec<i32>>("x", "k"), None);

        let video = dir.join("v.mp4");
        std::fs::write(&video, b"abc").unwrap();
        c.remember_download("https://y/1", &video, "Título");
        assert_eq!(c.downloaded("https://y/1").map(|d| d.1), Some("Título".into()));
        std::fs::remove_file(&video).unwrap();
        assert!(c.downloaded("https://y/1").is_none());
        c.clear().unwrap();
    }

    #[test]
    fn keys_are_stable() {
        assert_eq!(fnv(b"abc"), 0xe71fa2190541574b);
        assert_eq!(params_key(&("a", 1)), params_key(&("a", 1)));
        assert_ne!(params_key(&("a", 1)), params_key(&("a", 2)));
    }
}
