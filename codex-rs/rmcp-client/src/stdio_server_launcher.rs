//! Launch MCP stdio servers and return the transport rmcp should use.
//!
//! This module owns the "where does the server process run?" decision:
//!
//! - [`LocalStdioServerLauncher`] starts the configured command as a child of
//!   the orchestrator process.
//! - [`ExecutorStdioServerLauncher`] starts the configured command through the
//!   executor process API.
//!
//! Both paths return [`StdioServerTransport`], so `RmcpClient` can hand the
//! resulting byte stream to rmcp without knowing where the process lives. The
//! executor-specific byte adaptation lives in `executor_process_transport`.
//! Unix local servers use the stdio-only descriptor policy.

use std::collections::HashMap;
use std::ffi::OsString;
use std::future::Future;
use std::io;
#[cfg(windows)]
use std::os::windows::io::OwnedHandle;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(unix)]
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
#[cfg(unix)]
use std::thread::sleep;
#[cfg(unix)]
use std::thread::spawn;
use std::time::Duration;

use crate::local_stdio_transport::DiscoveryOutcome;
use anyhow::Result;
use anyhow::anyhow;
use codex_config::types::McpServerEnvVar;
use codex_exec_server::ExecBackend;
use codex_exec_server::ExecEnvPolicy;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcess;
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
#[cfg(unix)]
use codex_utils_pty::Child;
use codex_utils_pty::Command;
#[cfg(unix)]
use codex_utils_pty::DescriptorPolicy;
use codex_utils_pty::ProcessMode;
#[cfg(unix)]
use codex_utils_pty::process_group::kill_process_group;
#[cfg(unix)]
use codex_utils_pty::process_group::terminate_process_group;
#[cfg(unix)]
use codex_utils_pty::process_group::wait_for_process_group_exit;
use futures::FutureExt;
use futures::future::BoxFuture;
use futures::future::Shared;
use rmcp::service::RoleClient;
use rmcp::service::RxJsonRpcMessage;
use rmcp::service::TxJsonRpcMessage;
use rmcp::transport::Transport;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::runtime::Handle;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::time::Instant;
use tokio_util::task::AbortOnDropHandle;
use tracing::info;
use tracing::warn;

use crate::executor_process_transport::ExecutorProcessTransport;
use crate::local_stdio_transport::LocalStdioTransport;
use crate::program_resolver;
use crate::protocol_mode::McpProtocolMode;
use crate::utils::create_env_for_mcp_server;
use crate::utils::create_env_overlay_for_remote_mcp_server;
use crate::utils::remote_mcp_env_var_names;

// General purpose public code.

/// Launches an MCP stdio server and returns the transport for rmcp.
///
/// This trait is the boundary between MCP lifecycle code and process placement.
/// `RmcpClient` owns MCP operations such as `initialize` and `tools/list`; the
/// launcher owns starting the configured command and producing an rmcp
/// [`Transport`] over the server's stdin/stdout bytes.
pub trait StdioServerLauncher: private::Sealed + Send + Sync {
    /// Start the configured stdio server and return its rmcp-facing transport.
    fn launch(
        &self,
        command: StdioServerCommand,
    ) -> BoxFuture<'static, io::Result<StdioServerTransport>>;
}

/// Command-line process shape shared by stdio server launchers.
#[derive(Clone)]
pub struct StdioServerCommand {
    program: OsString,
    args: Vec<OsString>,
    env: Option<HashMap<OsString, OsString>>,
    env_vars: Vec<McpServerEnvVar>,
    cwd: Option<String>,
    protocol_mode: McpProtocolMode,
    replacement_slot: Option<StdioProcessSlot>,
}

/// A replacement holds the slot from OS spawn through registration. Closing it seals that handoff.
pub(super) type StdioProcessSlot = Arc<AsyncMutex<StdioProcessRegistration>>;

pub(super) struct StdioProcessRegistration {
    process: StdioProcessRef,
    closed: bool,
}

impl StdioProcessRegistration {
    pub(super) fn close(&mut self) -> StdioProcessRef {
        self.closed = true;
        self.process.clone()
    }
}

type DiscoveryCleanupResult = Shared<BoxFuture<'static, Result<(), Arc<io::Error>>>>;

struct DiscoveryCleanup {
    process: Weak<StdioServerProcessHandle>,
    result: OnceLock<DiscoveryCleanupResult>,
}

