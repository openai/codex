use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_utils_pty::ProcessHandle;
#[cfg(unix)]
use codex_utils_pty::pty::ChildFdMapping;
#[cfg(unix)]
use codex_utils_pty::pty::spawn_process_with_fd_mappings;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::cdp::CdpClient;
#[cfg(unix)]
use crate::cdp::ConnectedPage;
use crate::devtools::deny_downloads;
use crate::handles::BrowserHandles;
use crate::network::BrowserNetworkPolicy;
use crate::profile::BrowserProfileLock;
use crate::runtime::BrowserRuntime;
use crate::sandbox::prepare_browser_launch;
use crate::screen::BrowserStatus;
use crate::screen::TerminalScreen;
use crate::screen::TerminalSize;
use crate::session::Inner;
use crate::session::RenderMode;
use crate::session::SessionConfig;
use crate::url_policy::spawn_navigation_policy_task;

pub(crate) struct BrowserSession {
    pub(crate) config: SessionConfig,
    pub(crate) cdp: CdpClient,
    pub(crate) handles: BrowserHandles,
    _runtime: BrowserRuntime,
    _profile_lock: Option<BrowserProfileLock>,
    output_task: JoinHandle<()>,
    exit_task: JoinHandle<()>,
    navigation_policy_task: JoinHandle<()>,
}

struct StartupGuard<'a> {
    process_slot: &'a std::sync::Mutex<Option<Arc<ProcessHandle>>>,
    process: Option<Arc<ProcessHandle>>,
    profile_lock: Option<BrowserProfileLock>,
    output_task: Option<JoinHandle<()>>,
    exit_task: Option<JoinHandle<()>>,
    navigation_policy_task: Option<JoinHandle<()>>,
}

struct CommittedStartup {
    profile_lock: Option<BrowserProfileLock>,
    output_task: JoinHandle<()>,
    exit_task: JoinHandle<()>,
    navigation_policy_task: JoinHandle<()>,
}

impl<'a> StartupGuard<'a> {
    fn new(
        process_slot: &'a std::sync::Mutex<Option<Arc<ProcessHandle>>>,
        process: Arc<ProcessHandle>,
        profile_lock: Option<BrowserProfileLock>,
    ) -> Self {
        Self {
            process_slot,
            process: Some(process),
            profile_lock,
            output_task: None,
            exit_task: None,
            navigation_policy_task: None,
        }
    }

    fn set_output_task(&mut self, output_task: JoinHandle<()>) {
        self.output_task = Some(output_task);
    }

    fn set_exit_task(&mut self, exit_task: JoinHandle<()>) {
        self.exit_task = Some(exit_task);
    }

    fn set_navigation_policy_task(&mut self, task: JoinHandle<()>) {
        self.navigation_policy_task = Some(task);
    }

    fn commit(mut self) -> Result<CommittedStartup> {
        let output_task = self
            .output_task
            .take()
            .context("startup output task must be present")?;
        let exit_task = self
            .exit_task
            .take()
            .context("startup exit task must be present")?;
        let navigation_policy_task = self
            .navigation_policy_task
            .take()
            .context("startup navigation policy task must be present")?;
        self.process.take();
        Ok(CommittedStartup {
            profile_lock: self.profile_lock.take(),
            output_task,
            exit_task,
            navigation_policy_task,
        })
    }
}

impl Drop for StartupGuard<'_> {
    fn drop(&mut self) {
        let Some(process) = self.process.take() else {
            return;
        };
        if let Some(task) = self.output_task.take() {
            task.abort();
        }
        if let Some(task) = self.exit_task.take() {
            task.abort();
        }
        if let Some(task) = self.navigation_policy_task.take() {
            task.abort();
        }
        process.terminate();
        let mut current = self
            .process_slot
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if current
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &process))
        {
            current.take();
        }
    }
}

