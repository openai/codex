use std::path::Path;
use std::path::PathBuf;
#[cfg(unix)]
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;
use serde::Serialize;
use tokio::fs;
#[cfg(unix)]
use tokio::process::Command;
use tokio::time::sleep;

const STOP_POLL_INTERVAL: Duration = Duration::from_millis(50);
const STOP_GRACE_PERIOD: Duration = Duration::from_secs(60);
const STOP_TIMEOUT: Duration = Duration::from_secs(70);
const START_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) struct PidBackend {
    codex_bin: PathBuf,
    pid_file: PathBuf,
    lock_file: PathBuf,
    command_kind: PidCommandKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PidRecord {
    pid: u32,
    process_start_time: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PidFileState {
    Missing,
    Starting,
    Running(PidRecord),
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(unix), allow(dead_code))]
enum PidCommandKind {
    AppServer { remote_control_enabled: bool },
    UpdateLoop,
}

impl PidBackend {
    pub(crate) fn new(codex_bin: PathBuf, pid_file: PathBuf, remote_control_enabled: bool) -> Self {
        let lock_file = pid_file.with_extension("pid.lock");
        Self {
            codex_bin,
            pid_file,
            lock_file,
            command_kind: PidCommandKind::AppServer {
                remote_control_enabled,
            },
        }
    }

    pub(crate) fn new_update_loop(codex_bin: PathBuf, pid_file: PathBuf) -> Self {
        let lock_file = pid_file.with_extension("pid.lock");
        Self {
            codex_bin,
            pid_file,
            lock_file,
            command_kind: PidCommandKind::UpdateLoop,
        }
    }

    pub(crate) async fn is_starting_or_running(&self) -> Result<bool> {
        loop {
            match self.read_pid_file_state().await? {
                PidFileState::Missing => return Ok(false),
                PidFileState::Starting => return Ok(true),
                PidFileState::Running(record) => {
                    if self.record_is_active(&record).await? {
                        return Ok(true);
                    }
                    match self.refresh_after_stale_record(&record).await? {
                        PidFileState::Missing => return Ok(false),
                        PidFileState::Starting | PidFileState::Running(_) => continue,
                    }
                }
            }
        }
    }

    #[cfg(unix)]
    pub(crate) async fn start(&self) -> Result<Option<u32>> {
        if let Some(parent) = self.pid_file.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create pid directory {}", parent.display()))?;
        }
        let reservation_lock = self.acquire_reservation_lock().await?;
        let _pid_file = loop {
            match fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&self.pid_file)
                .await
            {
                Ok(pid_file) => break pid_file,
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    match self.read_pid_file_state_with_lock_held().await? {
                        PidFileState::Missing => continue,
                        PidFileState::Running(record) => {
                            if self.record_is_active(&record).await? {
                                return Ok(None);
                            }
                            let _ = fs::remove_file(&self.pid_file).await;
                            continue;
                        }
                        PidFileState::Starting => {
                            unreachable!("lock holder cannot observe starting")
                        }
                    }
                }
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("failed to reserve pid file {}", self.pid_file.display())
                    });
                }
            }
        };
        let mut command = Command::new(&self.codex_bin);
        command
            .args(self.command_args())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(unix)]
        {
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }

        let child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                let _ = fs::remove_file(&self.pid_file).await;
                return Err(err).with_context(|| {
                    format!(
                        "failed to spawn detached app-server process using {}",
                        self.codex_bin.display()
                    )
                });
            }
        };
        let pid = child
            .id()
            .context("spawned app-server process has no pid")?;
        let record = match read_process_start_time(pid).await {
            Ok(process_start_time) => PidRecord {
                pid,
                process_start_time,
            },
            Err(err) => {
                let _ = self.terminate_process(pid);
                let _ = fs::remove_file(&self.pid_file).await;
                return Err(err);
            }
        };
        let contents = serde_json::to_vec(&record).context("failed to serialize pid record")?;
        let temp_pid_file = self.pid_file.with_extension("pid.tmp");
        if let Err(err) = fs::write(&temp_pid_file, &contents).await {
            let _ = self.terminate_process(pid);
            let _ = fs::remove_file(&self.pid_file).await;
            return Err(err).with_context(|| {
                format!("failed to write pid temp file {}", temp_pid_file.display())
            });
        }
        if let Err(err) = fs::rename(&temp_pid_file, &self.pid_file).await {
            let _ = self.terminate_process(pid);
            let _ = fs::remove_file(&temp_pid_file).await;
            let _ = fs::remove_file(&self.pid_file).await;
            return Err(err).with_context(|| {
                format!("failed to publish pid file {}", self.pid_file.display())
            });
        }
        drop(reservation_lock);
        Ok(Some(pid))
    }

    #[cfg(not(unix))]
    pub(crate) async fn start(&self) -> Result<Option<u32>> {
        bail!("pid-managed app-server startup is unsupported on this platform")
    }

    pub(crate) async fn stop(&self) -> Result<()> {
        loop {
            let Some(record) = self.wait_for_pid_start().await? else {
                return Ok(());
            };
            if !self.record_is_active(&record).await? {
                match self.refresh_after_stale_record(&record).await? {
                    PidFileState::Missing => return Ok(()),
                    PidFileState::Starting | PidFileState::Running(_) => continue,
                }
            }

            let pid = record.pid;
            self.terminate_process(pid)?;
            let started_at = tokio::time::Instant::now();
            let deadline = tokio::time::Instant::now() + STOP_TIMEOUT;
            let mut forced = false;
            while tokio::time::Instant::now() < deadline {
                if !self.record_is_active(&record).await? {
                    match self.refresh_after_stale_record(&record).await? {
                        PidFileState::Missing => return Ok(()),
                        PidFileState::Starting | PidFileState::Running(_) => break,
                    }
                }
                if !forced && started_at.elapsed() >= STOP_GRACE_PERIOD {
                    self.force_terminate_process(pid)?;
                    forced = true;
                }
                sleep(STOP_POLL_INTERVAL).await;
            }

            if self.record_is_active(&record).await? {
                bail!("timed out waiting for pid-managed app server {pid} to stop");
            }
        }
    }

    async fn wait_for_pid_start(&self) -> Result<Option<PidRecord>> {
        let deadline = tokio::time::Instant::now() + START_TIMEOUT;
        loop {
            match self.read_pid_file_state().await? {
                PidFileState::Missing => return Ok(None),
                PidFileState::Running(record) => return Ok(Some(record)),
                PidFileState::Starting if tokio::time::Instant::now() < deadline => {
                    sleep(STOP_POLL_INTERVAL).await;
                }
                PidFileState::Starting => {
                    bail!(
                        "timed out waiting for pid reservation in {} to finish initializing",
                        self.pid_file.display()
                    );
                }
            }
        }
    }

    async fn read_pid_file_state(&self) -> Result<PidFileState> {
        let contents = match fs::read_to_string(&self.pid_file).await {
            Ok(contents) => contents,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return if reservation_lock_is_active(&self.lock_file).await? {
                    Ok(PidFileState::Starting)
                } else {
                    Ok(PidFileState::Missing)
                };
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to read pid file {}", self.pid_file.display())
                });
            }
        };
        if contents.trim().is_empty() {
            match inspect_empty_pid_reservation(&self.pid_file, &self.lock_file).await? {
                EmptyPidReservation::Active => {
                    return Ok(PidFileState::Starting);
                }
                EmptyPidReservation::Stale => {
                    return Ok(PidFileState::Missing);
                }
                EmptyPidReservation::Record(record) => return Ok(PidFileState::Running(record)),
            }
        }
        let record = serde_json::from_str(&contents)
            .with_context(|| format!("invalid pid file contents in {}", self.pid_file.display()))?;
        Ok(PidFileState::Running(record))
    }

    async fn read_pid_file_state_with_lock_held(&self) -> Result<PidFileState> {
        let contents = match fs::read_to_string(&self.pid_file).await {
            Ok(contents) => contents,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(PidFileState::Missing);
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to read pid file {}", self.pid_file.display())
                });
            }
        };
        if contents.trim().is_empty() {
            let _ = fs::remove_file(&self.pid_file).await;
            return Ok(PidFileState::Missing);
        }
        let record = serde_json::from_str(&contents)
            .with_context(|| format!("invalid pid file contents in {}", self.pid_file.display()))?;
        Ok(PidFileState::Running(record))
    }

    async fn refresh_after_stale_record(&self, expected: &PidRecord) -> Result<PidFileState> {
        let reservation_lock = self.acquire_reservation_lock().await?;
        let state = match self.read_pid_file_state_with_lock_held().await? {
            PidFileState::Running(record) if record == *expected => {
                let _ = fs::remove_file(&self.pid_file).await;
                PidFileState::Missing
            }
            state => state,
        };
        drop(reservation_lock);
        Ok(state)
    }

    async fn acquire_reservation_lock(&self) -> Result<fs::File> {
        let reservation_lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&self.lock_file)
            .await
            .with_context(|| {
                format!("failed to open pid lock file {}", self.lock_file.display())
            })?;
        let lock_deadline = tokio::time::Instant::now() + START_TIMEOUT;
        while !try_lock_file(&reservation_lock)? {
            if tokio::time::Instant::now() >= lock_deadline {
                bail!(
                    "timed out waiting for pid lock {}",
                    self.lock_file.display()
                );
            }
            sleep(STOP_POLL_INTERVAL).await;
        }
        Ok(reservation_lock)
    }

    #[cfg(unix)]
    fn command_args(&self) -> Vec<&'static str> {
        match self.command_kind {
            PidCommandKind::AppServer {
                remote_control_enabled: true,
            } => vec![
                "--enable",
                "remote_control",
                "app-server",
                "--listen",
                "unix://",
            ],
            PidCommandKind::AppServer {
                remote_control_enabled: false,
            } => vec!["app-server", "--listen", "unix://"],
            PidCommandKind::UpdateLoop => vec!["app-server", "daemon", "pid-update-loop"],
        }
    }

    fn terminate_process(&self, pid: u32) -> Result<()> {
        match self.command_kind {
            PidCommandKind::AppServer { .. } => terminate_process(pid),
            PidCommandKind::UpdateLoop => terminate_process(pid),
        }
    }

    fn force_terminate_process(&self, pid: u32) -> Result<()> {
        match self.command_kind {
            PidCommandKind::AppServer { .. } => force_terminate_process(pid),
            PidCommandKind::UpdateLoop => force_terminate_process_group(pid),
        }
    }

    async fn record_is_active(&self, record: &PidRecord) -> Result<bool> {
        process_matches_record(record).await
    }
}

