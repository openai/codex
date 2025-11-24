#[macro_use]
extern crate cfg_if;
#[cfg_attr(not(any(target_os = "redox")), macro_use)]
extern crate nix;

#[macro_use]
mod common;
mod mount;
mod sys;
#[cfg(not(target_os = "redox"))]
mod test_dir;
mod test_errno;
mod test_fcntl;
#[cfg(linux_android)]
mod test_kmod;
#[cfg(any(
    freebsdlike,
    all(target_os = "linux", not(target_env = "ohos")),
    target_os = "netbsd"
))]
mod test_mq;
#[cfg(not(target_os = "redox"))]
mod test_net;
mod test_nix_path;
mod test_poll;
#[cfg(not(any(
    target_os = "redox",
    target_os = "fuchsia",
    target_os = "haiku"
)))]
mod test_pty;
#[cfg(any(
    linux_android,
    target_os = "dragonfly",
    all(target_os = "freebsd", fbsd14),
))]
mod test_sched;
#[cfg(any(linux_android, freebsdlike, apple_targets, solarish))]
mod test_sendfile;
#[cfg(any(
    target_os = "freebsd",
    target_os = "haiku",
    target_os = "linux",
    target_os = "netbsd",
    apple_targets
))]
mod test_spawn;

mod test_syslog;

mod test_time;
mod test_unistd;

use nix::unistd::{chdir, getcwd, read};
use parking_lot::{Mutex, RwLock, RwLockWriteGuard};
use std::os::unix::io::AsFd;
use std::path::PathBuf;

/// Helper function analogous to `std::io::Read::read_exact`, but for `Fd`s
fn read_exact<Fd: AsFd>(f: Fd, buf: &mut [u8]) {
    let mut len = 0;
    while len < buf.len() {
        // get_mut would be better than split_at_mut, but it requires nightly
        let (_, remaining) = buf.split_at_mut(len);
        len += read(&f, remaining).unwrap();
    }
}

/// Any test that creates child processes or can be affected by child processes
/// must grab this mutex, regardless of what it does with those children.
///
/// It must hold the mutex until the child processes are waited upon.
pub static FORK_MTX: Mutex<()> = Mutex::new(());
/// Any test that changes the process's current working directory must grab
/// the RwLock exclusively.  Any process that cares about the current
/// working directory must grab it shared.
pub static CWD_LOCK: RwLock<()> = RwLock::new(());
/// Any test that changes the process's supplementary groups must grab this
/// mutex
pub static GROUPS_MTX: Mutex<()> = Mutex::new(());
/// Any tests that loads or unloads kernel modules must grab this mutex
pub static KMOD_MTX: Mutex<()> = Mutex::new(());
/// Any test that calls ptsname(3) must grab this mutex.
pub static PTSNAME_MTX: Mutex<()> = Mutex::new(());
/// Any test that alters signal handling must grab this mutex.
pub static SIGNAL_MTX: Mutex<()> = Mutex::new(());

/// RAII object that restores a test's original directory on drop
struct DirRestore<'a> {
    d: PathBuf,
    _g: RwLockWriteGuard<'a, ()>,
}

impl DirRestore<'_> {
    fn new() -> Self {
        let guard = crate::CWD_LOCK.write();
        DirRestore {
            _g: guard,
            d: getcwd().unwrap(),
        }
    }
}

impl Drop for DirRestore<'_> {
    fn drop(&mut self) {
        let r = chdir(&self.d);
        if std::thread::panicking() {
            r.unwrap();
        }
    }
}
