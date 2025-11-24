use libc::_exit;
use nix::errno::Errno;
use nix::sys::signal::*;
use nix::sys::wait::*;
use nix::unistd::ForkResult::*;
use nix::unistd::*;

#[test]
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
fn test_wait_signal() {
    let _m = crate::FORK_MTX.lock();

    // Safe: The child only calls `pause` and/or `_exit`, which are async-signal-safe.
    match unsafe { fork() }.expect("Error: Fork Failed") {
        Child => {
            pause();
            unsafe { _exit(123) }
        }
        Parent { child } => {
            kill(child, Some(SIGKILL)).expect("Error: Kill Failed");
            assert_eq!(
                waitpid(child, None),
                Ok(WaitStatus::Signaled(child, SIGKILL, false))
            );
        }
    }
}

#[test]
#[cfg(any(
    target_os = "android",
    target_os = "freebsd",
    //target_os = "haiku",
    all(target_os = "linux", not(target_env = "uclibc")),
))]
#[cfg(not(any(
    target_arch = "mips",
    target_arch = "mips32r6",
    target_arch = "mips64",
    target_arch = "mips64r6"
)))]
fn test_waitid_signal() {
    let _m = crate::FORK_MTX.lock();

    // Safe: The child only calls `pause` and/or `_exit`, which are async-signal-safe.
    match unsafe { fork() }.expect("Error: Fork Failed") {
        Child => {
            pause();
            unsafe { _exit(123) }
        }
        Parent { child } => {
            kill(child, Some(SIGKILL)).expect("Error: Kill Failed");
            assert_eq!(
                waitid(Id::Pid(child), WaitPidFlag::WEXITED),
                Ok(WaitStatus::Signaled(child, SIGKILL, false)),
            );
        }
    }
}

#[test]
fn test_wait_exit() {
    let _m = crate::FORK_MTX.lock();

    // Safe: Child only calls `_exit`, which is async-signal-safe.
    match unsafe { fork() }.expect("Error: Fork Failed") {
        Child => unsafe {
            _exit(12);
        },
        Parent { child } => {
            assert_eq!(waitpid(child, None), Ok(WaitStatus::Exited(child, 12)));
        }
    }
}

#[cfg(not(target_os = "haiku"))]
#[test]
#[cfg(any(
    target_os = "android",
    target_os = "freebsd",
    target_os = "haiku",
    all(target_os = "linux", not(target_env = "uclibc")),
))]
#[cfg(not(any(
    target_arch = "mips",
    target_arch = "mips32r6",
    target_arch = "mips64",
    target_arch = "mips64r6"
)))]
fn test_waitid_exit() {
    let _m = crate::FORK_MTX.lock();

    // Safe: Child only calls `_exit`, which is async-signal-safe.
    match unsafe { fork() }.expect("Error: Fork Failed") {
        Child => unsafe {
            _exit(12);
        },
        Parent { child } => {
            assert_eq!(
                waitid(Id::Pid(child), WaitPidFlag::WEXITED),
                Ok(WaitStatus::Exited(child, 12)),
            );
        }
    }
}

#[test]
fn test_waitstatus_from_raw() {
    let pid = Pid::from_raw(1);
    assert_eq!(
        WaitStatus::from_raw(pid, 0x0002),
        Ok(WaitStatus::Signaled(pid, Signal::SIGINT, false))
    );
    assert_eq!(
        WaitStatus::from_raw(pid, 0x0200),
        Ok(WaitStatus::Exited(pid, 2))
    );
    assert_eq!(WaitStatus::from_raw(pid, 0x7f7f), Err(Errno::EINVAL));
}

#[test]
fn test_waitstatus_pid() {
    let _m = crate::FORK_MTX.lock();

    match unsafe { fork() }.unwrap() {
        Child => unsafe { _exit(0) },
        Parent { child } => {
            let status = waitpid(child, None).unwrap();
            assert_eq!(status.pid(), Some(child));
        }
    }
}

#[test]
#[cfg(any(
    target_os = "android",
    target_os = "freebsd",
    target_os = "haiku",
    all(target_os = "linux", not(target_env = "uclibc")),
))]
fn test_waitid_pid() {
    let _m = crate::FORK_MTX.lock();

    match unsafe { fork() }.unwrap() {
        Child => unsafe { _exit(0) },
        Parent { child } => {
            let status = waitid(Id::Pid(child), WaitPidFlag::WEXITED).unwrap();
            assert_eq!(status.pid(), Some(child));
        }
    }
}

