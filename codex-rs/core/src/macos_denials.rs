#[cfg(target_os = "macos")]
use std::collections::HashSet;

#[cfg(target_os = "macos")]
use tokio::io::AsyncBufReadExt;
#[cfg(target_os = "macos")]
use tokio::process::Child;
#[cfg(target_os = "macos")]
use tokio::task::JoinHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SandboxDenial {
    pub(crate) name: String,
    pub(crate) capability: String,
}

pub(crate) struct SeatbeltDenialLogger {
    #[cfg(target_os = "macos")]
    log_stream: Child,
    #[cfg(target_os = "macos")]
    pid_tracker: Option<PidTracker>,
    #[cfg(target_os = "macos")]
    log_reader: Option<JoinHandle<Vec<u8>>>,
}

impl SeatbeltDenialLogger {
    #[cfg(target_os = "macos")]
    pub(crate) fn new() -> Option<Self> {
        let mut log_stream = start_log_stream()?;
        let stdout = log_stream.stdout.take()?;
        let log_reader = tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut logs = Vec::new();
            let mut chunk = Vec::new();
            loop {
                match reader.read_until(b'\n', &mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        logs.extend_from_slice(&chunk);
                        chunk.clear();
                    }
                }
            }
            logs
        });

        Some(Self {
            log_stream,
            pid_tracker: None,
            log_reader: Some(log_reader),
        })
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn new() -> Option<Self> {
        None
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn on_child_pid(&mut self, child_pid: Option<u32>) {
        if let Some(root_pid) = child_pid {
            self.pid_tracker = PidTracker::new(root_pid as i32);
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn on_child_pid(&mut self, child_pid: Option<u32>) {
        let _ = child_pid;
    }

    #[cfg(target_os = "macos")]
    pub(crate) async fn finish(mut self) -> Vec<SandboxDenial> {
        let pid_set = match self.pid_tracker {
            Some(tracker) => tracker.stop().await,
            None => Default::default(),
        };

        if pid_set.is_empty() {
            return Vec::new();
        }

        let _ = self.log_stream.kill().await;
        let _ = self.log_stream.wait().await;

        let logs_bytes = match self.log_reader.take() {
            Some(handle) => handle.await.unwrap_or_default(),
            None => Vec::new(),
        };
        let logs = String::from_utf8_lossy(&logs_bytes);

        let mut seen: HashSet<(String, String)> = HashSet::new();
        let mut denials = Vec::new();
        for line in logs.lines() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line)
                && let Some(msg) = json.get("eventMessage").and_then(|v| v.as_str())
                && let Some((pid, name, capability)) = parse_message(msg)
                && pid_set.contains(&pid)
                && seen.insert((name.clone(), capability.clone()))
            {
                denials.push(SandboxDenial { name, capability });
            }
        }
        denials
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) async fn finish(self) -> Vec<SandboxDenial> {
        Vec::new()
    }
}

pub(crate) fn format_sandbox_denials(denials: &[SandboxDenial]) -> Option<Vec<u8>> {
    if denials.is_empty() {
        return None;
    }

    let mut formatted = String::from("\n=== Sandbox denials ===\n");
    for SandboxDenial { name, capability } in denials {
        formatted.push_str(&format!("({name}) {capability}\n"));
    }
    Some(formatted.into_bytes())
}

#[cfg(target_os = "macos")]
fn start_log_stream() -> Option<Child> {
    use std::process::Stdio;

    const PREDICATE: &str = r#"(((processID == 0) AND (senderImagePath CONTAINS "/Sandbox")) OR (subsystem == "com.apple.sandbox.reporting"))"#;

    tokio::process::Command::new("log")
        .args(["stream", "--style", "ndjson", "--predicate", PREDICATE])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()
}

