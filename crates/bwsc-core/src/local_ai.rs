//! IA local: llama.cpp (`llama-server`) iniciado sob demanda, ou um Ollama já instalado.
//!
//! As respostas são sempre forçadas a seguir um JSON Schema (gramática do
//! llama.cpp / `format` do Ollama), então modelos pequenos não "quebram" o formato.

use std::fs::File;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::config::{LlmProvider, Settings};
use crate::process::{command, kill, Cancel, Cancelled};
use crate::tools::Tools;
use crate::transcribe::truncate;

/// Algo que responde a um prompt com JSON válido para o schema.
pub trait JsonChat {
    fn chat_json(&self, system: &str, user: &str, schema: &Value, max_tokens: u32) -> Result<Value>;
    /// Nome para mostrar ao usuário.
    fn label(&self) -> String;
}

/// Inicia o backend local configurado (ou devolve erro explicando o que falta).
pub fn start_backend(settings: &Settings, tools: &Tools, work: &Path, cancel: &Cancel) -> Result<Box<dyn JsonChat>> {
    match settings.llm_provider {
        LlmProvider::Ollama => Ok(Box::new(Ollama::connect(settings)?)),
        _ => {
            let exe = tools.llama()?;
            let model = tools.llm_model()?;
            Ok(Box::new(LlamaServer::start(exe, model, settings.llm_context, settings.llm_threads, work, cancel)?))
        }
    }
}

fn threads(n: usize) -> usize {
    if n > 0 {
        n
    } else {
        // Núcleos físicos costumam render mais que lógicos na inferência.
        let logical = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        (logical / 2).clamp(2, 16)
    }
}

// ------------------------------------------------------------------ llama.cpp

pub struct LlamaServer {
    child: Child,
    base: String,
    log: PathBuf,
    client: reqwest::blocking::Client,
    model_name: String,
}