#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    let result = unsafe { libc::kill(pid, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(unix)]
fn terminate_process(pid: u32) -> Result<()> {
    let raw_pid = libc::pid_t::try_from(pid)
        .with_context(|| format!("pid-managed app server pid {pid} is out of range"))?;
    let result = unsafe { libc::kill(raw_pid, libc::SIGTERM) };
    if result == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(err).with_context(|| format!("failed to terminate pid-managed app server {pid}"))
}

#[cfg(unix)]
fn force_terminate_process(pid: u32) -> Result<()> {
    let raw_pid = libc::pid_t::try_from(pid)
        .with_context(|| format!("pid-managed app server pid {pid} is out of range"))?;
    let result = unsafe { libc::kill(raw_pid, libc::SIGKILL) };
    if result == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(err).with_context(|| format!("failed to force terminate pid-managed app server {pid}"))
}

#[cfg(unix)]
fn force_terminate_process_group(pid: u32) -> Result<()> {
    let raw_pid = libc::pid_t::try_from(pid)
        .with_context(|| format!("pid-managed updater pid {pid} is out of range"))?;
    let result = unsafe { libc::kill(-raw_pid, libc::SIGKILL) };
    if result == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(err).with_context(|| format!("failed to force terminate pid-managed updater group {pid}"))
}

