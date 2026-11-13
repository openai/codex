use codex_keyring_store::CredentialStoreError;
use codex_keyring_store::KeyringStore;
use codex_keyring_store::tests::MockKeyringStore;
use pretty_assertions::assert_ne;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;
use std::sync::PoisonError;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tokio::sync::oneshot;

use tempfile::tempdir;

/// Build the actual client's Stop-policy route pool before measuring protocol deadlines.
/// Client construction loads platform/custom CA state and can be slow on developer hosts.
/// Use a separate local endpoint so warmup cannot affect the test server's request assertions;
/// the same production HTTP capability still applies all proxy and certificate policy.
pub(crate) async fn warm_http_client(
    client: &dyn codex_exec_server::HttpClient,
) -> anyhow::Result<()> {
    let server = wiremock::MockServer::start().await;
    client
        .http_request(codex_exec_server::HttpRequestParams {
            method: "GET".to_string(),
            url: server.uri(),
            headers: Vec::new(),
            body: None,
            timeout_ms: None,
            redirect_policy: codex_exec_server::HttpRedirectPolicy::Stop,
            request_id: "test-client-warmup".to_string(),
            stream_response: false,
        })
        .await?;
    Ok(())
}

/// Serializes tests that mutate process-wide CODEX_HOME.
///
/// Keep OAuth tests on this one guard instead of defining per-module helpers; otherwise
/// concurrently running test modules can point File/Secrets storage at different homes.
pub(crate) struct TempCodexHome {
    _guard: MutexGuard<'static, ()>,
    _dir: tempfile::TempDir,
}

impl TempCodexHome {
    pub(crate) fn new() -> Self {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let guard = LOCK
            .get_or_init(Mutex::default)
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let dir = tempdir().expect("create CODEX_HOME temp dir");
        unsafe {
            std::env::set_var("CODEX_HOME", dir.path());
        }
        Self {
            _guard: guard,
            _dir: dir,
        }
    }

    pub(super) fn path(&self) -> &Path {
        self._dir.path()
    }
}

impl Drop for TempCodexHome {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("CODEX_HOME");
        }
    }
}

pub(crate) const REREAD_TEST_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 5);
const REREAD_PANIC_SENTINEL: &str = "credential-panic-payload-sentinel";

#[derive(Clone, Copy, Debug)]
pub(crate) enum ReadOutcome {
    Stored,
    BackendError,
    Panic,
}

#[derive(Clone, Debug)]
pub(crate) struct GatedKeyringStore {
    inner: MockKeyringStore,
    executor_thread: thread::ThreadId,
    entered: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    release: Arc<Mutex<mpsc::Receiver<ReadOutcome>>>,
}

impl GatedKeyringStore {
    pub(crate) fn new(
        inner: MockKeyringStore,
    ) -> (Self, oneshot::Receiver<()>, mpsc::Sender<ReadOutcome>) {
        let (entered_tx, entered_rx) = oneshot::channel();
        // Dropping the sender also releases the worker on early return or panic.
        let (release_tx, release_rx) = mpsc::channel();
        let keyring = Self {
            inner,
            executor_thread: thread::current().id(),
            entered: Arc::new(Mutex::new(Some(entered_tx))),
            release: Arc::new(Mutex::new(release_rx)),
        };
        (keyring, entered_rx, release_tx)
    }
}

impl KeyringStore for GatedKeyringStore {
    fn load(&self, service: &str, account: &str) -> Result<Option<String>, CredentialStoreError> {
        // Fail before blocking if a regression puts this read on the current-thread executor.
        assert_ne!(thread::current().id(), self.executor_thread);
        if let Some(entered) = self
            .entered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = entered.send(());
        }
        let outcome = match self
            .release
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv_timeout(REREAD_TEST_TIMEOUT)
        {
            Ok(outcome) => outcome,
            Err(mpsc::RecvTimeoutError::Disconnected) => ReadOutcome::Stored,
            Err(mpsc::RecvTimeoutError::Timeout) => panic!("credential reread gate timed out"),
        };
        match outcome {
            ReadOutcome::Stored => self.inner.load(service, account),
            ReadOutcome::BackendError => Err(CredentialStoreError::new(
                keyring::Error::PlatformFailure(Box::new(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "credential backend unavailable",
                ))),
            )),
            ReadOutcome::Panic => panic!("{REREAD_PANIC_SENTINEL}"),
        }
    }

    fn save(&self, service: &str, account: &str, value: &str) -> Result<(), CredentialStoreError> {
        self.inner.save(service, account, value)
    }

    fn delete(&self, service: &str, account: &str) -> Result<bool, CredentialStoreError> {
        self.inner.delete(service, account)
    }
}