impl DiscoveryCleanup {
    fn start(&self, runtime: &Handle, reason: StdioTermination) -> &DiscoveryCleanupResult {
        self.result.get_or_init(|| {
            let process = self.process.upgrade();
            // The owner waits for this worker on shutdown. Its grace periods must also advance
            // when the caller leaves a current-thread runtime idle.
            runtime
                .spawn_blocking(move || {
                    process
                        .ok_or_else(|| io::Error::other("discovery process already released"))?
                        .terminate_discovery(reason)
                })
                .map(|joined| match joined {
                    Ok(result) => result.map_err(Arc::new),
                    Err(error) => Err(Arc::new(io::Error::other(error))),
                })
                .boxed()
                .shared()
        })
    }
}

#[derive(Clone)]
pub(super) struct StdioProcessRef {
    process: Weak<StdioServerProcessHandle>,
    discovery_cleanup: Option<Arc<DiscoveryCleanup>>,
}

impl StdioProcessRef {
    pub(super) async fn terminate(&self, reason: StdioTermination) -> io::Result<()> {
        if let Some(cleanup) = &self.discovery_cleanup {
            let result = cleanup.start(&Handle::current(), reason);
            return result.clone().await.map_err(io::Error::other);
        }
        if let Some(process) = self.process.upgrade() {
            process.terminate().await?;
        }
        Ok(())
    }
}

/// Client-side rmcp transport for a launched MCP stdio server.
///
/// The concrete process placement stays private to this module. `RmcpClient`
/// only sees the standard rmcp transport abstraction and can pass this value
/// directly to `rmcp::service::serve_client`.
pub struct StdioServerTransport {
    inner: StdioServerTransportInner,
    process: Arc<StdioServerProcessHandle>,
    discovery_cleanup: Option<Arc<DiscoveryCleanup>>,
    protocol_mode: McpProtocolMode,
}

enum StdioServerTransportInner {
    Local(LocalStdioTransport),
    Executor(ExecutorProcessTransport),
}

impl Transport<RoleClient> for StdioServerTransport {
    type Error = io::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = std::result::Result<(), Self::Error>> + Send + 'static {
        // Both variants already implement rmcp's transport contract. This
        // wrapper keeps process placement private while leaving rmcp's send
        // semantics unchanged.
        match &mut self.inner {
            StdioServerTransportInner::Local(transport) => transport.send(item).boxed(),
            StdioServerTransportInner::Executor(transport) => transport.send(item).boxed(),
        }
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<RoleClient>>> + Send {
        // rmcp reads from the same transport shape for both placements. The
        // executor variant turns pushed process-output events back into the
        // line-delimited JSON stream expected by rmcp.
        match &mut self.inner {
            StdioServerTransportInner::Local(transport) => transport.receive().boxed(),
            StdioServerTransportInner::Executor(transport) => transport.receive().boxed(),
        }
    }

    async fn close(&mut self) -> std::result::Result<(), Self::Error> {
        if self.discovery_cleanup.is_none() {
            self.process.terminate().await?;
        }
        match &mut self.inner {
            StdioServerTransportInner::Local(transport) => transport.close().await,
            StdioServerTransportInner::Executor(transport) => transport.close().await,
        }
    }
}

impl Drop for StdioServerTransport {
    fn drop(&mut self) {
        // A client or handshake still holding this result may retry after the SDK drops a
        // failed transport. Otherwise the process handle runs its ordinary shutdown on drop.
        if let Some(cleanup) = &self.discovery_cleanup
            && Arc::strong_count(cleanup) > 1
            && !self.process.terminated.load(Ordering::Acquire)
            && let Ok(runtime) = Handle::try_current()
        {
            cleanup.start(&runtime, StdioTermination::Restart);
        }
    }
}

impl StdioServerTransport {
    pub(super) fn protocol_mode(&self) -> McpProtocolMode {
        self.protocol_mode
    }

    /// Observes the first discovery reply or a clean EOF during local stdio startup.
    pub(super) fn watch_discovery(
        &mut self,
        timeout: Option<Duration>,
    ) -> Option<oneshot::Receiver<DiscoveryOutcome>> {
        match &mut self.inner {
            StdioServerTransportInner::Local(local) => Some(local.watch_discovery(timeout)),
            StdioServerTransportInner::Executor(_) => None,
        }
    }

    pub(super) fn process_ref(&self) -> StdioProcessRef {
        StdioProcessRef {
            process: Arc::downgrade(&self.process),
            discovery_cleanup: self.discovery_cleanup.clone(),
        }
    }

    pub(super) fn process_slot(&self) -> StdioProcessSlot {
        Arc::new(AsyncMutex::new(StdioProcessRegistration {
            process: self.process_ref(),
            closed: false,
        }))
    }
}

impl StdioServerCommand {
    /// Build the stdio process parameters before choosing where the process
    /// runs.
    pub(super) fn new(
        program: OsString,
        args: Vec<OsString>,
        env: Option<HashMap<OsString, OsString>>,
        env_vars: Vec<McpServerEnvVar>,
        cwd: Option<String>,
        protocol_mode: McpProtocolMode,
    ) -> Self {
        Self {
            program,
            args,
            env,
            env_vars,
            cwd,
            protocol_mode,
            replacement_slot: None,
        }
    }

