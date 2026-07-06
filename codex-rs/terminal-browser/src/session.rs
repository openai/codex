use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::diagnostics::BrowserDoctorReport;
use crate::diagnostics::BrowserInstallation;
use crate::diagnostics::inspect_installation;
use crate::diagnostics::installation_report;
use crate::diagnostics::unavailable_report;
use crate::human_control::HumanInputSender;
use crate::human_control::QueuedHumanInput;
use crate::network::BrowserNetworkPolicy;
use crate::process::BrowserSession;
use crate::profile::BrowserProfileName;
use crate::profile::BrowserProfileStore;
use crate::sandbox::BrowserLaunchContext;
use crate::screen::BrowserStatus;
use crate::screen::BrowserView;
use crate::screen::TerminalSize;
use anyhow::Result;
use codex_utils_pty::ProcessHandle;
use serde::Deserialize;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::watch;

const CARBONYL_BINARY_ENV: &str = "CODEX_CARBONYL_BINARY";

#[derive(Clone)]
pub struct TerminalBrowser {
    pub(crate) inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub(crate) binary: RwLock<Option<PathBuf>>,
    pub(crate) installation: AsyncMutex<Option<BrowserInstallation>>,
    pub(crate) launch_context: BrowserLaunchContext,
    pub(crate) profile_store: Option<BrowserProfileStore>,
    pub(crate) selected_profile: Mutex<Option<BrowserProfileName>>,
    pub(crate) session_key: Mutex<Option<String>>,
    pub(crate) network_policy: RwLock<BrowserNetworkPolicy>,
    pub(crate) view: RwLock<BrowserView>,
    pub(crate) updates: watch::Sender<u64>,
    pub(crate) update_sequence: AtomicU64,
    pub(crate) operation: AsyncMutex<()>,
    pub(crate) session: AsyncMutex<Option<BrowserSession>>,
    pub(crate) process: Mutex<Option<Arc<ProcessHandle>>>,
    pub(crate) resize_tx: watch::Sender<TerminalSize>,
    pub(crate) closing: AtomicBool,
    pub(crate) terminated: AtomicBool,
    pub(crate) human_control: AtomicBool,
    pub(crate) human_control_transition: AtomicBool,
    pub(crate) human_control_generation: AtomicU64,
    pub(crate) human_input_tx: HumanInputSender,
    pub(crate) human_input_rx: Mutex<Option<tokio::sync::mpsc::Receiver<QueuedHumanInput>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SessionConfig {
    pub(crate) render_mode: RenderMode,
    pub(crate) network_policy: BrowserNetworkPolicy,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RenderMode {
    #[default]
    NativeText,
    Bitmap,
}

impl TerminalBrowser {
    pub fn discover() -> Self {
        Self::discover_with_launch_context(BrowserLaunchContext::default())
    }

    pub fn discover_with_launch_context(launch_context: BrowserLaunchContext) -> Self {
        let size = TerminalSize::default();
        let profile_store =
            BrowserProfileStore::from_context(&launch_context).unwrap_or_else(|error| {
                tracing::warn!(%error, "failed to configure terminal-browser profile storage");
                None
            });
        let binary = discover_binary();
        let status = match &binary {
            Ok(_) => BrowserStatus::Idle,
            Err(reason) => BrowserStatus::Unavailable {
                reason: reason.clone(),
            },
        };
        let (updates, _) = watch::channel(/*init*/ 0);
        let (resize_tx, _) = watch::channel(size);
        let (human_input_tx, human_input_rx) = Self::human_input_channel();

        Self {
            inner: Arc::new(Inner {
                binary: RwLock::new(binary.ok()),
                installation: AsyncMutex::new(/*value*/ None),
                launch_context,
                profile_store,
                selected_profile: Mutex::new(/*value*/ None),
                session_key: Mutex::new(/*t*/ None),
                network_policy: RwLock::new(/*t*/ BrowserNetworkPolicy::Disabled),
                view: RwLock::new(BrowserView::new(status, size)),
                updates,
                update_sequence: AtomicU64::new(/*v*/ 0),
                operation: AsyncMutex::new(()),
                session: AsyncMutex::new(/*t*/ None),
                process: Mutex::new(/*t*/ None),
                resize_tx,
                closing: AtomicBool::new(/*v*/ false),
                terminated: AtomicBool::new(/*v*/ false),
                human_control: AtomicBool::new(/*v*/ false),
                human_control_transition: AtomicBool::new(/*v*/ false),
                human_control_generation: AtomicU64::new(/*v*/ 0),
                human_input_tx,
                human_input_rx: Mutex::new(Some(human_input_rx)),
            }),
        }
    }

    pub fn is_available(&self) -> bool {
        self.inner
            .binary
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    pub async fn doctor(&self) -> BrowserDoctorReport {
        let binary = match discover_binary() {
            Ok(binary) => binary,
            Err(reason) => {
                self.inner.update_view(|view| {
                    view.status = BrowserStatus::Unavailable {
                        reason: reason.clone(),
                    };
                });
                return unavailable_report(&reason);
            }
        };
        let result = inspect_installation(&binary, &self.inner.launch_context).await;
        let report = installation_report(&result);
        if let Ok(installation) = result {
            *self
                .inner
                .binary
                .write()
                .unwrap_or_else(PoisonError::into_inner) = Some(installation.path.clone());
            *self.inner.installation.lock().await = Some(installation);
            self.inner.update_view(|view| {
                if matches!(view.status, BrowserStatus::Unavailable { .. }) {
                    view.status = BrowserStatus::Idle;
                }
            });
        }
        report
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.inner.updates.subscribe()
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "policy changes must serialize with browser spawn and stateful CDP calls"
    )]
    pub async fn set_network_policy(&self, network_policy: BrowserNetworkPolicy) {
        let _operation = self.inner.operation.lock().await;
        let changed = {
            let mut current = self
                .inner
                .network_policy
                .write()
                .unwrap_or_else(PoisonError::into_inner);
            if *current == network_policy {
                false
            } else {
                *current = network_policy;
                true
            }
        };
        if !changed {
            return;
        }
        self.inner.close_session().await;
    }

    pub fn view(&self) -> BrowserView {
        self.inner
            .view
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn set_visibility(&self, visible: bool) {
        self.inner.update_view(|view| view.visible = visible);
    }

    pub fn resize(&self, size: TerminalSize) -> Result<()> {
        anyhow::ensure!(
            size.rows > 0 && size.cols > 0,
            "browser size must be non-zero"
        );
        let process = self
            .inner
            .process
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let changed = self.inner.resize_tx.send_if_modified(|current| {
            if *current == size {
                false
            } else {
                *current = size;
                true
            }
        });
        if !changed {
            return Ok(());
        }
        if let Some(process) = process.as_ref() {
            process.resize(size.into())?;
        }
        Ok(())
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the operation guard prevents close from racing a stateful CDP call"
    )]
    pub async fn close(&self) {
        let _operation = self.inner.operation.lock().await;
        self.inner.close_session().await;
    }

    /// Immediately terminates the browser process during application teardown.
    ///
    /// Normal lifecycle paths should prefer [`Self::close`] so Carbonyl can exit gracefully.
    pub fn terminate(&self) {
        self.inner.terminate_now();
    }
}

impl Inner {
    pub(crate) fn network_policy(&self) -> BrowserNetworkPolicy {
        self.network_policy
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn set_crashed(&self, message: String) {
        self.human_control.store(/*val*/ false, Ordering::SeqCst);
        self.human_control_generation
            .fetch_add(/*val*/ 1, Ordering::SeqCst);
        self.update_view(|view| {
            view.human_control = false;
            view.status = BrowserStatus::Crashed { message };
        });
    }

    pub(crate) fn update_view(&self, update: impl FnOnce(&mut BrowserView)) {
        update(&mut self.view.write().unwrap_or_else(PoisonError::into_inner));
        let sequence = self.update_sequence.fetch_add(/*val*/ 1, Ordering::Relaxed) + 1;
        self.updates.send_replace(sequence);
    }
}

fn discover_binary() -> std::result::Result<PathBuf, String> {
    if !cfg!(any(target_os = "macos", target_os = "linux")) {
        return Err("Carbonyl terminal browsing is only supported on macOS and Linux".to_string());
    }
    if let Some(path) = std::env::var_os(CARBONYL_BINARY_ENV) {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path).ok_or_else(|| {
            format!("{CARBONYL_BINARY_ENV} does not point to a Carbonyl executable")
        });
    }
    which::which("carbonyl").map_err(|_| {
        "Carbonyl was not found on PATH; install it or set CODEX_CARBONYL_BINARY".to_string()
    })
}
