use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_utils_pty::ProcessHandle;
use codex_utils_pty::spawn_pty_process;
use reqwest::Client;
use serde::Deserialize;
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use url::Host;
use url::Url;

use crate::cdp::CdpClient;
use crate::network::BrowserNetworkPolicy;
use crate::screen::BrowserStatus;
use crate::screen::TerminalScreen;
use crate::screen::TerminalSize;
use crate::session::Inner;
use crate::session::RenderMode;
use crate::session::SessionConfig;

pub(crate) struct BrowserSession {
    pub(crate) config: SessionConfig,
    pub(crate) cdp: CdpClient,
    _profile: TempDir,
    output_task: JoinHandle<()>,
    exit_task: JoinHandle<()>,
}

impl Inner {
    pub(crate) async fn ensure_session(self: &Arc<Self>, config: SessionConfig) -> Result<()> {
        let expected_network_policy = config.network_policy.clone();
        let process_exited = self
            .process
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|process| process.has_exited());
        let existing_config = self
            .session
            .lock()
            .await
            .as_ref()
            .map(|session| session.config.clone());
        match existing_config {
            None => self.start_session(config).await?,
            Some(existing) if process_exited || existing != config => {
                let (visible, url) = {
                    let view = self.view.read().unwrap_or_else(PoisonError::into_inner);
                    (view.visible, view.url.clone())
                };
                self.close_session().await;
                self.update_view(|view| {
                    view.status = BrowserStatus::Starting;
                    view.visible = visible;
                    view.url = url;
                });
                self.start_session(config).await?;
            }
            Some(_) => {}
        }
        if self.network_policy() != expected_network_policy {
            self.close_session().await;
            anyhow::bail!("browser network policy changed while Carbonyl was starting");
        }
        Ok(())
    }

    async fn start_session(self: &Arc<Self>, config: SessionConfig) -> Result<()> {
        let binary = self
            .binary
            .as_ref()
            .context("Carbonyl is unavailable; install it or set CODEX_CARBONYL_BINARY")?;
        self.closing.store(/*val*/ false, Ordering::SeqCst);
        let profile = tempfile::Builder::new()
            .prefix("codex-carbonyl-")
            .tempdir()
            .context("create Carbonyl profile directory")?;
        let args = carbonyl_args(
            /*debugging_port*/ 0,
            profile.path().to_string_lossy().as_ref(),
            &config.network_policy,
            config.render_mode,
        );
        // Carbonyl is an external browser process. Pass only the process basics it needs instead
        // of forwarding Codex credentials and service-specific environment variables.
        let mut env = HashMap::new();
        for key in [
            "PATH",
            "HOME",
            "TMPDIR",
            "TMP",
            "TEMP",
            "LANG",
            "LC_ALL",
            "LC_CTYPE",
            "TZ",
            "XDG_RUNTIME_DIR",
            "SSL_CERT_FILE",
            "SSL_CERT_DIR",
        ] {
            if let Ok(value) = std::env::var(key) {
                env.insert(key.to_string(), value);
            }
        }
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        env.insert("COLORTERM".to_string(), "truecolor".to_string());
        let cwd = std::env::current_dir().context("resolve current directory")?;
        let size = *self.size.lock().unwrap_or_else(PoisonError::into_inner);
        let arg0 = None;
        let spawned = match spawn_pty_process(
            binary.to_string_lossy().as_ref(),
            &args,
            &cwd,
            &env,
            &arg0,
            size.into(),
        )
        .await
        {
            Ok(spawned) => spawned,
            Err(error) => {
                return Err(error).context("launch Carbonyl");
            }
        };

        let process = Arc::new(spawned.session);
        let writer = process.writer_sender();
        let (resize_tx, resize_rx) = mpsc::unbounded_channel();
        *self.process.lock().unwrap_or_else(PoisonError::into_inner) = Some(process.clone());
        *self
            .resize_tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(resize_tx);
        if self.network_policy() != config.network_policy {
            self.closing.store(/*val*/ true, Ordering::SeqCst);
            process.terminate();
            self.clear_process_handles();
            anyhow::bail!("browser network policy changed while Carbonyl was starting");
        }
        let output_task =
            spawn_screen_task(self.clone(), spawned.stdout_rx, writer, resize_rx, size);
        let mut exit_rx = spawned.exit_rx;

        let target = tokio::select! {
            target = discover_page_target(profile.path()) => target,
            exit = &mut exit_rx => Err(anyhow!("Carbonyl exited during startup: {exit:?}")),
        };
        let target = match target {
            Ok(target) => target,
            Err(error) => {
                output_task.abort();
                process.terminate();
                self.clear_process_handles();
                return Err(error);
            }
        };
        let cdp = match CdpClient::connect(&target.websocket_url).await {
            Ok(cdp) => cdp,
            Err(error) => {
                output_task.abort();
                process.terminate();
                self.clear_process_handles();
                return Err(error);
            }
        };
        let exit_inner = self.clone();
        let exit_task = tokio::spawn(async move {
            let exit = exit_rx.await;
            if !exit_inner.closing.load(Ordering::SeqCst) {
                exit_inner.set_crashed(match exit {
                    Ok(code) => format!("Carbonyl exited with status {code}"),
                    Err(_) => "lost Carbonyl exit status".to_string(),
                });
            }
        });
        self.update_view(|view| {
            view.status = BrowserStatus::Running;
            if !target.title.is_empty() {
                view.title = Some(target.title.clone());
            }
        });
        *self.session.lock().await = Some(BrowserSession {
            config,
            cdp,
            _profile: profile,
            output_task,
            exit_task,
        });
        Ok(())
    }

    pub(crate) async fn close_session(&self) {
        self.closing.store(/*val*/ true, Ordering::SeqCst);
        let session = self.session.lock().await.take();
        let process = self
            .process
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        self.resize_tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(process) = process {
            let _ = process.writer_sender().send(vec![b'\x03']).await;
            let graceful = tokio::time::timeout(
                Duration::from_millis(/*millis*/ 500),
                wait_for_exit(process.clone()),
            )
            .await
            .is_ok();
            if !graceful {
                process.terminate();
            }
        }
        if let Some(session) = session {
            session.output_task.abort();
            session.exit_task.abort();
        }
        let status = if self.binary.is_some() {
            BrowserStatus::Idle
        } else {
            self.view
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .status
                .clone()
        };
        self.update_view(|view| {
            view.status = status;
            view.visible = false;
            view.title = None;
            view.url = None;
        });
    }

    fn clear_process_handles(&self) {
        self.process
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        self.resize_tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }
}