    /// Registers the replacement with its original client before the native spawn can finish.
    pub(super) fn for_restart(
        &self,
        protocol_mode: McpProtocolMode,
        slot: StdioProcessSlot,
    ) -> Self {
        Self {
            protocol_mode,
            replacement_slot: Some(slot),
            ..self.clone()
        }
    }
}

// Local public implementation.

/// Starts MCP stdio servers as local child processes.
///
/// This is the existing behavior for local MCP servers: the orchestrator
/// process spawns the configured command and rmcp talks to the child's local
/// stdin/stdout pipes directly.
#[derive(Clone)]
pub struct LocalStdioServerLauncher {
    fallback_cwd: PathBuf,
}

impl LocalStdioServerLauncher {
    /// Creates a local stdio launcher.
    ///
    /// `fallback_cwd` is used when the MCP server config omits `cwd`, so
    /// relative commands resolve from the caller's runtime working directory.
    pub fn new(fallback_cwd: PathBuf) -> Self {
        Self { fallback_cwd }
    }
}

impl StdioServerLauncher for LocalStdioServerLauncher {
    fn launch(
        &self,
        command: StdioServerCommand,
    ) -> BoxFuture<'static, io::Result<StdioServerTransport>> {
        let fallback_cwd = self.fallback_cwd.clone();
        async move {
            // Cancel queued launches when startup is dropped. A worker that already started
            // drops its transport and cleans up the child when it finishes.
            AbortOnDropHandle::new(tokio::task::spawn_blocking(move || {
                Self::launch_server(command, fallback_cwd)
            }))
            .await
            .map_err(io::Error::other)?
        }
        .boxed()
    }
}

// Local private implementation.

#[cfg(unix)]
const PROCESS_GROUP_TERM_GRACE_PERIOD: Duration = Duration::from_secs(2);

#[cfg(unix)]
const RESTART_DRAIN_GRACE_PERIOD: Duration = Duration::from_millis(250);

// Keep queued stderr diagnostics before closing the reader, even when an
// escaped descendant prevents the pipe from reaching EOF.
const STDERR_READER_DRAIN_GRACE_PERIOD: Duration = Duration::from_millis(250);

#[cfg(unix)]
enum LocalProcessTerminator {
    ProcessGroup(u32),
    // The SDK may drop the failed transport before the caller can decide whether to restart it.
    DiscoveryLeader {
        leader: Mutex<Option<Box<Child>>>,
        runtime: Handle,
    },
}

#[cfg(unix)]
#[derive(Clone)]
struct KillDiscoveryGroupOnDrop(Arc<Mutex<Option<Box<Child>>>>);

#[cfg(unix)]
impl KillDiscoveryGroupOnDrop {
    fn new(child: Box<Child>) -> Self {
        Self(Arc::new(Mutex::new(Some(child))))
    }