impl Inner {
    pub(crate) async fn ensure_session(self: &Arc<Self>, config: SessionConfig) -> Result<()> {
        anyhow::ensure!(
            !self.terminated.load(Ordering::SeqCst),
            "terminal browser has been terminated"
        );
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

    async fn validated_binary(&self) -> Result<std::path::PathBuf> {
        let binary = self
            .binary
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .context("Carbonyl is unavailable; install it or set CODEX_CARBONYL_BINARY")?;
        let installation =
            crate::diagnostics::inspect_installation(&binary, &self.launch_context).await?;
        let path = installation.path.clone();
        *self.installation.lock().await = Some(installation);
        Ok(path)
    }

    #[cfg(unix)]
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the session guard is explicitly dropped before the conditional close await"
    )]
    async fn start_session(self: &Arc<Self>, config: SessionConfig) -> Result<()> {
        let binary = self.validated_binary().await?;
        anyhow::ensure!(
            !self.terminated.load(Ordering::SeqCst),
            "terminal browser has been terminated"
        );
        self.closing.store(/*val*/ false, Ordering::SeqCst);
        let persistent_profile = self.selected_profile_resources()?;
        let runtime =
            BrowserRuntime::create(persistent_profile.as_ref().map(|(profile, _lock)| profile))?;
        let args = carbonyl_args(
            runtime.profile.as_path().to_string_lossy().as_ref(),
            &config.network_policy,
            config.render_mode,
        );
        // Carbonyl is an external browser process. Pass only the process basics it needs instead
        // of forwarding Codex credentials and service-specific environment variables.
        let env = runtime.environment(&config.network_policy);
        let launch = prepare_browser_launch(
            &binary,
            args,
            &runtime.root,
            &runtime.profile,
            env,
            &config.network_policy,
            &self.launch_context,
        )?;
        let (browser_read, codex_write) = std::os::unix::net::UnixStream::pair()
            .context("create Carbonyl DevTools input pipe")?;
        let (codex_read, browser_write) = std::os::unix::net::UnixStream::pair()
            .context("create Carbonyl DevTools output pipe")?;
        codex_write
            .set_nonblocking(/*nonblocking*/ true)
            .context("configure Carbonyl DevTools input pipe")?;
        codex_read
            .set_nonblocking(/*nonblocking*/ true)
            .context("configure Carbonyl DevTools output pipe")?;
        let child_fd_mappings = [
            ChildFdMapping::new(&browser_read, /*child_fd*/ 3),
            ChildFdMapping::new(&browser_write, /*child_fd*/ 4),
        ];
        // `dup2` clears `FD_CLOEXEC` on these fixed targets. Bubblewrap's monitor close-fd path
        // explicitly passes any other non-CLOEXEC fds on to the child, so the Linux sandbox
        // wrapper needs no fd-specific flag or ambient-descriptor widening for the CDP pipe pair.
        let size = *self.resize_tx.borrow();
        let spawned = match spawn_process_with_fd_mappings(
            &launch.program,
            &launch.args,
            launch.cwd.as_path(),
            &launch.env,
            &launch.arg0,
            size.into(),
            &child_fd_mappings,
        )
        .await
        {
            Ok(spawned) => spawned,
            Err(error) => {
                return Err(error).context("launch Carbonyl");
            }
        };
        drop(browser_read);
        drop(browser_write);

        let process = Arc::new(spawned.session);
        let writer = process.writer_sender();
        let profile_lock = persistent_profile.map(|(_profile, lock)| lock);
        let mut startup = StartupGuard::new(&self.process, process.clone(), profile_lock);
        let resize_result = {
            let mut current_process = self.process.lock().unwrap_or_else(PoisonError::into_inner);
            *current_process = Some(process.clone());
            let current_size = *self.resize_tx.borrow();
            (current_size != size)
                .then(|| process.resize(current_size.into()))
                .transpose()
        };
        if let Err(error) = resize_result {
            return Err(error).context("resize Carbonyl after startup");
        }
        if self.network_policy() != config.network_policy {
            self.closing.store(/*val*/ true, Ordering::SeqCst);
            anyhow::bail!("browser network policy changed while Carbonyl was starting");
        }
        let output_task = spawn_screen_task(
            self.clone(),
            spawned.stdout_rx,
            writer,
            self.resize_tx.subscribe(),
        );
        startup.set_output_task(output_task);
        let mut exit_rx = spawned.exit_rx;

        let connection = tokio::select! {
            connection = CdpClient::connect_pipe(codex_read, codex_write) => connection,
            exit = &mut exit_rx => Err(anyhow!("Carbonyl exited during startup: {exit:?}")),
        };
        let ConnectedPage { client: cdp, title } = connection?;
        deny_downloads(&cdp).await?;
        startup.set_navigation_policy_task(spawn_navigation_policy_task(self.clone(), cdp.clone()));
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
        startup.set_exit_task(exit_task);
        let mut session_slot = self.session.lock().await;
        anyhow::ensure!(
            !self.terminated.load(Ordering::SeqCst),
            "terminal browser has been terminated"
        );
        let CommittedStartup {
            profile_lock,
            output_task,
            exit_task,
            navigation_policy_task,
        } = startup.commit()?;
        *session_slot = Some(BrowserSession {
            config,
            cdp,
            handles: BrowserHandles::default(),
            _runtime: runtime,
            _profile_lock: profile_lock,
            output_task,
            exit_task,
            navigation_policy_task,
        });
        drop(session_slot);
        self.update_view(|view| {
            if !self.terminated.load(Ordering::SeqCst) {
                view.status = BrowserStatus::Running;
                if !title.is_empty() {
                    view.title = Some(title);
                }
            }
        });
        if self.terminated.load(Ordering::SeqCst) {
            self.close_session().await;
            anyhow::bail!("terminal browser has been terminated");
        }
        Ok(())
    }

    #[cfg(not(unix))]
    async fn start_session(self: &Arc<Self>, _config: SessionConfig) -> Result<()> {
        anyhow::bail!("Carbonyl terminal browsing is only supported on macOS and Linux")
    }

    fn selected_profile_resources(
        &self,
    ) -> Result<
        Option<(
            codex_utils_absolute_path::AbsolutePathBuf,
            BrowserProfileLock,
        )>,
    > {
        let selected = self
            .selected_profile
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let Some(selected) = selected else {
            return Ok(None);
        };
        let store = self
            .profile_store
            .as_ref()
            .context("named terminal-browser profiles are unavailable")?;
        store.lock_existing(&selected).map(Some)
    }

    pub(crate) async fn close_session(&self) {
        self.closing.store(/*val*/ true, Ordering::SeqCst);
        self.human_control.store(/*val*/ false, Ordering::SeqCst);
        self.human_control_generation
            .fetch_add(/*val*/ 1, Ordering::SeqCst);
        let session = self.session.lock().await.take();
        let process = self
            .process
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
            session.navigation_policy_task.abort();
        }
        let status = if self
            .binary
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
        {
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
            view.human_control = false;
        });
    }

    pub(crate) fn terminate_now(&self) {
        self.terminated.store(/*val*/ true, Ordering::SeqCst);
        self.closing.store(/*val*/ true, Ordering::SeqCst);
        self.human_control.store(/*val*/ false, Ordering::SeqCst);
        self.human_control_generation
            .fetch_add(/*val*/ 1, Ordering::SeqCst);

        if let Some(process) = self
            .process
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            process.terminate();
        }
        if let Ok(mut session) = self.session.try_lock()
            && let Some(session) = session.take()
        {
            session.output_task.abort();
            session.exit_task.abort();
            session.navigation_policy_task.abort();
        }

        let status = if self
            .binary
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
        {
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
            view.human_control = false;
        });
    }
}

