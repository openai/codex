use crate::config::STATSIG_API_KEY;
use crate::config::STATSIG_API_KEY_HEADER;
use crate::config::STATSIG_OTLP_HTTP_ENDPOINT;
use opentelemetry_http::Bytes;
use opentelemetry_http::HttpClient;
use opentelemetry_http::HttpError;
use opentelemetry_http::Request;
use opentelemetry_http::Response;
use opentelemetry_otlp::OTEL_EXPORTER_OTLP_METRICS_TIMEOUT;
use reqwest::header::CONTENT_TYPE;
use std::ffi::OsStr;
use std::fmt;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::mpsc;
use thiserror::Error;

const PROCESS_EXIT_UPLOAD_ARG: &str = "--codex-upload-statsig-metrics";

const MAX_UPLOAD_BYTES: usize = 1024 * 1024;
const MAX_PROCESS_EXIT_UPLOADS: usize = 2;

#[derive(Clone)]
pub(crate) struct StatsigUpload {
    inner: Arc<StatsigUploadInner>,
}

struct StatsigUploadInner {
    state: Mutex<StatsigUploadState>,
    normal_helper_finished: Condvar,
}

#[derive(Debug, Default)]
struct StatsigUploadState {
    executable: Option<PathBuf>,
    process_exit: bool,
    normal_helper_active: bool,
    active_process_exit_helpers: usize,
    reaper: Option<mpsc::Sender<ReapRequest>>,
}

impl fmt::Debug for StatsigUpload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StatsigUpload")
            .finish_non_exhaustive()
    }
}

impl StatsigUpload {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(StatsigUploadInner {
                state: Mutex::new(StatsigUploadState::default()),
                normal_helper_finished: Condvar::new(),
            }),
        }
    }

    pub(crate) fn configure_executable(&self, executable: PathBuf) {
        let mut state = self.state();
        if state.reaper.is_some() {
            state.executable = Some(executable);
            return;
        }
        let (sender, receiver) = mpsc::channel();
        match std::thread::Builder::new()
            .name("codex-statsig-uploader-reaper".to_string())
            .spawn(move || reap_uploaders(receiver))
        {
            Ok(_) => {
                state.reaper = Some(sender);
                state.executable = Some(executable);
            }
            Err(err) => {
                tracing::warn!(%err, "Failed to start Statsig metrics uploader reaper");
            }
        }
    }

    pub(crate) fn prepare(&self) -> bool {
        let mut state = self.state();
        if state.executable.is_none() {
            return false;
        }
        state.process_exit = true;
        self.inner.normal_helper_finished.notify_all();
        true
    }

    fn action(&self) -> io::Result<StatsigUploadAction> {
        let mut state = self.state();
        let Some(executable) = state.executable.clone() else {
            return Ok(StatsigUploadAction::SendDirectly);
        };
        while state.normal_helper_active && !state.process_exit {
            state = self
                .inner
                .normal_helper_finished
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        if state.process_exit {
            if state.active_process_exit_helpers >= MAX_PROCESS_EXIT_UPLOADS {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "Statsig metrics uploader has too many active process-exit helpers",
                ));
            }
            state.active_process_exit_helpers += 1;
            return Ok(StatsigUploadAction::Spawn(
                executable,
                HelperSlot::new(self.clone(), UploadKind::ProcessExit),
            ));
        }
        state.normal_helper_active = true;
        Ok(StatsigUploadAction::Spawn(
            executable,
            HelperSlot::new(self.clone(), UploadKind::Normal),
        ))
    }

    fn spawn(&self, executable: PathBuf, payload: Bytes, slot: HelperSlot) -> io::Result<()> {
        let mut file = tempfile::tempfile()?;
        file.write_all(&payload)?;
        file.seek(SeekFrom::Start(0))?;
        let child = spawn_uploader(executable, file)?;
        self.reap(ReapRequest { child, _slot: slot });
        Ok(())
    }

    fn reap(&self, request: ReapRequest) {
        let failed = match self.state().reaper.clone() {
            Some(sender) => match sender.send(request) {
                Ok(()) => return,
                Err(err) => err.0,
            },
            None => request,
        };
        tracing::warn!("Statsig metrics uploader reaper is unavailable");
        let ReapRequest { child, _slot } = failed;
        drop(child);
        std::mem::forget(_slot);
    }

    fn state(&self) -> std::sync::MutexGuard<'_, StatsigUploadState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