    fn kill(&self) -> io::Result<()> {
        // Runtime teardown must wait for a timer that has already started sending the signal.
        let mut leader = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(child) = leader.as_ref()
            && let Some(pid) = child.id()
        {
            kill_process_group_and_warn(pid)?;
        }
        leader.take();
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for KillDiscoveryGroupOnDrop {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

#[cfg(unix)]
fn kill_process_group_and_warn(pid: u32) -> io::Result<()> {
    kill_process_group(pid)
        .inspect_err(|error| warn!("Failed to kill MCP process group {pid}: {error}"))
}

#[cfg(windows)]
enum LocalProcessTerminator {
    Job {
        job: codex_utils_pty::JobObject,
        discovery_root: Option<OwnedHandle>,
    },
    Process(OwnedHandle),
}

#[cfg(not(any(unix, windows)))]
struct LocalProcessTerminator;

pub(crate) struct StdioServerProcessHandle {
    program_name: String,
    kind: StdioServerProcessKind,
    terminated: AtomicBool,
    // An escaped descendant can keep stderr open after the MCP server exits.
    stderr_reader: Option<watch::Sender<()>>,
}

enum StdioServerProcessKind {
    Local(Option<LocalProcessTerminator>),
    Executor(Arc<dyn ExecProcess>),
}

#[derive(Clone, Copy)]
pub(super) enum StdioTermination {
    Shutdown,
    Restart,
}

mod private {
    pub trait Sealed {}
}

impl private::Sealed for LocalStdioServerLauncher {}

impl LocalStdioServerLauncher {
    fn launch_server(
        command: StdioServerCommand,
        fallback_cwd: PathBuf,
    ) -> io::Result<StdioServerTransport> {
        let StdioServerCommand {
            program,
            args,
            env,
            env_vars,
            cwd,
            protocol_mode,
            replacement_slot,
        } = command;
        let program_name = program.to_string_lossy().into_owned();
        let envs = create_env_for_mcp_server(env, &env_vars).map_err(io::Error::other)?;
        let cwd = cwd.map(PathBuf::from).unwrap_or(fallback_cwd);
        let resolved_program =
            program_resolver::resolve(program, &envs, &cwd).map_err(io::Error::other)?;

        let build_command = || {
            let mut command = Command::new(&resolved_program);
            command.current_dir(&cwd).envs(&envs).args(&args);
            command.process_mode(ProcessMode::NewGroup);
            // MCP uses only stdio; select Explicit to exclude unrelated
            // orchestrator descriptors from the server and commands it launches.
            // Descriptor allowlisting is Unix-only. Windows can still inherit unrelated
            // handles and needs a handle allowlist in the shared spawn backend.
            #[cfg(unix)]
            command.descriptor_policy(DescriptorPolicy::Explicit);
            command
        };
        #[cfg(windows)]
        let mut command = build_command();
        #[cfg(not(windows))]
        let command = build_command();
        #[cfg(windows)]
        let job = match codex_utils_pty::JobObject::create_without_breakaway() {
            Ok(job) => {
                command.prepare_suspended_spawn(&job);
                Some(job)
            }
            Err(error) => {
                warn!("Windows MCP process job containment unavailable: {error}");
                None
            }
        };

        // The async caller can be cancelled while this blocking worker is inside spawn.
        let slot = replacement_slot.as_ref().map(|slot| slot.blocking_lock());
        if slot.as_ref().is_some_and(|slot| slot.closed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "MCP client is shut down",
            ));
        }
        let child = command.spawn()?;
        #[cfg(windows)]
        let (child, job) = match job {
            Some(job) => match child
                .id()
                .ok_or_else(|| io::Error::other("missing suspended MCP server process id"))
                .and_then(|process_id| job.assign_and_resume_process(process_id))
            {
                Ok(Some(root)) => (child, Some((job, root))),
                Ok(None) => (child, None),
                Err(error) => {
                    warn!(
                        "Windows MCP process job containment failed; retrying without it: {error}"
                    );
                    drop(child);
                    drop(job);
                    (build_command().spawn()?, None)
                }
            },
            None => (child, None),
        };
        #[cfg(windows)]
        let protocol_mode = if job.is_none() && protocol_mode == McpProtocolMode::DiscoverLocalStdio
        {
            // A probe can require a restart. Without a job we cannot stop its descendants.
            McpProtocolMode::Legacy
        } else {
            protocol_mode
        };
        let process_id = child.id();
        // A legacy replacement still belongs to the discovery startup that launched it.
        let discovery_owned = (protocol_mode == McpProtocolMode::DiscoverLocalStdio
            || replacement_slot.is_some())
            && process_id.is_some();
        let (transport, stderr) =
            LocalStdioTransport::new(child, program_name.clone(), protocol_mode)?;
        #[cfg(windows)]
        let terminator = match job {
            Some((job, root)) => Some(LocalProcessTerminator::Job {
                job,
                discovery_root: discovery_owned.then_some(root),
            }),
            None => process_id.and_then(|process_id| {
                match codex_utils_pty::JobObject::open_process_handle(process_id) {
                    Ok(handle) => Some(LocalProcessTerminator::Process(handle)),
                    Err(error) => {
                        warn!("Windows MCP process handle unavailable: {error}");
                        None
                    }
                }
            }),
        };
        #[cfg(unix)]
        let (transport, terminator) = {
            let mut transport = transport;
            let terminator = process_id.map(|pid| {
                if discovery_owned {
                    LocalProcessTerminator::DiscoveryLeader {
                        leader: Mutex::new(transport.take_child()),
                        runtime: Handle::current(),
                    }
                } else {
                    LocalProcessTerminator::ProcessGroup(pid)
                }
            });
            (transport, terminator)
        };
        #[cfg(not(any(unix, windows)))]
        let terminator = process_id.map(|_| LocalProcessTerminator);
        let stderr_reader = stderr.map(|stderr| {
            let program_name = program_name.clone();
            let (stop_tx, mut stop_rx) = watch::channel(());
            std::mem::drop(tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                // Give queued diagnostics time to reach the logs without waiting
                // indefinitely for a descendant that still has stderr open.
                let drain_deadline = tokio::time::sleep(STDERR_READER_DRAIN_GRACE_PERIOD);
                tokio::pin!(drain_deadline);
                let mut draining = false;
                loop {
                    tokio::select! {
                        biased;
                        _ = &mut drain_deadline, if draining => break,
                        _ = stop_rx.changed(), if !draining => {
                            draining = true;
                            drain_deadline.as_mut().reset(
                                Instant::now() + STDERR_READER_DRAIN_GRACE_PERIOD
                            );
                        }
                        line = reader.next_line() => {
                            match line {
                                Ok(Some(line)) => {
                                    info!("MCP server stderr ({program_name}): {line}");
                                }
                                Ok(None) => break,
                                Err(error) => {
                                    warn!("Failed to read MCP server stderr ({program_name}): {error}");
                                    break;
                                }
                            }
                        },
                    }
                }
            }));
            stop_tx
        });
        let process = StdioServerProcessHandle::local(program_name, terminator, stderr_reader);
        let discovery_cleanup = discovery_owned.then(|| {
            Arc::new(DiscoveryCleanup {
                process: Arc::downgrade(&process),
                result: OnceLock::new(),
            })
        });
        let transport = StdioServerTransport {
            inner: StdioServerTransportInner::Local(transport),
            process,
            discovery_cleanup,
            protocol_mode,
        };
        if let Some(mut slot) = slot {
            slot.process = transport.process_ref();
        }