#[cfg(not(unix))]
fn terminate_process(_pid: u32) -> Result<()> {
    bail!("pid-managed app-server shutdown is unsupported on this platform")
}

#[cfg(not(unix))]
fn force_terminate_process(_pid: u32) -> Result<()> {
    bail!("pid-managed app-server shutdown is unsupported on this platform")
}

#[cfg(not(unix))]
fn force_terminate_process_group(_pid: u32) -> Result<()> {
    bail!("pid-managed updater shutdown is unsupported on this platform")
}

#[cfg(unix)]
async fn process_matches_record(record: &PidRecord) -> Result<bool> {
    if !process_exists(record.pid) {
        return Ok(false);
    }

    match read_process_start_time(record.pid).await {
        Ok(start_time) => Ok(start_time == record.process_start_time),
        Err(_err) if !process_exists(record.pid) => Ok(false),
        Err(err) => Err(err),
    }
}

#[cfg(not(unix))]
async fn process_matches_record(_record: &PidRecord) -> Result<bool> {
    Ok(false)
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(unix), allow(dead_code))]
enum EmptyPidReservation {
    Active,
    Stale,
    Record(PidRecord),
}

#[cfg(unix)]
fn try_lock_file(file: &fs::File) -> Result<bool> {
    use std::os::fd::AsRawFd;

    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        return Ok(true);
    }

    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::EWOULDBLOCK) {
        return Ok(false);
    }
    Err(err).context("failed to lock pid reservation")
}

