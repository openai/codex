use serde::Deserialize;
use serde::Serialize;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone)]
pub struct TraceConfig {
    pub path: Option<PathBuf>,
    pub redact: bool,
}

impl TraceConfig {
    // Enable by setting CODEX_TRACE_PATH=/path/to/trace.jsonl
    pub fn from_env() -> Self {
        let path = std::env::var_os("CODEX_TRACE_PATH").map(PathBuf::from);
        let redact = std::env::var("CODEX_TRACE_REDACT")
            .ok()
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(true);

        Self { path, redact }
    }

    pub fn is_enabled(&self) -> bool {
        self.path.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceEvent {
    RunStarted(RunStarted),
    RunFinished(RunFinished),
    ToolCallStarted(ToolCallStarted),
    ToolCallFinished(ToolCallFinished),
    ModelRequestStarted(ModelRequestStarted),
    ModelRequestFinished(ModelRequestFinished),
    MessageEmitted(MessageEmitted),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStarted {
    pub ts_ms: u64,
    pub run_id: String,
    pub mode: String, // "tui" | "exec" | "cli"
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFinished {
    pub ts_ms: u64,
    pub run_id: String,
    pub ok: bool,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallStarted {
    pub ts_ms: u64,
    pub run_id: String,
    pub tool: String,
    pub call_id: String,
    pub args_redacted: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFinished {
    pub ts_ms: u64,
    pub run_id: String,
    pub tool: String,
    pub call_id: String,
    pub ok: bool,
    pub result_redacted: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequestStarted {
    pub ts_ms: u64,
    pub run_id: String,
    pub request_id: String,
    pub model: Option<String>,
    pub input_redacted: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequestFinished {
    pub ts_ms: u64,
    pub run_id: String,
    pub request_id: String,
    pub ok: bool,
    pub output_redacted: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEmitted {
    pub ts_ms: u64,
    pub run_id: String,
    pub role: String, // "user" | "assistant" | "system"
    pub content_redacted: Option<String>,
}

pub struct TraceWriter {
    run_id: String,
    redact: bool,
    out: BufWriter<File>,
}

impl TraceWriter {
    pub fn open<P: AsRef<Path>>(path: P, run_id: String, redact: bool) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            run_id,
            redact,
            out: BufWriter::new(file),
        })
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn redact(&self) -> bool {
        self.redact
    }

    pub fn write_event(&mut self, event: TraceEvent) -> std::io::Result<()> {
        let line = serde_json::to_string(&event)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.out.write_all(line.as_bytes())?;
        self.out.write_all(b"\n")?;
        self.out.flush()?;
        Ok(())
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn redact_string(s: &str, enabled: bool) -> String {
    if enabled {
        "[REDACTED]".to_string()
    } else {
        s.to_string()
    }
}