fn spawn_screen_task(
    inner: Arc<Inner>,
    mut output: mpsc::Receiver<Vec<u8>>,
    writer: mpsc::Sender<Vec<u8>>,
    mut resize_rx: watch::Receiver<TerminalSize>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let size = *resize_rx.borrow_and_update();
        let mut terminal = TerminalScreen::new(size);
        let mut interval = tokio::time::interval(Duration::from_millis(/*millis*/ 33));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut dirty = true;
        loop {
            tokio::select! {
                chunk = output.recv() => match chunk {
                    Some(chunk) => {
                        for response in terminal.process(&chunk) {
                            let _ = writer.send(response).await;
                        }
                        let starting = matches!(
                            &inner
                                .view
                                .read()
                                .unwrap_or_else(PoisonError::into_inner)
                                .status,
                            BrowserStatus::Starting
                        );
                        if starting {
                            update_screen_view(&inner, &terminal);
                            dirty = false;
                        } else {
                            dirty = true;
                        }
                    }
                    None => break,
                },
                changed = resize_rx.changed() => match changed {
                    Ok(()) => {
                        let size = *resize_rx.borrow_and_update();
                        terminal.resize(size);
                        dirty = true;
                    }
                    Err(_) => break,
                },
                _ = interval.tick(), if dirty => {
                    update_screen_view(&inner, &terminal);
                    dirty = false;
                }
            }
        }
    })
}

fn update_screen_view(inner: &Inner, terminal: &TerminalScreen) {
    inner.update_view(|view| {
        view.screen = terminal.snapshot();
        if let Some(title) = terminal.title() {
            view.title = Some(title);
        }
    });
}

async fn wait_for_exit(process: Arc<ProcessHandle>) {
    while !process.has_exited() {
        tokio::time::sleep(Duration::from_millis(/*millis*/ 20)).await;
    }
}

pub(crate) fn carbonyl_args(
    profile: &str,
    network_policy: &BrowserNetworkPolicy,
    render_mode: RenderMode,
) -> Vec<String> {
    let mut args = vec![
        "--remote-debugging-pipe".to_string(),
        format!("--user-data-dir={profile}"),
        "--disable-extensions".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-sync".to_string(),
        "--disable-default-apps".to_string(),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-component-update".to_string(),
        "--password-store=basic".to_string(),
        // Every Carbonyl process already inherits the Codex platform sandbox. Chromium cannot
        // initialize a second Seatbelt profile from inside that sandbox on macOS.
        "--no-sandbox".to_string(),
        // Carbonyl does not need a hardware GPU for terminal rendering, and keeping rendering in
        // software avoids granting the browser access to platform GPU device interfaces.
        "--disable-gpu".to_string(),
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

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
