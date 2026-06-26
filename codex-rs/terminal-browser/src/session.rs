use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_utils_pty::ProcessHandle;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::mpsc;
use tokio::sync::watch;
use url::Url;

use crate::actions;
use crate::actions::BrowserToolOutput;
use crate::network::BrowserNetworkPolicy;
use crate::process::BrowserSession;
use crate::screen::BrowserStatus;
use crate::screen::BrowserView;
use crate::screen::TerminalSize;

const CARBONYL_BINARY_ENV: &str = "CODEX_CARBONYL_BINARY";
const MAX_URL_INPUT_BYTES: usize = 4 * 1024;
const MAX_FILL_TEXT_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub struct TerminalBrowser {
    inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub(crate) binary: Option<PathBuf>,
    pub(crate) session_key: Mutex<Option<String>>,
    pub(crate) network_policy: RwLock<BrowserNetworkPolicy>,
    pub(crate) size: Mutex<TerminalSize>,
    pub(crate) view: RwLock<BrowserView>,
    pub(crate) updates: watch::Sender<u64>,
    pub(crate) update_sequence: AtomicU64,
    pub(crate) operation: AsyncMutex<()>,
    pub(crate) session: AsyncMutex<Option<BrowserSession>>,
    pub(crate) process: Mutex<Option<Arc<ProcessHandle>>>,
    pub(crate) resize_tx: Mutex<Option<mpsc::UnboundedSender<TerminalSize>>>,
    pub(crate) closing: AtomicBool,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenArgs {
    url: String,
    visible: Option<bool>,
    render_mode: Option<RenderMode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NodeArgs {
    node_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FillArgs {
    node_id: String,
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PressArgs {
    key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScrollArgs {
    #[serde(default)]
    delta_x: i64,
    #[serde(default)]
    delta_y: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VisibilityArgs {
    visible: bool,
}

impl TerminalBrowser {
    pub fn discover() -> Self {
        let size = TerminalSize::default();
        let binary = discover_binary();
        let status = match &binary {
            Ok(_) => BrowserStatus::Idle,
            Err(reason) => BrowserStatus::Unavailable {
                reason: reason.clone(),
            },
        };
        let (updates, _) = watch::channel(/*init*/ 0);
        Self {
            inner: Arc::new(Inner {
                binary: binary.ok(),
                session_key: Mutex::new(/*t*/ None),
                network_policy: RwLock::new(/*t*/ BrowserNetworkPolicy::Disabled),
                size: Mutex::new(size),
                view: RwLock::new(BrowserView::new(status, size)),
                updates,
                update_sequence: AtomicU64::new(/*v*/ 0),
                operation: AsyncMutex::new(()),
                session: AsyncMutex::new(/*t*/ None),
                process: Mutex::new(/*t*/ None),
                resize_tx: Mutex::new(/*t*/ None),
                closing: AtomicBool::new(/*v*/ false),
            }),
        }
    }

    pub fn is_available(&self) -> bool {
        self.inner.binary.is_some()
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
        *self
            .inner
            .size
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = size;
        if let Some(process) = self
            .inner
            .process
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            process.resize(size.into())?;
        }
        if let Some(tx) = self
            .inner
            .resize_tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            let _ = tx.send(size);
        }
        Ok(())
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the operation and session guards serialize stateful CDP calls with lifecycle changes"
    )]
    pub async fn execute(
        &self,
        session_key: &str,
        tool: &str,
        arguments: Value,
    ) -> Result<BrowserToolOutput> {
        let _operation = self
            .inner
            .operation
            .try_lock()
            .map_err(|_| anyhow!("browser_busy: another terminal browser action is running"))?;
        let session_changed = {
            let mut current = self
                .inner
                .session_key
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if current.as_deref() == Some(session_key) {
                false
            } else {
                *current = Some(session_key.to_string());
                true
            }
        };
        if session_changed {
            self.inner.close_session().await;
        }
        if !matches!(tool, "open" | "set_visibility" | "close") {
            let network_policy = self.inner.network_policy();
            let policy_changed = self
                .inner
                .session
                .lock()
                .await
                .as_ref()
                .is_some_and(|session| session.config.network_policy != network_policy);
            if policy_changed {
                self.inner.close_session().await;
                anyhow::bail!("browser network policy changed; reopen the page before continuing");
            }
        }
        match tool {
            "open" => self.open(serde_json::from_value(arguments)?).await,
            "snapshot" => {
                let mut session = self.inner.session.lock().await;
                let session = session.as_mut().context("terminal browser is not open")?;
                let output = actions::snapshot(&mut session.cdp).await?;
                self.refresh_page_metadata(session).await;
                Ok(output)
            }
            "click" => {
                let args: NodeArgs = serde_json::from_value(arguments)?;
                let mut session = self.inner.session.lock().await;
                let session = session.as_mut().context("terminal browser is not open")?;
                let output = actions::click(&mut session.cdp, &args.node_id).await?;
                self.refresh_page_metadata(session).await;
                Ok(output)
            }
            "fill" => {
                let args: FillArgs = serde_json::from_value(arguments)?;
                anyhow::ensure!(
                    args.text.len() <= MAX_FILL_TEXT_BYTES,
                    "fill text exceeds the 64 KiB input limit"
                );
                let mut session = self.inner.session.lock().await;
                let session = session.as_mut().context("terminal browser is not open")?;
                let output = actions::fill(&mut session.cdp, &args.node_id, &args.text).await?;
                self.refresh_page_metadata(session).await;
                Ok(output)
            }
            "press" => {
                let args: PressArgs = serde_json::from_value(arguments)?;
                let mut session = self.inner.session.lock().await;
                let session = session.as_mut().context("terminal browser is not open")?;
                let output = actions::press(&mut session.cdp, &args.key).await?;
                self.refresh_page_metadata(session).await;
                Ok(output)
            }
            "scroll" => {
                let args: ScrollArgs = serde_json::from_value(arguments)?;
                let mut session = self.inner.session.lock().await;
                let session = session.as_mut().context("terminal browser is not open")?;
                let output = actions::scroll(&mut session.cdp, args.delta_x, args.delta_y).await?;
                self.refresh_page_metadata(session).await;
                Ok(output)
            }
            "screenshot" => {
                let mut session = self.inner.session.lock().await;
                let session = session.as_mut().context("terminal browser is not open")?;
                let output = actions::screenshot(&mut session.cdp).await?;
                self.refresh_page_metadata(session).await;
                Ok(output)
            }
            "set_visibility" => {
                let args: VisibilityArgs = serde_json::from_value(arguments)?;
                self.set_visibility(args.visible);
                Ok(BrowserToolOutput::Text(format!(
                    "terminal browser visibility set to {}",
                    args.visible
                )))
            }
            "close" => {
                self.inner.close_session().await;
                Ok(BrowserToolOutput::Text(
                    "terminal browser closed".to_string(),
                ))
            }
            _ => anyhow::bail!("unknown terminal browser tool: {tool}"),
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the operation guard prevents close from racing a stateful CDP call"
    )]
    pub async fn close(&self) {
        let _operation = self.inner.operation.lock().await;
        self.inner.close_session().await;
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the session guard keeps navigation and metadata refresh on the same CDP client"
    )]
    async fn open(&self, args: OpenArgs) -> Result<BrowserToolOutput> {
        anyhow::ensure!(
            args.url.len() <= MAX_URL_INPUT_BYTES,
            "URL exceeds the 4 KiB input limit"
        );
        let url = Url::parse(&args.url).context("parse terminal browser URL")?;
        anyhow::ensure!(
            matches!(url.scheme(), "http" | "https"),
            "terminal_browser only supports http and https URLs"
        );
        let network_policy = self.inner.network_policy();
        anyhow::ensure!(
            !matches!(&network_policy, BrowserNetworkPolicy::Disabled),
            "terminal browser network access is disabled by the active permission profile"
        );
        let config = SessionConfig {
            render_mode: args.render_mode.unwrap_or_default(),
            network_policy,
        };
        let visible = args.visible.unwrap_or(/*default*/ true);
        self.inner.update_view(|view| {
            view.status = BrowserStatus::Starting;
            view.visible = visible;
            view.url = Some(url.as_str().to_string());
        });
        if let Err(error) = self.inner.ensure_session(config).await {
            self.inner.set_crashed(error.to_string());
            return Err(error);
        }

        let mut session = self.inner.session.lock().await;
        let session = session.as_mut().context("terminal browser did not start")?;
        let metadata = match async {
            actions::navigate(&mut session.cdp, url.as_str()).await?;
            actions::page_metadata(&mut session.cdp).await
        }
        .await
        {
            Ok(metadata) => metadata,
            Err(error) => {
                self.inner
                    .update_view(|view| view.status = BrowserStatus::Running);
                return Err(error);
            }
        };
        let final_url = metadata.url.unwrap_or_else(|| url.as_str().to_string());
        self.inner.update_view(|view| {
            view.status = BrowserStatus::Running;
            view.title = metadata.title;
            view.url = Some(final_url);
            view.visible = visible;
        });
        Ok(BrowserToolOutput::Text(format!(
            "opened {} in the terminal browser",
            url.as_str()
        )))
    }

    async fn refresh_page_metadata(&self, session: &mut BrowserSession) {
        match actions::page_metadata(&mut session.cdp).await {
            Ok(metadata) => self.inner.update_view(|view| {
                view.url = metadata.url;
                view.title = metadata.title;
            }),
            Err(error) => {
                tracing::debug!(%error, "failed to refresh terminal browser page metadata");
            }
        }
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
        self.update_view(|view| view.status = BrowserStatus::Crashed { message });
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
