use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use tokio::task::JoinHandle;
use tracing::warn;

/// Tracks the descendants of a process by using `kqueue` to watch for fork/exec events, and
/// `proc_listchildpids` to list the children of a process.
pub(crate) struct PidTracker {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<HashSet<i32>>,
}

impl PidTracker {
    pub(crate) fn new(root_pid: i32) -> Option<Self> {
        if root_pid <= 0 {
            return None;
        }

        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = stop.clone();
        let handle = tokio::task::spawn_blocking(move || track_descendants(root_pid, stop_clone));

        Some(Self { stop, handle })
    }

    pub(crate) async fn stop(self) -> HashSet<i32> {
        self.stop.store(true, Ordering::SeqCst);
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
            if count < 0 {
                let err = std::io::Error::last_os_error().raw_os_error();
                if err == Some(libc::ESRCH) {
                    return Vec::new();
                }
                return Vec::new();
            }
            if count == 0 {
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

fn ensure_children(
    kq: libc::c_int,
    parent: i32,
    seen: &mut HashSet<i32>,
    active: &mut HashSet<i32>,
) {
    for child_pid in list_child_pids(parent) {
        add_pid_watch(kq, child_pid, seen, active);
    }
}

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
        ensure_children(kq, pid, seen, active);
    }
}

fn track_descendants(root_pid: i32, stop: Arc<AtomicBool>) -> HashSet<i32> {
    use std::thread;
    use std::time::Duration;

    let kq = unsafe { libc::kqueue() };
    if kq < 0 {
        let mut seen = HashSet::new();
        seen.insert(root_pid);
        return seen;
    }

    let mut seen: HashSet<i32> = HashSet::new();
    let mut active: HashSet<i32> = HashSet::new();

    add_pid_watch(kq, root_pid, &mut seen, &mut active);

    const EVENTS_CAP: usize = 32;
    let mut events: [libc::kevent; EVENTS_CAP] =
        unsafe { std::mem::MaybeUninit::zeroed().assume_init() };

    while !stop.load(Ordering::Relaxed) {
        if active.is_empty() {
            if !pid_is_alive(root_pid) {
                break;
            }
            add_pid_watch(kq, root_pid, &mut seen, &mut active);
            if active.is_empty() {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
        }

        let timeout = libc::timespec {
            tv_sec: 0,
            tv_nsec: 50_000_000,
        };

        let nev = unsafe {
            libc::kevent(
                kq,
                std::ptr::null::<libc::kevent>(),
                0,
                events.as_mut_ptr(),
                EVENTS_CAP as libc::c_int,
                &timeout,
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

            if (ev.flags & libc::EV_ERROR) != 0 {
                if ev.data == libc::ESRCH as isize {
                    active.remove(&pid);
                }
                continue;
            }

            if (ev.fflags & libc::NOTE_FORK) != 0 {
                ensure_children(kq, pid, &mut seen, &mut active);
            }

            if (ev.fflags & libc::NOTE_EXIT) != 0 {
                active.remove(&pid);
            }
        }
    }

    let _ = unsafe { libc::close(kq) };

    seen
}