#[cfg(target_os = "macos")]
fn parse_message(msg: &str) -> Option<(i32, String, String)> {
    // Example message:
    // Sandbox: processname(1234) deny(1) capability-name args...
    static RE: std::sync::OnceLock<regex_lite::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        #[expect(clippy::unwrap_used)]
        regex_lite::Regex::new(r"^Sandbox:\s*(.+?)\((\d+)\)\s+deny\(.*?\)\s*(.+)$").unwrap()
    });

    let (_, [name, pid_str, capability]) = re.captures(msg)?.extract();
    let pid = pid_str.trim().parse::<i32>().ok()?;
    Some((pid, name.to_string(), capability.to_string()))
}

#[cfg(target_os = "macos")]
struct PidTracker {
    kq: libc::c_int,
    handle: JoinHandle<HashSet<i32>>,
}

#[cfg(target_os = "macos")]
impl PidTracker {
    fn new(root_pid: i32) -> Option<Self> {
        if root_pid <= 0 {
            return None;
        }

        let kq = unsafe { libc::kqueue() };
        let handle = tokio::task::spawn_blocking(move || track_descendants(kq, root_pid));

        Some(Self { kq, handle })
    }

    async fn stop(self) -> HashSet<i32> {
        trigger_stop_event(self.kq);
        self.handle.await.unwrap_or_default()
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn proc_listchildpids(
        ppid: libc::c_int,
        buffer: *mut libc::c_void,
        buffersize: libc::c_int,
    ) -> libc::c_int;
}

#[cfg(target_os = "macos")]
fn list_child_pids(parent: i32) -> Vec<i32> {
    unsafe {
        let mut capacity: usize = 16;
        loop {
            let mut buf: Vec<i32> = vec![0; capacity];
            let count = proc_listchildpids(
                parent as libc::c_int,
                buf.as_mut_ptr() as *mut libc::c_void,
                (buf.len() * std::mem::size_of::<i32>()) as libc::c_int,
            );
            if count <= 0 {
                return Vec::new();
            }
            let returned = count as usize;
            if returned < capacity {
                buf.truncate(returned);
                return buf;
            }
            capacity = capacity.saturating_mul(2).max(returned + 16);
        }
    }
}

#[cfg(target_os = "macos")]
fn pid_is_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    let res = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if res == 0 {
        true
    } else {
        matches!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EPERM)
        )
    }
}

#[cfg(target_os = "macos")]
enum WatchPidError {
    ProcessGone,
    Other(std::io::Error),
}

