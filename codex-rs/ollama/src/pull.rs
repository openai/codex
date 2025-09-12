use std::collections::HashMap;
use std::io;
use std::io::Write;

/// Events emitted while pulling a model from Ollama.
#[derive(Debug, Clone)]
pub enum PullEvent {
    /// A human-readable status message (e.g., "verifying", "writing").
    Status(String),
    /// Byte-level progress update for a specific layer digest.
    ChunkProgress {
        digest: String,
        total: Option<u64>,
        completed: Option<u64>,
    },
    /// The pull finished successfully.
    Success,

    /// Error event with a message.
    Error(String),
}

/// A simple observer for pull progress events. Implementations decide how to
/// render progress (CLI, TUI, logs, ...).
pub trait PullProgressReporter {
    fn on_event(&mut self, event: &PullEvent) -> io::Result<()>;
}

/// A minimal CLI reporter that writes inline progress to stderr.
pub struct CliProgressReporter {
    printed_header: bool,
    last_line_len: usize,
    last_completed_sum: u64,
    last_instant: std::time::Instant,
    totals_by_digest: HashMap<String, (u64, u64)>,
}

impl Default for CliProgressReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl CliProgressReporter {
    pub fn new() -> Self {
        Self {
            printed_header: false,
            last_line_len: 0,
            last_completed_sum: 0,
            last_instant: std::time::Instant::now(),
            totals_by_digest: HashMap::new(),
        }
    }
}

impl PullProgressReporter for CliProgressReporter {
    fn on_event(&mut self, event: &PullEvent) -> io::Result<()> {
        let mut out = std::io::stderr();
        match event {
            PullEvent::Status(status) => {
                // Avoid noisy manifest messages; otherwise show status inline.
                if status.eq_ignore_ascii_case("pulling manifest") {
                    return Ok(());
                }
                let pad = self.last_line_len.saturating_sub(status.len());
                let line = format!("\r{status}{}", " ".repeat(pad));
                self.last_line_len = status.len();
                out.write_all(line.as_bytes())?;
                out.flush()
            }
            PullEvent::ChunkProgress {
                digest,
                total,
                completed,
            } => {
                if let Some(t) = *total {
                    self.totals_by_digest
                        .entry(digest.clone())
                        .or_insert((0, 0))
                        .0 = t;
                }
                if let Some(c) = *completed {
                    self.totals_by_digest
                        .entry(digest.clone())
                        .or_insert((0, 0))
                        .1 = c;
                }

                let (sum_total, sum_completed) = self
                    .totals_by_digest
                    .values()
                    .fold((0u64, 0u64), |acc, (t, c)| (acc.0 + *t, acc.1 + *c));
                if sum_total > 0 {
                    if !self.printed_header {
                        let gb = (sum_total as f64) / (1024.0 * 1024.0 * 1024.0);
                        let header = format!("Downloading model: total {gb:.2} GB\n");
                        out.write_all(b"\r\x1b[2K")?;
                        out.write_all(header.as_bytes())?;
                        self.printed_header = true;
                    }
                    let now = std::time::Instant::now();
                    let dt = now
                        .duration_since(self.last_instant)
                        .as_secs_f64()
                        .max(0.001);
                    let dbytes = sum_completed.saturating_sub(self.last_completed_sum) as f64;
                    let speed_mb_s = dbytes / (1024.0 * 1024.0) / dt;
                    self.last_completed_sum = sum_completed;
                    self.last_instant = now;

                    let done_gb = (sum_completed as f64) / (1024.0 * 1024.0 * 1024.0);
                    let total_gb = (sum_total as f64) / (1024.0 * 1024.0 * 1024.0);
                    let pct = (sum_completed as f64) * 100.0 / (sum_total as f64);
                    let text =
                        format!("{done_gb:.2}/{total_gb:.2} GB ({pct:.1}%) {speed_mb_s:.1} MB/s");
                    let pad = self.last_line_len.saturating_sub(text.len());
                    let line = format!("\r{text}{}", " ".repeat(pad));
                    self.last_line_len = text.len();
                    out.write_all(line.as_bytes())?;
                    out.flush()
                } else {
                    Ok(())
                }
            }
            PullEvent::Error(_) => {
                // This will be handled by the caller, so we don't do anything
                // here or the error will be printed twice.
                Ok(())
            }
            PullEvent::Success => {
                out.write_all(b"\n")?;
                out.flush()
            }
        }
    }
}

/// For now the TUI reporter delegates to the CLI reporter. This keeps UI and
/// CLI behavior aligned until a dedicated TUI integration is implemented.
#[derive(Default)]
pub struct TuiProgressReporter(CliProgressReporter);

impl PullProgressReporter for TuiProgressReporter {
    fn on_event(&mut self, event: &PullEvent) -> io::Result<()> {
        self.0.on_event(event)
    }
}
