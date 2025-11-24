#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "x86"),
    target_env = "gnu"
))]
use memoffset::offset_of;
use nix::errno::Errno;
use nix::sys::ptrace;
#[cfg(linux_android)]
use nix::sys::ptrace::Options;
use nix::unistd::getpid;

#[cfg(linux_android)]
use std::mem;

use crate::*;

#[test]
fn test_ptrace() {
    // Just make sure ptrace can be called at all, for now.
    // FIXME: qemu-user doesn't implement ptrace on all arches, so permit ENOSYS
    require_capability!("test_ptrace", CAP_SYS_PTRACE);
    let err = ptrace::attach(getpid()).unwrap_err();
    assert!(
        err == Errno::EPERM || err == Errno::EINVAL || err == Errno::ENOSYS
    );
}

// Just make sure ptrace_setoptions can be called at all, for now.
#[test]
#[cfg(linux_android)]
fn test_ptrace_setoptions() {
    require_capability!("test_ptrace_setoptions", CAP_SYS_PTRACE);
    let err = ptrace::setoptions(getpid(), Options::PTRACE_O_TRACESYSGOOD)
        .unwrap_err();
    assert_ne!(err, Errno::EOPNOTSUPP);
}

// Just make sure ptrace_getevent can be called at all, for now.
#[test]
#[cfg(linux_android)]
fn test_ptrace_getevent() {
    require_capability!("test_ptrace_getevent", CAP_SYS_PTRACE);
    let err = ptrace::getevent(getpid()).unwrap_err();
    assert_ne!(err, Errno::EOPNOTSUPP);
}

// Just make sure ptrace_getsiginfo can be called at all, for now.
#[test]
#[cfg(linux_android)]
fn test_ptrace_getsiginfo() {
    require_capability!("test_ptrace_getsiginfo", CAP_SYS_PTRACE);
    if let Err(Errno::EOPNOTSUPP) = ptrace::getsiginfo(getpid()) {
        panic!("ptrace_getsiginfo returns Errno::EOPNOTSUPP!");
    }
}

// Just make sure ptrace_setsiginfo can be called at all, for now.
#[test]
#[cfg(linux_android)]
fn test_ptrace_setsiginfo() {
    require_capability!("test_ptrace_setsiginfo", CAP_SYS_PTRACE);
    let siginfo = unsafe { mem::zeroed() };
    if let Err(Errno::EOPNOTSUPP) = ptrace::setsiginfo(getpid(), &siginfo) {
        panic!("ptrace_setsiginfo returns Errno::EOPNOTSUPP!");
    }
}