enum StatsigUploadAction {
    SendDirectly,
    Spawn(PathBuf, HelperSlot),
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum UploadKind {
    Normal,
    ProcessExit,
}

struct HelperSlot {
    upload: StatsigUpload,
    kind: UploadKind,
}

impl HelperSlot {
    fn new(upload: StatsigUpload, kind: UploadKind) -> Self {
        Self { upload, kind }
    }
}

impl Drop for HelperSlot {
    fn drop(&mut self) {
        let mut state = self.upload.state();
        match self.kind {
            UploadKind::Normal => {
                debug_assert!(state.normal_helper_active);
                state.normal_helper_active = false;
                self.upload.inner.normal_helper_finished.notify_all();
            }
            UploadKind::ProcessExit => {
                debug_assert!(state.active_process_exit_helpers > 0);
                state.active_process_exit_helpers -= 1;
            }
        }
    }
}

struct ReapRequest {
    child: Child,
    _slot: HelperSlot,
}

#[derive(Debug)]
pub(crate) struct StatsigUploadClient {
    inner: reqwest::blocking::Client,
    upload: StatsigUpload,
}

impl StatsigUploadClient {
    pub(crate) fn new(inner: reqwest::blocking::Client, upload: StatsigUpload) -> Self {
        Self { inner, upload }
    }

    fn validate(request: &Request<Bytes>) -> Result<(), HttpError> {
        if request.body().len() > MAX_UPLOAD_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Statsig metrics upload exceeded the {MAX_UPLOAD_BYTES}-byte detached limit"
                ),
            )
            .into());
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl HttpClient for StatsigUploadClient {
    async fn send_bytes(&self, request: Request<Bytes>) -> Result<Response<Bytes>, HttpError> {
        // PeriodicReader serializes exports and shutdown on one worker. Once
        // the TUI can re-exec Codex, keep network I/O out of that worker so a
        // final flush cannot queue behind an in-flight HTTP request.
        Self::validate(&request)?;
        match self.upload.action()? {
            StatsigUploadAction::SendDirectly => self.inner.send_bytes(request).await,
            StatsigUploadAction::Spawn(executable, slot) => {
                self.upload.spawn(executable, request.into_body(), slot)?;
                synthetic_success()
            }
        }
    }
}

fn spawn_uploader(executable: PathBuf, payload: File) -> io::Result<Child> {
    let mut command = Command::new(executable);
    command
        .arg(PROCESS_EXIT_UPLOAD_ARG)
        .stdin(Stdio::from(payload))
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        // SAFETY: setsid is async-signal-safe and does not access parent memory.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }

    command.spawn()
}

fn synthetic_success() -> Result<Response<Bytes>, HttpError> {
    Ok(Response::builder()
        .status(http::StatusCode::OK)
        .body(Bytes::new())?)
}

fn reap_uploaders(receiver: mpsc::Receiver<ReapRequest>) {
    while let Ok(mut request) = receiver.recv() {
        log_uploader_status(request.child.wait());
    }
}

fn log_uploader_status(status: io::Result<std::process::ExitStatus>) {
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => {
            tracing::warn!(?status, "Statsig metrics uploader exited unsuccessfully");
        }
        Err(err) => {
            tracing::warn!(%err, "Failed to reap Statsig metrics uploader");
        }
    }
}

#[derive(Debug, Error)]
pub enum ProcessExitUploadError {
    #[error("failed to read process-exit metrics payload")]
    Read(#[source] io::Error),

    #[error("process-exit metrics payload exceeded the {MAX_UPLOAD_BYTES}-byte limit")]
    TooLarge,

    #[error("failed to build Statsig process-exit upload client")]
    BuildClient(#[source] reqwest::Error),

    #[error("failed to upload process-exit metrics")]
    Upload(#[source] reqwest::Error),
}

pub fn run_process_exit_upload_if_requested() -> Option<Result<(), ProcessExitUploadError>> {
    (std::env::args_os().nth(1).as_deref() == Some(OsStr::new(PROCESS_EXIT_UPLOAD_ARG)))
        .then(upload_statsig_metrics_from_stdin)
}

fn upload_statsig_metrics_from_stdin() -> Result<(), ProcessExitUploadError> {
    upload_statsig_metrics(io::stdin().lock(), STATSIG_OTLP_HTTP_ENDPOINT)
}

fn upload_statsig_metrics(reader: impl Read, endpoint: &str) -> Result<(), ProcessExitUploadError> {
    let mut body = Vec::new();
    reader
        .take((MAX_UPLOAD_BYTES + 1) as u64)
        .read_to_end(&mut body)
        .map_err(ProcessExitUploadError::Read)?;
    if body.len() > MAX_UPLOAD_BYTES {
        return Err(ProcessExitUploadError::TooLarge);
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(crate::otlp::resolve_otlp_timeout(
            OTEL_EXPORTER_OTLP_METRICS_TIMEOUT,
        ))
        .build()
        .map_err(ProcessExitUploadError::BuildClient)?;
    client
        .post(endpoint)
        .header(CONTENT_TYPE, "application/json")
        .header(STATSIG_API_KEY_HEADER, STATSIG_API_KEY)
        .body(body)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(ProcessExitUploadError::Upload)?;
    Ok(())
}

#[cfg(test)]
#[path = "process_exit_upload_tests.rs"]
mod tests;
