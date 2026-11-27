//! Execução de programas externos com cancelamento e leitura de saída linha a linha.

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};

/// Sinal de cancelamento compartilhado entre a interface e o trabalho.
#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            bail!(Cancelled)
        }
        Ok(())
    }
}

/// Erro usado quando o usuário cancela.
#[derive(Debug)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cancelado pelo usuário")
    }
}
impl std::error::Error for Cancelled {}

pub fn is_cancelled_error(e: &anyhow::Error) -> bool {
    e.downcast_ref::<Cancelled>().is_some()
}

/// Cria um `Command` sem abrir janela de console no Windows.
pub fn command(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Qual fluxo de saída gerou a linha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

enum Msg {
    Line(Stream, String),
    Closed,
}

fn pump<R: Read + Send + 'static>(r: R, stream: Stream, tx: mpsc::Sender<Msg>) {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(r);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            // Divide em '\n' e também em '\r' (barras de progresso).
            match read_until_any(&mut reader, &mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf).trim_end().to_string();
                    if !line.is_empty() && tx.send(Msg::Line(stream, line)).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(Msg::Closed);
    });
}

fn read_until_any<R: BufRead>(r: &mut R, buf: &mut Vec<u8>) -> std::io::Result<usize> {
    let mut total = 0;
    loop {
        let (done, used) = {
            let available = r.fill_buf()?;
            if available.is_empty() {
                return Ok(total);
            }
            match available.iter().position(|&b| b == b'\n' || b == b'\r') {
                Some(i) => {
                    buf.extend_from_slice(&available[..i]);
                    (true, i + 1)
                }
                None => {
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
            }
        };
        r.consume(used);
        total += used;
        if done {
            return Ok(total);
        }
    }
}

/// Resultado de um programa executado.
pub struct RunOutput {
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
}

/// Executa o comando, chamando `on_line` para cada linha de stdout/stderr.
/// Mata o processo se `cancel` for acionado. Falha se o código de saída não for 0.
pub fn run_lines(cmd: Command, cancel: &Cancel, on_line: impl FnMut(Stream, &str)) -> Result<RunOutput> {
    run_lines_watchdog(cmd, cancel, None, on_line)
}

/// O processo ficou tempo demais sem mostrar nenhum sinal de vida.
#[derive(Debug)]
pub struct Stalled {
    pub program: String,
    pub secs: u64,
    pub tail: String,
}
impl std::fmt::Display for Stalled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ficou {} s sem avançar e foi interrompido.\n{}", short_name(&self.program), self.secs, self.tail)
    }
}
impl std::error::Error for Stalled {}

/// Como [`run_lines`], mas mata o processo se ele passar `stall` sem escrever nenhuma linha.
pub fn run_lines_watchdog(
    mut cmd: Command,
    cancel: &Cancel,
    stall: Option<Duration>,
    mut on_line: impl FnMut(Stream, &str),
) -> Result<RunOutput> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .with_context(|| format!("não foi possível executar {program}"))?;
    let (tx, rx) = mpsc::channel();
    pump(child.stdout.take().unwrap(), Stream::Stdout, tx.clone());
    pump(child.stderr.take().unwrap(), Stream::Stderr, tx);

    let mut out = RunOutput { stdout: Vec::new(), stderr: Vec::new() };
    let mut open = 2;
    let mut last_activity = std::time::Instant::now();
    while open > 0 {
        if cancel.is_cancelled() {
            kill(&mut child);
            bail!(Cancelled);
        }
        if let Some(limit) = stall {
            if last_activity.elapsed() > limit {
                kill(&mut child);
                let tail: Vec<&str> = out.stderr.iter().rev().take(8).rev().map(String::as_str).collect();
                bail!(Stalled { program, secs: limit.as_secs(), tail: tail.join("\n") });
            }
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Msg::Line(s, line)) => {
                last_activity = std::time::Instant::now();
                on_line(s, &line);
                let v = if s == Stream::Stdout { &mut out.stdout } else { &mut out.stderr };
                v.push(line);
                if v.len() > 4000 {
                    v.drain(..2000);
                }
            }
            Ok(Msg::Closed) => open -= 1,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let status = loop {
        if cancel.is_cancelled() {
            kill(&mut child);
            bail!(Cancelled);
        }
        if let Some(s) = child.try_wait()? {
            break s;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if !status.success() {
        let tail: Vec<&str> = out.stderr.iter().rev().take(12).rev().map(String::as_str).collect();
        return Err(anyhow!(
            "{} terminou com erro ({}):\n{}",
            short_name(&program),
            status,
            tail.join("\n")
        ));
    }
    Ok(out)
}

/// Executa sem callback.
pub fn run(cmd: Command, cancel: &Cancel) -> Result<RunOutput> {
    run_lines(cmd, cancel, |_, _| {})
}

pub fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

pub fn short_name(program: &str) -> String {
    Path::new(program)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| program.to_string())
}

/// Lê o stderr de um processo em segundo plano, guardando as últimas linhas.
pub fn drain_stderr(child: &mut Child) -> std::thread::JoinHandle<String> {
    let stderr = child.stderr.take();
    std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut s) = stderr {
            let _ = s.read_to_string(&mut text);
        }
        let lines: Vec<&str> = text.lines().collect();
        let start = lines.len().saturating_sub(15);
        lines[start..].join("\n")
    })
}