impl LlamaServer {
    pub fn start(exe: &Path, model: &Path, ctx: u32, n_threads: usize, work: &Path, cancel: &Cancel) -> Result<Self> {
        std::fs::create_dir_all(work)?;
        let port = TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
        let log = work.join("llama-server.log");
        let log_file = File::create(&log)?;
        let mut cmd = command(exe);
        cmd.arg("-m")
            .arg(model)
            .args(["-c", &ctx.max(2048).to_string(), "-t", &threads(n_threads).to_string()])
            .args(["--host", "127.0.0.1", "--port", &port.to_string(), "-np", "1", "--jinja"])
            .args(["--chat-template-kwargs", r#"{"enable_thinking":false}"#])
            .stdin(Stdio::null())
            .stdout(log_file.try_clone()?)
            .stderr(log_file);
        let child = cmd.spawn().context("não foi possível iniciar o llama-server")?;
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(900))
            .no_proxy()
            .build()?;
        let model_name = model.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let mut server = Self { child, base: format!("http://127.0.0.1:{port}"), log, client, model_name };
        server.wait_ready(cancel)?;
        Ok(server)
    }

    fn log_tail(&self) -> String {
        let text = std::fs::read_to_string(&self.log).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        lines[lines.len().saturating_sub(12)..].join("\n")
    }

    fn wait_ready(&mut self, cancel: &Cancel) -> Result<()> {
        let started = Instant::now();
        let probe = reqwest::blocking::Client::builder().timeout(Duration::from_secs(3)).no_proxy().build()?;
        loop {
            if cancel.is_cancelled() {
                kill(&mut self.child);
                bail!(Cancelled);
            }
            if let Some(status) = self.child.try_wait()? {
                bail!("o llama-server fechou ao carregar o modelo ({status}):\n{}", self.log_tail());
            }
            if let Ok(r) = probe.get(format!("{}/health", self.base)).send() {
                if r.status().is_success() {
                    return Ok(());
                }
            }
            if started.elapsed() > Duration::from_secs(600) {
                kill(&mut self.child);
                bail!("o llama-server demorou demais para carregar o modelo:\n{}", self.log_tail());
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }
}

impl Drop for LlamaServer {
    fn drop(&mut self) {
        kill(&mut self.child);
    }
}

impl LlamaServer {
    fn post(&self, path: &str, body: &Value) -> Result<Value> {
        let resp = self
            .client
            .post(format!("{}{path}", self.base))
            .json(body)
            .send()
            .with_context(|| format!("falha ao falar com o llama-server:\n{}", self.log_tail()))?;
        let status = resp.status();
        let text = resp.text()?;
        if !status.is_success() {
            bail!("llama-server respondeu {status}: {}", truncate(&text, 400));
        }
        Ok(serde_json::from_str(&text)?)
    }
}

impl JsonChat for LlamaServer {
    fn chat_json(&self, system: &str, user: &str, schema: &Value, max_tokens: u32) -> Result<Value> {
        let messages = json!([
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ]);
        // 1) Formata com o template do próprio modelo (sem "thinking").
        let templated = self.post(
            "/apply-template",
            &json!({"messages": messages, "chat_template_kwargs": {"enable_thinking": false}}),
        );
        match templated.ok().and_then(|v| v["prompt"].as_str().map(str::to_string)) {
            Some(prompt) => {
                // 2) Gera presa ao JSON Schema (gramática), sem depender do parser de chat.
                let v = self.post(
                    "/completion",
                    &json!({
                        "prompt": prompt,
                        "n_predict": max_tokens,
                        "temperature": 0.3,
                        "cache_prompt": true,
                        "json_schema": schema,
                    }),
                )?;
                parse_json_reply(v["content"].as_str().unwrap_or_default())
            }
            None => {
                // Versões antigas sem /apply-template: usa a API compatível com OpenAI.
                let v = self.post(
                    "/v1/chat/completions",
                    &json!({
                        "messages": messages,
                        "temperature": 0.3,
                        "max_tokens": max_tokens,
                        "chat_template_kwargs": {"enable_thinking": false},
                        "response_format": {"type": "json_schema", "json_schema": {"name": "resposta", "schema": schema}},
                    }),
                )?;
                parse_json_reply(v["choices"][0]["message"]["content"].as_str().unwrap_or_default())
            }
        }
    }

    fn label(&self) -> String {
        format!("IA local ({})", self.model_name)
    }
}

// ------------------------------------------------------------------ Ollama

pub struct Ollama {
    base: String,
    model: String,
    ctx: u32,
    client: reqwest::blocking::Client,
}

impl Ollama {
    pub fn connect(settings: &Settings) -> Result<Self> {
        let base = settings.ollama_base_url.trim_end_matches('/').to_string();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(900))
            .no_proxy()
            .build()?;
        let tags: Value = client
            .get(format!("{base}/api/tags"))
            .timeout(Duration::from_secs(5))
            .send()
            .and_then(|r| r.json())
            .with_context(|| format!("o Ollama não está rodando em {base}"))?;
        let model = settings.ollama_model.trim().to_string();
        let installed = tags["models"]
            .as_array()
            .map(|m| m.iter().any(|x| x["name"].as_str().is_some_and(|n| n == model || n == format!("{model}:latest"))))
            .unwrap_or(false);
        if !installed {
            bail!("o modelo \"{model}\" não está no Ollama — rode: ollama pull {model}");
        }
        Ok(Self { base, model, ctx: settings.llm_context, client })
    }
}

impl JsonChat for Ollama {
    fn chat_json(&self, system: &str, user: &str, schema: &Value, max_tokens: u32) -> Result<Value> {
        let body = json!({
            "model": self.model,
            "stream": false,
            "think": false,
            "format": schema,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ],
            "options": {"num_ctx": self.ctx, "temperature": 0.3, "num_predict": max_tokens},
        });
        let resp = self.client.post(format!("{}/api/chat", self.base)).json(&body).send().context("falha ao falar com o Ollama")?;
        let status = resp.status();
        let text = resp.text()?;
        if !status.is_success() {
            bail!("Ollama respondeu {status}: {}", truncate(&text, 400));
        }
        let v: Value = serde_json::from_str(&text)?;
        parse_json_reply(v["message"]["content"].as_str().unwrap_or_default())
    }

    fn label(&self) -> String {
        format!("Ollama ({})", self.model)
    }
}

/// Extrai o objeto JSON da resposta (tolerando texto ao redor).
pub fn parse_json_reply(content: &str) -> Result<Value> {
    let start = content.find('{').context("a IA não respondeu em JSON")?;
    let end = content.rfind('}').context("JSON incompleto")?;
    serde_json::from_str(&content[start..=end]).with_context(|| format!("JSON inválido: {}", truncate(content, 200)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Teste de integração com um llama-server real:
    /// `BWSC_LLAMA_SERVER=/caminho/llama-server BWSC_TEST_GGUF=/caminho/modelo.gguf cargo test -- --ignored`
    #[test]
    #[ignore]
    fn real_llama_server_returns_schema_json() {
        let (Ok(exe), Ok(model)) = (std::env::var("BWSC_LLAMA_SERVER"), std::env::var("BWSC_TEST_GGUF")) else {
            eprintln!("defina BWSC_LLAMA_SERVER e BWSC_TEST_GGUF");
            return;
        };
        let work = std::env::temp_dir().join(format!("bwsc-llama-{}", std::process::id()));
        let server = LlamaServer::start(Path::new(&exe), Path::new(&model), 2048, 2, &work, &Cancel::new()).unwrap();
        let schema = json!({
            "type": "object",
            "properties": {
                "score": {"type": "integer", "minimum": 0, "maximum": 100},
                "end_sentence": {"type": "integer", "minimum": 1, "maximum": 3},
                "title": {"type": "string", "maxLength": 20}
            },
            "required": ["score", "end_sentence", "title"]
        });
        let v = server.chat_json("Responda em JSON.", "[1] Olá.\n[2] Tudo bem?", &schema, 400).unwrap();
        let score = v["score"].as_i64().unwrap();
        assert!((0..=100).contains(&score), "{v}");
        assert!((1..=3).contains(&v["end_sentence"].as_i64().unwrap()), "{v}");
        // (o modelo de teste tem pesos aleatórios: o título pode vir "embaralhado")
        assert!(v["title"].is_string(), "{v}");
        eprintln!("resposta: {v}");
        drop(server);
        let _ = std::fs::remove_dir_all(&work);
    }
}
