use std::collections::HashSet;
use tokio::task::JoinHandle;
use tracing::warn;

/// Tracks the (recursive) descendants of a process by using `kqueue` to watch for fork events, and
/// `proc_listchildpids` to list the children of a process.
pub(crate) struct PidTracker {
    kq: libc::c_int,
    handle: JoinHandle<HashSet<i32>>,
}

impl PidTracker {
    pub(crate) fn new(root_pid: i32) -> Option<Self> {
        if root_pid <= 0 {
            return None;
        }

        let kq = unsafe { libc::kqueue() };
        let handle = tokio::task::spawn_blocking(move || track_descendants(kq, root_pid));

        Some(Self { kq, handle })
    }

    pub(crate) async fn stop(self) -> HashSet<i32> {
        trigger_stop_event(self.kq);
        self.handle.await.unwrap_or_default()
    }
}

unsafe extern "C" {
    fn proc_listchildpids(
        ppid: libc::c_int,
        buffer: *mut libc::c_void,
        buffersize: libc::c_int,
    ) -> libc::c_int;
}

/// Wrap proc_listchildpids.
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

enum WatchPidError {
    ProcessGone,
    Other(std::io::Error),
}

/// Add `pid` to the watch list in `kq`.
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

/// Watch `pid` and its children, updating `seen` and `active` sets.
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
                warn!("failed to watch pid {pid}: {err}");
                active.remove(&pid);
                return;
            }
        }
    }

    if should_recurse {
        watch_children(kq, pid, seen, active);
    }
}
const STOP_IDENT: libc::uintptr_t = 1;

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

/// Put all of the above together to track all the descendants of `root_pid`.
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

    let mut seen: HashSet<i32> = HashSet::new();
    let mut active: HashSet<i32> = HashSet::new();

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
                std::ptr::null::<libc::kevent>(),
                0,
                events.as_mut_ptr(),
                EVENTS_CAP as libc::c_int,
                std::ptr::null(),
            )
        };

        if nev < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            break;
        }

        if nev == 0 {
            continue;
        }

        for ev in events.iter().take(nev as usize) {
            let pid = ev.ident as i32;

            if ev.filter == libc::EVFILT_USER && ev.ident == STOP_IDENT {
                stop_requested = true;
                break;
            }

            if (ev.flags & libc::EV_ERROR) != 0 {
                if ev.data == libc::ESRCH as isize {
                    active.remove(&pid);
                }
                continue;
            }

            if (ev.fflags & libc::NOTE_FORK) != 0 {
                watch_children(kq, pid, &mut seen, &mut active);
            }

            if (ev.fflags & libc::NOTE_EXIT) != 0 {
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
    use std::process::Command;
    use std::process::Stdio;
    use std::time::Duration;

    #[test]
    fn pid_is_alive_detects_current_process() {
        let pid = std::process::id() as i32;
        assert!(pid_is_alive(pid));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn list_child_pids_includes_spawned_child() {
        let mut child = Command::new("/bin/sleep")
            .arg("5")
            .stdin(Stdio::null())
            .spawn()
            .expect("failed to spawn child process");

        let child_pid = child.id() as i32;
        let parent_pid = std::process::id() as i32;

        let mut found = false;
        for _ in 0..100 {
            if list_child_pids(parent_pid).contains(&child_pid) {
                found = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        let _ = child.kill();
        let _ = child.wait();

        assert!(found, "expected to find child pid {child_pid} in list");
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn pid_tracker_collects_spawned_children() {
        let tracker = PidTracker::new(std::process::id() as i32).expect("failed to create tracker");

        let mut child = Command::new("/bin/sleep")
            .arg("0.1")
            .stdin(Stdio::null())
            .spawn()
            .expect("failed to spawn child process");

        let child_pid = child.id() as i32;
        let parent_pid = std::process::id() as i32;

        let _ = child.wait();

        let seen = tracker.stop().await;

        assert!(
            seen.contains(&parent_pid),
            "expected tracker to include parent pid {parent_pid}"
        );
        assert!(
            seen.contains(&child_pid),
            "expected tracker to include child pid {child_pid}"
        );
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn pid_tracker_collects_bash_subshell_descendants() {
        let tracker = PidTracker::new(std::process::id() as i32).expect("failed to create tracker");

        let child = Command::new("/bin/bash")
            .arg("-c")
            .arg("(sleep 0.1 & echo $!; wait)")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn bash");

        let output = child.wait_with_output().unwrap().stdout;
        let subshell_pid = String::from_utf8_lossy(&output)
            .trim()
            .parse::<i32>()
            .expect("failed to parse subshell pid");

        let seen = tracker.stop().await;

        assert!(
            seen.contains(&subshell_pid),
            "expected tracker to include subshell pid {subshell_pid}"
        );
    }
}