        Ok(transport)
    }
}

impl LocalProcessTerminator {
    #[cfg(unix)]
    fn terminate_discovery(&self, reason: StdioTermination) -> io::Result<()> {
        let Self::DiscoveryLeader { leader, .. } = self else {
            return Err(io::Error::other("missing Unix MCP discovery leader"));
        };
        let child = leader
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .ok_or_else(|| io::Error::other("Unix MCP discovery leader already released"))?;
        let pid = child
            .id()
            .ok_or_else(|| io::Error::other("missing Unix MCP discovery leader PID"))?;
        let cleanup = KillDiscoveryGroupOnDrop::new(child);
        // A failed transport closed stdin. Let the helper release its endpoint before TERM,
        // and reserve its PID through the last group signal.
        let term_grace = match reason {
            StdioTermination::Restart => {
                sleep(RESTART_DRAIN_GRACE_PERIOD);
                RESTART_DRAIN_GRACE_PERIOD
            }
            StdioTermination::Shutdown => PROCESS_GROUP_TERM_GRACE_PERIOD,
        };
        if terminate_process_group(pid)? {
            // A wrapper may exit before a child handling SIGTERM has released its own endpoint.
            wait_for_process_group_exit(pid, term_grace);
        }
        cleanup.kill()?;
        wait_for_process_group_exit(pid, Duration::from_secs(1));
        Ok(())
    }

    #[cfg(unix)]
    fn terminate(&self) {
        let (process_group_id, discovery) = match self {
            Self::ProcessGroup(pid) => (*pid, None),
            Self::DiscoveryLeader { leader, runtime } => {
                let Some(leader) = leader
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take()
                else {
                    return;
                };
                let Some(pid) = leader.id() else {
                    return;
                };
                (pid, Some((runtime, leader)))
            }
        };
        let should_escalate = match terminate_process_group(process_group_id) {
            Ok(exists) => exists,
            Err(error) => {
                warn!("Failed to terminate MCP process group {process_group_id}: {error}");
                false
            }
        };
        if should_escalate {
            let discovery = discovery.map(|(runtime, leader)| {
                let on_timeout = KillDiscoveryGroupOnDrop::new(leader);
                let on_runtime_exit = on_timeout.clone();
                let (finished, wait) = oneshot::channel::<()>();
                std::mem::drop(runtime.spawn(async move {
                    let _ = wait.await;
                    drop(on_runtime_exit);
                }));
                (on_timeout, finished)
            });
            // A caller can leave a current-thread runtime idle for the entire grace period.
            spawn(move || {
                sleep(PROCESS_GROUP_TERM_GRACE_PERIOD);
                if let Some((on_timeout, finished)) = discovery {
                    drop(on_timeout);
                    drop(finished);
                } else {
                    let _ = kill_process_group_and_warn(process_group_id);
                }
            });
        }
    }

    #[cfg(windows)]
    fn terminate_discovery(&self, _reason: StdioTermination) -> io::Result<()> {
        match self {
            Self::Job {
                job,
                discovery_root: Some(root),
            } => job.terminate_and_wait(root, Duration::from_secs(1)),
            _ => Err(io::Error::other(
                "cannot restart an uncontained Windows MCP server",
            )),
        }
    }

    #[cfg(windows)]
    fn terminate(&self) {
        let result = match self {
            Self::Job { job, .. } => job.terminate(),
            Self::Process(process_handle) => {
                codex_utils_pty::JobObject::terminate_process_handle(process_handle)
            }
        };
        if let Err(error) = result {
            warn!("Failed to terminate Windows MCP process: {error}");
        }
    }