#[cfg(linux_android)]
// FIXME: qemu-user doesn't implement ptrace on most arches
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod ptrace {
    use crate::*;
    use libc::_exit;
    use nix::sys::ptrace::{self, Event, Options};
    use nix::sys::signal::*;
    use nix::sys::wait::*;
    use nix::unistd::ForkResult::*;
    use nix::unistd::*;

    fn ptrace_child() -> ! {
        ptrace::traceme().unwrap();
        // As recommended by ptrace(2), raise SIGTRAP to pause the child
        // until the parent is ready to continue
        raise(SIGTRAP).unwrap();
        unsafe { _exit(0) }
    }

    fn ptrace_wait_parent(child: Pid) {
        // Wait for the raised SIGTRAP
        assert_eq!(
            waitpid(child, None),
            Ok(WaitStatus::Stopped(child, SIGTRAP))
        );
        // We want to test a syscall stop and a PTRACE_EVENT stop
        ptrace::setoptions(
            child,
            Options::PTRACE_O_TRACESYSGOOD | Options::PTRACE_O_TRACEEXIT,
        )
        .expect("setoptions failed");

        // First, stop on the next system call, which will be exit()
        ptrace::syscall(child, None).expect("syscall failed");
        assert_eq!(waitpid(child, None), Ok(WaitStatus::PtraceSyscall(child)));
        // Then get the ptrace event for the process exiting
        ptrace::cont(child, None).expect("cont failed");
        assert_eq!(
            waitpid(child, None),
            Ok(WaitStatus::PtraceEvent(
                child,
                SIGTRAP,
                Event::PTRACE_EVENT_EXIT as i32
            ))
        );
        // Finally get the normal wait() result, now that the process has exited
        ptrace::cont(child, None).expect("cont failed");
        assert_eq!(waitpid(child, None), Ok(WaitStatus::Exited(child, 0)));
    }

    #[cfg(not(target_env = "uclibc"))]
    fn ptrace_waitid_parent(child: Pid) {
        // Wait for the raised SIGTRAP
        //
        // Unlike waitpid(), waitid() can distinguish trap events from regular
        // stop events, so unlike ptrace_wait_parent(), we get a PtraceEvent here
        assert_eq!(
            waitid(Id::Pid(child), WaitPidFlag::WEXITED),
            Ok(WaitStatus::PtraceEvent(child, SIGTRAP, 0)),
        );
        // We want to test a syscall stop and a PTRACE_EVENT stop
        ptrace::setoptions(
            child,
            Options::PTRACE_O_TRACESYSGOOD | Options::PTRACE_O_TRACEEXIT,
        )
        .expect("setopts failed");

        // First, stop on the next system call, which will be exit()
        ptrace::syscall(child, None).expect("syscall failed");
        assert_eq!(
            waitid(Id::Pid(child), WaitPidFlag::WEXITED),
            Ok(WaitStatus::PtraceSyscall(child)),
        );
        // Then get the ptrace event for the process exiting
        ptrace::cont(child, None).expect("cont failed");
        assert_eq!(
            waitid(Id::Pid(child), WaitPidFlag::WEXITED),
            Ok(WaitStatus::PtraceEvent(
                child,
                SIGTRAP,
                Event::PTRACE_EVENT_EXIT as i32
            )),
        );
        // Finally get the normal wait() result, now that the process has exited
        ptrace::cont(child, None).expect("cont failed");
        assert_eq!(
            waitid(Id::Pid(child), WaitPidFlag::WEXITED),
            Ok(WaitStatus::Exited(child, 0)),
        );
    }

    #[test]
    fn test_wait_ptrace() {
        require_capability!("test_wait_ptrace", CAP_SYS_PTRACE);
        let _m = crate::FORK_MTX.lock();

        match unsafe { fork() }.expect("Error: Fork Failed") {
            Child => ptrace_child(),
            Parent { child } => ptrace_wait_parent(child),
        }
    }

    #[test]
    #[cfg(not(target_env = "uclibc"))]
    fn test_waitid_ptrace() {
        require_capability!("test_waitid_ptrace", CAP_SYS_PTRACE);
        let _m = crate::FORK_MTX.lock();

        match unsafe { fork() }.expect("Error: Fork Failed") {
            Child => ptrace_child(),
            Parent { child } => ptrace_waitid_parent(child),
        }
    }
}