#[cfg(not(unix))]
fn try_lock_file(_file: &fs::File) -> Result<bool> {
    bail!("pid-managed app-server startup is unsupported on this platform")
}

#[cfg(unix)]
async fn reservation_lock_is_active(path: &Path) -> Result<bool> {
    let file = match fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .await
    {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(false);
        }
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to inspect pid lock file {}", path.display()));
        }
    };
    Ok(!try_lock_file(&file)?)
}

#[cfg(not(unix))]
async fn reservation_lock_is_active(_path: &Path) -> Result<bool> {
    Ok(false)
}

#[cfg(unix)]
async fn inspect_empty_pid_reservation(
    pid_path: &Path,
    lock_path: &Path,
) -> Result<EmptyPidReservation> {
    let file = match fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .await
    {
        Ok(file) => file,
        Err(err) => {
            return Err(err).with_context(|| {
                format!("failed to inspect pid lock file {}", lock_path.display())
            });
        }
    };
    if !try_lock_file(&file)? {
        return Ok(EmptyPidReservation::Active);
    }

    let contents = match fs::read_to_string(pid_path).await {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(EmptyPidReservation::Stale);
        }
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to reread pid file {}", pid_path.display()));
        }
    };
    if contents.trim().is_empty() {
        let _ = fs::remove_file(pid_path).await;
        return Ok(EmptyPidReservation::Stale);
    }

    let record = serde_json::from_str(&contents)
        .with_context(|| format!("invalid pid file contents in {}", pid_path.display()))?;
    Ok(EmptyPidReservation::Record(record))
}

#[cfg(not(unix))]
async fn inspect_empty_pid_reservation(
    _pid_path: &Path,
    _lock_path: &Path,
) -> Result<EmptyPidReservation> {
    Ok(EmptyPidReservation::Stale)
}

#[cfg(unix)]
async fn read_process_start_time(pid: u32) -> Result<String> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "lstart="])
        .output()
        .await
        .context("failed to invoke ps for pid-managed app server")?;
    if !output.status.success() {
        bail!("failed to read start time for pid-managed app server {pid}");
    }

    let start_time = String::from_utf8(output.stdout)
        .context("pid-managed app server start time was not utf-8")?;
    let start_time = start_time.trim();
    if start_time.is_empty() {
        bail!("pid-managed app server {pid} has no recorded start time");
    }
    Ok(start_time.to_string())
}

#[cfg(all(test, unix))]
#[path = "pid_tests.rs"]
mod tests;