    #[cfg(not(any(unix, windows)))]
    fn terminate_discovery(&self, _reason: StdioTermination) -> io::Result<()> {
        Err(io::Error::other("MCP stdio restart is unsupported"))
    }

    #[cfg(not(any(unix, windows)))]
    fn terminate(&self) {}
}

impl StdioServerProcessHandle {
    fn local(
        program_name: String,
        terminator: Option<LocalProcessTerminator>,
        stderr_reader: Option<watch::Sender<()>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            program_name,
            kind: StdioServerProcessKind::Local(terminator),
            terminated: AtomicBool::new(false),
            stderr_reader,
        })
    }

    pub(crate) fn executor(program_name: String, process: Arc<dyn ExecProcess>) -> Arc<Self> {
        Arc::new(Self {
            program_name,
            kind: StdioServerProcessKind::Executor(process),
            terminated: AtomicBool::new(false),
            stderr_reader: None,
        })
    }

    fn begin_termination(&self) -> bool {
        if self.terminated.swap(true, Ordering::AcqRel) {
            return false;
        }
        if let Some(stderr_reader) = &self.stderr_reader {
            stderr_reader.send_replace(());
        }
        true
    }

    fn terminate_discovery(&self, reason: StdioTermination) -> io::Result<()> {
        if !self.begin_termination() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "MCP stdio server is already shutting down",
            ));
        }
        let StdioServerProcessKind::Local(Some(terminator)) = &self.kind else {
            return Err(io::Error::other("missing local MCP discovery process"));
        };
        // Stop an uncontained replacement, but don't permit a retry on it.
        terminator
            .terminate_discovery(reason)
            .inspect_err(|_| terminator.terminate())
    }

    pub(crate) async fn terminate(&self) -> io::Result<()> {
        if !self.begin_termination() {
            return Ok(());
        }
        match &self.kind {
            StdioServerProcessKind::Local(Some(terminator)) => {
                terminator.terminate();
                Ok(())
            }
            StdioServerProcessKind::Local(None) => Ok(()),
            StdioServerProcessKind::Executor(process) => {
                process.terminate().await.map_err(|error| {
                    self.terminated.store(false, Ordering::Release);
                    io::Error::other(error)
                })
            }
        }
    }
}

impl Drop for StdioServerProcessHandle {
    fn drop(&mut self) {
        if self.terminated.swap(true, Ordering::AcqRel) {
            return;
        }

        match &self.kind {
            StdioServerProcessKind::Local(Some(terminator)) => {
                terminator.terminate();
            }
            StdioServerProcessKind::Local(None) => {}
            StdioServerProcessKind::Executor(process) => {
                let process = Arc::clone(process);
                let program_name = self.program_name.clone();
                let Ok(handle) = Handle::try_current() else {
                    warn!(
                        "Could not schedule remote MCP server process termination on drop ({}): no Tokio runtime is available",
                        self.program_name
                    );
                    return;
                };

                std::mem::drop(handle.spawn(async move {
                    if let Err(error) = process.terminate().await {
                        warn!(
                            "Failed to terminate remote MCP server process on drop ({program_name}): {error}"
                        );
                    }
                }));
            }
        }
        if let Some(stderr_reader) = &self.stderr_reader {
            stderr_reader.send_replace(());
        }
    }
}

// Remote public implementation.

/// Starts MCP stdio servers through the executor process API.
///
/// MCP framing still runs in the orchestrator. The executor only owns the
/// child process and transports raw stdin/stdout/stderr bytes, so it does not
/// need to know about MCP methods such as `initialize` or `tools/list`.
///
/// Windows executor-backed servers retain the executor's normal descendant
/// lifetime. MCP-specific containment requires negotiated process ownership:
/// caller-controlled process IDs cannot safely select a destructive policy,
/// and a wrapper may exit while its descendants continue serving requests.
#[derive(Clone)]
pub struct ExecutorStdioServerLauncher {
    exec_backend: Arc<dyn ExecBackend>,
}

impl ExecutorStdioServerLauncher {
    /// Creates a stdio server launcher backed by the executor process API.
    pub fn new(exec_backend: Arc<dyn ExecBackend>) -> Self {
        Self { exec_backend }
    }
}

impl StdioServerLauncher for ExecutorStdioServerLauncher {
    fn launch(
        &self,
        command: StdioServerCommand,
    ) -> BoxFuture<'static, io::Result<StdioServerTransport>> {
        let exec_backend = Arc::clone(&self.exec_backend);
        async move { Self::launch_server(command, exec_backend).await }.boxed()
    }
}

// Remote private implementation.

impl private::Sealed for ExecutorStdioServerLauncher {}