#[test]
fn test_ptrace_cont() {
    use nix::sys::ptrace;
    use nix::sys::signal::{raise, Signal};
    use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
    use nix::unistd::fork;
    use nix::unistd::ForkResult::*;

    require_capability!("test_ptrace_cont", CAP_SYS_PTRACE);

    let _m = crate::FORK_MTX.lock();

    // FIXME: qemu-user doesn't implement ptrace on all architectures
    // and returns ENOSYS in this case.
    // We (ab)use this behavior to detect the affected platforms
    // and skip the test then.
    // On valid platforms the ptrace call should return Errno::EPERM, this
    // is already tested by `test_ptrace`.
    let err = ptrace::attach(getpid()).unwrap_err();
    if err == Errno::ENOSYS {
        return;
    }

    match unsafe { fork() }.expect("Error: Fork Failed") {
        Child => {
            ptrace::traceme().unwrap();
            // As recommended by ptrace(2), raise SIGTRAP to pause the child
            // until the parent is ready to continue
            loop {
                raise(Signal::SIGTRAP).unwrap();
            }
        }
        Parent { child } => {
            assert_eq!(
                waitpid(child, None),
                Ok(WaitStatus::Stopped(child, Signal::SIGTRAP))
            );
            ptrace::cont(child, None).unwrap();
            assert_eq!(
                waitpid(child, None),
                Ok(WaitStatus::Stopped(child, Signal::SIGTRAP))
            );
            ptrace::cont(child, Some(Signal::SIGKILL)).unwrap();
            match waitpid(child, None) {
                Ok(WaitStatus::Signaled(pid, Signal::SIGKILL, _))
                    if pid == child =>
                {
                    // FIXME It's been observed on some systems (apple) the
                    // tracee may not be killed but remain as a zombie process
                    // affecting other wait based tests. Add an extra kill just
                    // to make sure there are no zombies.
                    let _ = waitpid(child, Some(WaitPidFlag::WNOHANG));
                    while ptrace::cont(child, Some(Signal::SIGKILL)).is_ok() {
                        let _ = waitpid(child, Some(WaitPidFlag::WNOHANG));
                    }
                }
                _ => panic!("The process should have been killed"),
            }
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn test_ptrace_interrupt() {
    use nix::sys::ptrace;
    use nix::sys::signal::Signal;
    use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
    use nix::unistd::fork;
    use nix::unistd::ForkResult::*;
    use std::thread::sleep;
    use std::time::Duration;

    require_capability!("test_ptrace_interrupt", CAP_SYS_PTRACE);

    let _m = crate::FORK_MTX.lock();

    match unsafe { fork() }.expect("Error: Fork Failed") {
        Child => loop {
            sleep(Duration::from_millis(1000));
        },
        Parent { child } => {
            ptrace::seize(child, ptrace::Options::PTRACE_O_TRACESYSGOOD)
                .unwrap();
            ptrace::interrupt(child).unwrap();
            assert_eq!(
                waitpid(child, None),
                Ok(WaitStatus::PtraceEvent(child, Signal::SIGTRAP, 128))
            );
            ptrace::syscall(child, None).unwrap();
            assert_eq!(
                waitpid(child, None),
                Ok(WaitStatus::PtraceSyscall(child))
            );
            ptrace::detach(child, Some(Signal::SIGKILL)).unwrap();
            match waitpid(child, None) {
                Ok(WaitStatus::Signaled(pid, Signal::SIGKILL, _))
                    if pid == child =>
                {
                    let _ = waitpid(child, Some(WaitPidFlag::WNOHANG));
                    while ptrace::cont(child, Some(Signal::SIGKILL)).is_ok() {
                        let _ = waitpid(child, Some(WaitPidFlag::WNOHANG));
                    }
                }
                _ => panic!("The process should have been killed"),
            }
        }
    }
}

// ptrace::{setoptions, getregs} are only available in these platforms
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "x86"),
    target_env = "gnu"
))]
#[test]
fn test_ptrace_syscall() {
    use nix::sys::ptrace;
    use nix::sys::signal::kill;
    use nix::sys::signal::Signal;
    use nix::sys::wait::{waitpid, WaitStatus};
    use nix::unistd::fork;
    use nix::unistd::getpid;
    use nix::unistd::ForkResult::*;

    require_capability!("test_ptrace_syscall", CAP_SYS_PTRACE);

    let _m = crate::FORK_MTX.lock();

    match unsafe { fork() }.expect("Error: Fork Failed") {
        Child => {
            ptrace::traceme().unwrap();
            // first sigstop until parent is ready to continue
            let pid = getpid();
            kill(pid, Signal::SIGSTOP).unwrap();
            kill(pid, Signal::SIGTERM).unwrap();
            unsafe {
                ::libc::_exit(0);
            }
        }

        Parent { child } => {
            assert_eq!(
                waitpid(child, None),
                Ok(WaitStatus::Stopped(child, Signal::SIGSTOP))
            );

            // set this option to recognize syscall-stops
            ptrace::setoptions(child, ptrace::Options::PTRACE_O_TRACESYSGOOD)
                .unwrap();

            #[cfg(target_arch = "x86_64")]
            let get_syscall_id =
                || ptrace::getregs(child).unwrap().orig_rax as libc::c_long;

            #[cfg(target_arch = "x86")]
            let get_syscall_id =
                || ptrace::getregs(child).unwrap().orig_eax as libc::c_long;

            // this duplicates `get_syscall_id` for the purpose of testing `ptrace::read_user`.
            #[cfg(target_arch = "x86_64")]
            let rax_offset = offset_of!(libc::user_regs_struct, orig_rax);
            #[cfg(target_arch = "x86")]
            let rax_offset = offset_of!(libc::user_regs_struct, orig_eax);

            let get_syscall_from_user_area = || {
                // Find the offset of `user.regs.rax` (or `user.regs.eax` for x86)
                let rax_offset = offset_of!(libc::user, regs) + rax_offset;
                ptrace::read_user(child, rax_offset as _).unwrap()
                    as libc::c_long
            };

            // kill entry
            ptrace::syscall(child, None).unwrap();
            assert_eq!(
                waitpid(child, None),
                Ok(WaitStatus::PtraceSyscall(child))
            );
            assert_eq!(get_syscall_id(), ::libc::SYS_kill);
            assert_eq!(get_syscall_from_user_area(), ::libc::SYS_kill);

            // kill exit
            ptrace::syscall(child, None).unwrap();
            assert_eq!(
                waitpid(child, None),
                Ok(WaitStatus::PtraceSyscall(child))
            );
            assert_eq!(get_syscall_id(), ::libc::SYS_kill);
            assert_eq!(get_syscall_from_user_area(), ::libc::SYS_kill);

            // receive signal
            ptrace::syscall(child, None).unwrap();
            assert_eq!(
                waitpid(child, None),
                Ok(WaitStatus::Stopped(child, Signal::SIGTERM))
            );

            // inject signal
            ptrace::syscall(child, Signal::SIGTERM).unwrap();
            assert_eq!(
                waitpid(child, None),
                Ok(WaitStatus::Signaled(child, Signal::SIGTERM, false))
            );
        }
    }
}