fn spawn_screen_task(
    inner: Arc<Inner>,
    mut output: mpsc::Receiver<Vec<u8>>,
    writer: mpsc::Sender<Vec<u8>>,
    mut resize_rx: mpsc::UnboundedReceiver<TerminalSize>,
    size: TerminalSize,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut terminal = TerminalScreen::new(size);
        let mut interval = tokio::time::interval(Duration::from_millis(/*millis*/ 33));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut dirty = true;
        let mut output_open = true;
        let mut resize_open = true;
        loop {
            tokio::select! {
                chunk = output.recv(), if output_open => match chunk {
                    Some(chunk) => {
                        for response in terminal.process(&chunk) {
                            let _ = writer.send(response).await;
                        }
                        dirty = true;
                    }
                    None => output_open = false,
                },
                size = resize_rx.recv(), if resize_open => match size {
                    Some(size) => {
                        terminal.resize(size);
                        dirty = true;
                    }
                    None => resize_open = false,
                },
                _ = interval.tick(), if dirty => {
                    inner.update_view(|view| {
                        view.screen = terminal.snapshot();
                        if let Some(title) = terminal.title() {
                            view.title = Some(title);
                        }
                    });
                    dirty = false;
                }
                else => break,
            }
            if !output_open && !resize_open {
                break;
            }
        }
    })
}