impl ExecutorStdioServerLauncher {
    async fn launch_server(
        command: StdioServerCommand,
        exec_backend: Arc<dyn ExecBackend>,
    ) -> io::Result<StdioServerTransport> {
        let StdioServerCommand {
            program,
            args,
            env,
            env_vars,
            cwd,
            protocol_mode,
            replacement_slot: _,
        } = command;
        let Some(cwd) = cwd else {
            return Err(io::Error::other(
                "executor stdio server requires an explicit cwd",
            ));
        };
        let cwd: PathUri = LegacyAppPathString::from_path(Path::new(&cwd))
            .try_into()
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        let program_name = program.to_string_lossy().into_owned();
        let envs = create_env_overlay_for_remote_mcp_server(env, &env_vars);
        let remote_env_vars = remote_mcp_env_var_names(&env_vars);
        // The executor protocol carries argv/env as UTF-8 strings. Local stdio can
        // accept arbitrary OsString values because it calls the OS directly; remote
        // stdio must reject non-Unicode command, argument, or environment data
        // before sending an executor request.
        let argv = Self::process_api_argv(&program, &args).map_err(io::Error::other)?;
        let env = Self::process_api_env(envs).map_err(io::Error::other)?;
        let process_id = ExecutorProcessTransport::next_process_id();
        // Start the MCP server process on the executor with raw pipes. `tty=false`
        // keeps stdout as a clean protocol stream, while `pipe_stdin=true` lets
        // rmcp write JSON-RPC requests after the process starts.
        let started = exec_backend
            .start(ExecParams {
                metadata: Default::default(),
                process_id,
                argv,
                cwd,
                shell_snapshot: None,
                env_policy: Some(Self::remote_env_policy(&remote_env_vars, &env)),
                env,
                tty: false,
                pipe_stdin: true,
                arg0: None,
                sandbox: None,
                enforce_managed_network: false,
                managed_network: None,
                network_proxy: None,
            })
            .await
            .map_err(io::Error::other)?;

        let process =
            StdioServerProcessHandle::executor(program_name.clone(), Arc::clone(&started.process));
        Ok(StdioServerTransport {
            inner: StdioServerTransportInner::Executor(ExecutorProcessTransport::new(
                started.process,
                program_name,
            )),
            process,
            discovery_cleanup: None,
            protocol_mode,
        })
    }

    fn process_api_argv(program: &OsString, args: &[OsString]) -> Result<Vec<String>> {
        let mut argv = Vec::with_capacity(args.len() + 1);
        argv.push(Self::os_string_to_process_api_string(
            program.clone(),
            "command",
        )?);
        for arg in args {
            argv.push(Self::os_string_to_process_api_string(
                arg.clone(),
                "argument",
            )?);
        }
        Ok(argv)
    }

    fn process_api_env(env: HashMap<OsString, OsString>) -> Result<HashMap<String, String>> {
        env.into_iter()
            .map(|(key, value)| {
                Ok((
                    Self::os_string_to_process_api_string(key, "environment variable name")?,
                    Self::os_string_to_process_api_string(value, "environment variable value")?,
                ))
            })
            .collect()
    }

    fn os_string_to_process_api_string(value: OsString, label: &str) -> Result<String> {
        value
            .into_string()
            .map_err(|_| anyhow!("{label} must be valid Unicode for remote MCP stdio"))
    }

