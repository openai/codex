use lsp_types::Diagnostic;
use lsp_types::DiagnosticSeverity;
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::Notify;
use tracing::warn;

#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    pub path: PathBuf,
    pub diagnostic: Diagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeverityFilter {
    Errors,
    ErrorsAndWarnings,
    All,
}

impl SeverityFilter {
    pub fn matches(&self, diagnostic: &Diagnostic) -> bool {
        match self {
            SeverityFilter::All => true,
            SeverityFilter::Errors => diagnostic.severity == Some(DiagnosticSeverity::ERROR),
            SeverityFilter::ErrorsAndWarnings => matches!(
                diagnostic.severity,
                Some(DiagnosticSeverity::ERROR) | Some(DiagnosticSeverity::WARNING)
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticSummaryLine {
    pub path: PathBuf,
    pub line: u32,
    pub character: u32,
    pub severity: Option<DiagnosticSeverity>,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DiagnosticSummary {
    pub lines: Vec<DiagnosticSummaryLine>,
    pub omitted_files: usize,
    pub omitted_diagnostics: usize,
}

#[derive(Clone, Default)]
pub struct DiagnosticStore {
    inner: Arc<Mutex<BTreeMap<PathBuf, Vec<Diagnostic>>>>,
    notify: Arc<Notify>,
}

impl DiagnosticStore {
    fn lock_inner(&self) -> std::sync::MutexGuard<'_, BTreeMap<PathBuf, Vec<Diagnostic>>> {
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(err) => {
                warn!("diagnostics lock poisoned");
                err.into_inner()
            }
        }
    }

    pub fn update(&self, path: PathBuf, diagnostics: Vec<Diagnostic>) {
        let mut guard = self.lock_inner();
        guard.insert(path, diagnostics);
        drop(guard);
        self.notify.notify_waiters();
    }

    pub fn clear(&self, path: &Path) {
        let mut guard = self.lock_inner();
        guard.remove(path);
        drop(guard);
        self.notify.notify_waiters();
    }

    pub fn diagnostics_for(&self, path: &Path) -> Option<Vec<Diagnostic>> {
        let guard = self.lock_inner();
        guard.get(path).cloned()
    }

    pub fn all_diagnostics(&self) -> Vec<DiagnosticEntry> {
        let guard = self.lock_inner();
        guard
            .iter()
            .flat_map(|(path, diagnostics)| {
                diagnostics
                    .iter()
                    .cloned()
                    .map(|diagnostic| DiagnosticEntry {
                        path: path.clone(),
                        diagnostic,
                    })
            })
            .collect()
    }

    pub async fn wait_for_path(&self, path: &Path, timeout: std::time::Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if self.diagnostics_for(path).is_some() {
                return true;
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            if tokio::time::timeout(remaining, self.notify.notified())
                .await
                .is_err()
            {
                return false;
            }
        }
    }

    pub fn summarize(
        &self,
        filter: SeverityFilter,
        max_files: usize,
        max_per_file: usize,
    ) -> DiagnosticSummary {
        let guard = self.lock_inner();
        let mut lines = Vec::new();
        let mut omitted_files = 0;
        let mut omitted_diagnostics = 0;

        for (index, (path, diagnostics)) in guard.iter().enumerate() {
            if index >= max_files {
                omitted_files += 1;
                continue;
            }
            let mut kept_for_file = 0;
            let mut omitted_for_file = 0;
            for diagnostic in diagnostics.iter().filter(|d| filter.matches(d)) {
                if kept_for_file >= max_per_file {
                    omitted_for_file += 1;
                    continue;
                }
                kept_for_file += 1;
                lines.push(DiagnosticSummaryLine {
                    path: path.clone(),
                    line: diagnostic.range.start.line + 1,
                    character: diagnostic.range.start.character + 1,
                    severity: diagnostic.severity,
                    message: diagnostic.message.clone(),
                    source: diagnostic.source.clone(),
                });
            }
            omitted_diagnostics += omitted_for_file;
        }

        DiagnosticSummary {
            lines,
            omitted_files,
            omitted_diagnostics,
        }
    }
}

impl DiagnosticSummary {
    pub fn render(&self) -> String {
        let mut out = String::new();
        for line in &self.lines {
            let severity = match line.severity {
                Some(DiagnosticSeverity::ERROR) => "error",
                Some(DiagnosticSeverity::WARNING) => "warning",
                Some(DiagnosticSeverity::INFORMATION) => "info",
                Some(DiagnosticSeverity::HINT) => "hint",
                None => "unknown",
                _ => "unknown",
            };
            let path_display = line.path.display();
            let line_number = line.line;
            let character = line.character;
            let message = line.message.trim();
            if let Some(source) = line.source.as_deref() {
                out.push_str(&format!(
                    "- {path_display}:{line_number}:{character} [{severity}] {message} ({source})\n"
                ));
            } else {
                out.push_str(&format!(
                    "- {path_display}:{line_number}:{character} [{severity}] {message}\n"
                ));
            }
        }
        if self.omitted_files > 0 {
            let omitted_files = self.omitted_files;
            out.push_str(&format!("- ...and {omitted_files} more files\n"));
        }
        if self.omitted_diagnostics > 0 {
            let omitted_diagnostics = self.omitted_diagnostics;
            out.push_str(&format!(
                "- ...and {omitted_diagnostics} more diagnostics\n"
            ));
        }
        out
    }
}