async fn wait_for_exit(process: Arc<ProcessHandle>) {
    while !process.has_exited() {
        tokio::time::sleep(Duration::from_millis(/*millis*/ 20)).await;
    }
}

pub(crate) fn carbonyl_args(
    debugging_port: u16,
    profile: &str,
    network_policy: &BrowserNetworkPolicy,
    render_mode: RenderMode,
) -> Vec<String> {
    let mut args = vec![
        "--remote-debugging-address=127.0.0.1".to_string(),
        format!("--remote-debugging-port={debugging_port}"),
        format!("--user-data-dir={profile}"),
        "--disable-extensions".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-sync".to_string(),
        "--disable-default-apps".to_string(),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-component-update".to_string(),
        "--password-store=basic".to_string(),
        "--disable-quic".to_string(),
        "--force-webrtc-ip-handling-policy=disable_non_proxied_udp".to_string(),
    ];
    match network_policy {
        BrowserNetworkPolicy::Disabled | BrowserNetworkPolicy::Direct => {}
        BrowserNetworkPolicy::ManagedProxy { http_addr } => {
            args.push(format!("--proxy-server=http://{http_addr}"));
            args.push("--proxy-bypass-list=<-loopback>".to_string());
        }
    }
    if render_mode == RenderMode::Bitmap {
        args.push("--bitmap".to_string());
    }
    args.push("about:blank".to_string());
    args
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DevtoolsTarget {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    title: String,
    web_socket_debugger_url: Option<String>,
}

struct PageTarget {
    title: String,
    websocket_url: String,
}

async fn discover_page_target(profile: &Path) -> Result<PageTarget> {
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(/*secs*/ 2))
        .build()?;
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 12);
    let active_port_path = profile.join("DevToolsActivePort");
    let port = loop {
        match std::fs::read_to_string(&active_port_path) {
            Ok(contents) => {
                let port = contents
                    .lines()
                    .next()
                    .context("DevToolsActivePort did not contain a port")?
                    .parse::<u16>()
                    .context("DevToolsActivePort contained an invalid port")?;
                anyhow::ensure!(port != 0, "DevToolsActivePort contained port zero");
                break port;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).context("read Carbonyl DevToolsActivePort");
            }
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "timed out waiting for Carbonyl DevToolsActivePort"
        );
        tokio::time::sleep(Duration::from_millis(/*millis*/ 100)).await;
    };
    let endpoint = format!("http://127.0.0.1:{port}/json/list");
    loop {
        if let Ok(response) = client.get(&endpoint).send().await
            && let Ok(targets) = response.json::<Vec<DevtoolsTarget>>().await
        {
            for target in targets {
                if target.kind != "page" {
                    continue;
                }
                let Some(websocket_url) = target.web_socket_debugger_url else {
                    continue;
                };
                return Ok(PageTarget {
                    title: target.title,
                    websocket_url: validated_websocket_url(&websocket_url, port)?,
                });
            }
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "timed out waiting for Carbonyl DevTools on {endpoint}"
        );
        tokio::time::sleep(Duration::from_millis(/*millis*/ 100)).await;
    }
}

pub(crate) fn validated_websocket_url(websocket_url: &str, expected_port: u16) -> Result<String> {
    let parsed = Url::parse(websocket_url).context("parse Carbonyl DevTools WebSocket URL")?;
    anyhow::ensure!(parsed.scheme() == "ws", "Carbonyl DevTools URL must use ws");
    let loopback = match parsed.host() {
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        Some(Host::Domain(_)) | None => false,
    };
    anyhow::ensure!(loopback, "Carbonyl DevTools URL must use a loopback host");
    anyhow::ensure!(
        parsed.port() == Some(expected_port),
        "Carbonyl DevTools URL used an unexpected port"
    );
    Ok(parsed.to_string())
}

impl From<TerminalSize> for codex_utils_pty::TerminalSize {
    fn from(value: TerminalSize) -> Self {
        Self {
            rows: value.rows,
            cols: value.cols,
        }
    }
}