    fn remote_env_policy(
        remote_env_vars: &[String],
        env: &HashMap<String, String>,
    ) -> ExecEnvPolicy {
        let include_only = if remote_env_vars.is_empty() {
            Vec::new()
        } else {
            // `source = "remote"` means the value is read from the executor's
            // environment, not copied from Codex. Start from `All` only so the
            // named remote variable is available to the filter below; the
            // effective child env is still limited by `include_only`.
            // The orchestrator may be Unix while the executor is Windows.
            // Preserve Windows runtime initialization and temporary-directory
            // inputs even when explicit remote vars activate this allowlist.
            crate::utils::DEFAULT_ENV_VARS
                .iter()
                .chain(["SYSTEMROOT", "TEMP", "TMP", "SYSTEMDRIVE", "LOCALAPPDATA"].iter())
                .map(|name| (*name).to_string())
                .chain(remote_env_vars.iter().cloned())
                .collect()
        };
        ExecEnvPolicy {
            inherit: if remote_env_vars.is_empty() {
                ShellEnvironmentPolicyInherit::Core
            } else {
                ShellEnvironmentPolicyInherit::All
            },
            ignore_default_excludes: true,
            exclude: Vec::new(),
            // Apply overrides through the policy's Windows-aware merge, including
            // on older executors. Keep the final overlay for names filtered out below.
            r#set: env.clone(),
            include_only,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::config_types::EnvironmentVariablePattern;
    use codex_protocol::config_types::ShellEnvironmentPolicy;
    use codex_protocol::shell_environment;
    use pretty_assertions::assert_eq;

    #[test]
    fn remote_env_policy_uses_core_env_without_remote_source_vars() {
        let policy = ExecutorStdioServerLauncher::remote_env_policy(&[], &HashMap::new());

        assert_eq!(policy.inherit, ShellEnvironmentPolicyInherit::Core);
        assert!(policy.include_only.is_empty());
    }

    #[test]
    fn remote_env_policy_includes_remote_source_vars_without_full_env() {
        let policy = ExecutorStdioServerLauncher::remote_env_policy(
            &["REMOTE_TOKEN".to_string()],
            &HashMap::new(),
        );

        assert_eq!(policy.inherit, ShellEnvironmentPolicyInherit::All);
        assert!(
            policy.include_only.contains(&"REMOTE_TOKEN".to_string()),
            "remote source var should be included in executor env policy"
        );
        assert!(
            policy
                .include_only
                .contains(&crate::utils::DEFAULT_ENV_VARS[0].to_string()),
            "remote default env vars should remain available"
        );
    }

    #[test]
    fn remote_env_policy_effectively_filters_unrequested_vars() {
        let exec_policy = ExecutorStdioServerLauncher::remote_env_policy(
            &["REMOTE_TOKEN".to_string()],
            &HashMap::new(),
        );
        let mut policy = ShellEnvironmentPolicy {
            inherit: exec_policy.inherit,
            ignore_default_excludes: exec_policy.ignore_default_excludes,
            exclude: exec_policy
                .exclude
                .iter()
                .map(|pattern| EnvironmentVariablePattern::new_case_insensitive(pattern))
                .collect(),
            r#set: exec_policy.r#set,
            include_only: exec_policy
                .include_only
                .iter()
                .map(|pattern| EnvironmentVariablePattern::new_case_insensitive(pattern))
                .collect(),
            use_profile: false,
        };

        let env = shell_environment::create_env_from_vars(
            [
                ("PATH".to_string(), "/remote/bin".to_string()),
                ("SystemRoot".to_string(), r"C:\Windows".to_string()),
                ("SystemDrive".to_string(), "C:".to_string()),
                (
                    "LocalAppData".to_string(),
                    r"C:\Users\test\AppData\Local".to_string(),
                ),
                (
                    "TEMP".to_string(),
                    r"C:\Users\test\AppData\Local\Temp".to_string(),
                ),
                (
                    "TMP".to_string(),
                    r"C:\Users\test\AppData\Local\Temp".to_string(),
                ),
                ("REMOTE_TOKEN".to_string(), "remote-secret".to_string()),
                (
                    "UNREQUESTED_SECRET".to_string(),
                    "must-not-pass".to_string(),
                ),
            ],
            &policy,
            /*thread_id*/ None,
        );

        assert_eq!(env.get("PATH").map(String::as_str), Some("/remote/bin"));
        assert_eq!(env.get("SystemDrive").map(String::as_str), Some("C:"));
        assert_eq!(
            env.get("LocalAppData").map(String::as_str),
            Some(r"C:\Users\test\AppData\Local")
        );
        assert_eq!(
            env.get("SystemRoot").map(String::as_str),
            Some(r"C:\Windows")
        );
        for name in ["TEMP", "TMP"] {
            assert_eq!(
                env.get(name).map(String::as_str),
                Some(r"C:\Users\test\AppData\Local\Temp")
            );
        }
        assert_eq!(
            env.get("REMOTE_TOKEN").map(String::as_str),
            Some("remote-secret")
        );
        assert!(!env.contains_key("UNREQUESTED_SECRET"));

        let overrides = HashMap::from([
            ("SYSTEMDRIVE".to_string(), "D:".to_string()),
            ("LOCALAPPDATA".to_string(), r"D:\McpProfile".to_string()),
            ("EXPLICIT_ONLY".to_string(), "configured".to_string()),
        ]);
        policy.r#set = ExecutorStdioServerLauncher::remote_env_policy(
            &["REMOTE_TOKEN".to_string()],
            &overrides,
        )
        .r#set;
        let mut child_env =
            shell_environment::create_env_from_vars(env.clone(), &policy, /*thread_id*/ None);
        assert_eq!(
            child_env.get("LOCALAPPDATA").map(String::as_str),
            Some(r"D:\McpProfile")
        );
        // Older executors apply the final overlay with a case-sensitive merge.
        child_env.extend(overrides.clone());
        let mut expected = env;
        if cfg!(windows) {
            expected.remove("SystemDrive");
            expected.remove("LocalAppData");
        }
        expected.extend(overrides);
        assert_eq!(child_env, expected);
    }
}