#[cfg(target_os = "macos")]
fn watch_pid(kq: libc::c_int, pid: i32) -> Result<(), WatchPidError> {
    if pid <= 0 {
        return Err(WatchPidError::ProcessGone);
    }

    let kev = libc::kevent {
        ident: pid as libc::uintptr_t,
        filter: libc::EVFILT_PROC,
        flags: libc::EV_ADD | libc::EV_CLEAR,
        fflags: libc::NOTE_FORK | libc::NOTE_EXEC | libc::NOTE_EXIT,
        data: 0,
        udata: std::ptr::null_mut(),
    };

    let res = unsafe { libc::kevent(kq, &kev, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
    if res < 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ESRCH) {
            Err(WatchPidError::ProcessGone)
        } else {
            Err(WatchPidError::Other(err))
        }
    } else {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn watch_children(
    kq: libc::c_int,
    parent: i32,
    seen: &mut HashSet<i32>,
    active: &mut HashSet<i32>,
) {
    for child_pid in list_child_pids(parent) {
        add_pid_watch(kq, child_pid, seen, active);
    }
}

#[cfg(target_os = "macos")]
fn add_pid_watch(kq: libc::c_int, pid: i32, seen: &mut HashSet<i32>, active: &mut HashSet<i32>) {
    if pid <= 0 {
        return;
    }

    let newly_seen = seen.insert(pid);
    let mut should_recurse = newly_seen;

    if active.insert(pid) {
        match watch_pid(kq, pid) {
            Ok(()) => {
                should_recurse = true;
            }
            Err(WatchPidError::ProcessGone) => {
                active.remove(&pid);
                return;
            }
            Err(WatchPidError::Other(err)) => {
                tracing::warn!("failed to watch pid {pid}: {err}");
                active.remove(&pid);
                return;
            }
        }
    }

    if should_recurse {
        watch_children(kq, pid, seen, active);
    }
}

#[cfg(target_os = "macos")]
const STOP_IDENT: libc::uintptr_t = 1;

#[cfg(target_os = "macos")]
fn register_stop_event(kq: libc::c_int) -> bool {
    let kev = libc::kevent {
        ident: STOP_IDENT,
        filter: libc::EVFILT_USER,
        flags: libc::EV_ADD | libc::EV_CLEAR,
        fflags: 0,
        data: 0,
        udata: std::ptr::null_mut(),
    };

    let res = unsafe { libc::kevent(kq, &kev, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
    res >= 0
}

#[cfg(target_os = "macos")]
fn trigger_stop_event(kq: libc::c_int) {
    if kq < 0 {
        return;
    }

    let kev = libc::kevent {
        ident: STOP_IDENT,
        filter: libc::EVFILT_USER,
        flags: 0,
        fflags: libc::NOTE_TRIGGER,
        data: 0,
        udata: std::ptr::null_mut(),
    };

    let _ = unsafe { libc::kevent(kq, &kev, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
}

#[cfg(target_os = "macos")]
fn track_descendants(kq: libc::c_int, root_pid: i32) -> HashSet<i32> {
    if kq < 0 {
        let mut seen = HashSet::new();
        seen.insert(root_pid);
        return seen;
    }

    if !register_stop_event(kq) {
        let mut seen = HashSet::new();
        seen.insert(root_pid);
        let _ = unsafe { libc::close(kq) };
        return seen;
    }

    let mut seen = HashSet::new();
    let mut active = HashSet::new();

    add_pid_watch(kq, root_pid, &mut seen, &mut active);

    const EVENTS_CAP: usize = 32;
    let mut events: [libc::kevent; EVENTS_CAP] =
        unsafe { std::mem::MaybeUninit::zeroed().assume_init() };

    let mut stop_requested = false;
    loop {
        if active.is_empty() {
            if !pid_is_alive(root_pid) {
                break;
            }
            add_pid_watch(kq, root_pid, &mut seen, &mut active);
            if active.is_empty() {
                continue;
            }
        }

        let nev = unsafe {
            libc::kevent(
                kq,
                std::ptr::null(),
                0,
                events.as_mut_ptr(),
                EVENTS_CAP as libc::c_int,
                std::ptr::null(),
            )
        };

        if nev < 0 {
            break;
        }

        for ev in events.iter().take(nev as usize) {
            if ev.filter == libc::EVFILT_USER && ev.ident == STOP_IDENT {
                stop_requested = true;
                continue;
            }

            if ev.filter != libc::EVFILT_PROC {
                continue;
            }

            let pid = ev.ident as i32;
            if ev.fflags & libc::NOTE_FORK != 0
                || ev.fflags & libc::NOTE_EXEC != 0
                || ev.fflags & libc::NOTE_EXIT != 0
            {
                watch_children(kq, pid, &mut seen, &mut active);
            }
            if ev.fflags & libc::NOTE_EXIT != 0 || !pid_is_alive(pid) {
                active.remove(&pid);
            }
        }

        if stop_requested {
            break;
        }
    }

    let _ = unsafe { libc::close(kq) };
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_denials_for_stderr() {
        let formatted = format_sandbox_denials(&[SandboxDenial {
            name: "touch".to_string(),
            capability: "file-write-create /private/tmp/nope".to_string(),
        }])
        .expect("denial text");

        assert_eq!(
            String::from_utf8_lossy(&formatted),
            "\n=== Sandbox denials ===\n(touch) file-write-create /private/tmp/nope\n"
        );
    }
}
