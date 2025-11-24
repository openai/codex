use nix::errno::Errno;
use nix::sys::signal::*;
use nix::unistd::*;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(target_os = "redox"))]
use std::thread;

#[test]
fn test_kill_none() {
    kill(getpid(), None).expect("Should be able to send signal to myself.");
}

#[test]
#[cfg(not(target_os = "fuchsia"))]
fn test_killpg_none() {
    killpg(getpgrp(), None)
        .expect("Should be able to send signal to my process group.");
}

#[test]
fn test_old_sigaction_flags() {
    let _m = crate::SIGNAL_MTX.lock();

    extern "C" fn handler(_: ::libc::c_int) {}
    let act = SigAction::new(
        SigHandler::Handler(handler),
        SaFlags::empty(),
        SigSet::empty(),
    );
    let oact = unsafe { sigaction(SIGINT, &act) }.unwrap();
    let _flags = oact.flags();
    let oact = unsafe { sigaction(SIGINT, &act) }.unwrap();
    let _flags = oact.flags();
}

#[test]
fn test_sigprocmask_noop() {
    sigprocmask(SigmaskHow::SIG_BLOCK, None, None)
        .expect("this should be an effective noop");
}

#[test]
fn test_sigprocmask() {
    let _m = crate::SIGNAL_MTX.lock();

    // This needs to be a signal that rust doesn't use in the test harness.
    const SIGNAL: Signal = Signal::SIGCHLD;

    let mut old_signal_set = SigSet::empty();
    sigprocmask(SigmaskHow::SIG_BLOCK, None, Some(&mut old_signal_set))
        .expect("expect to be able to retrieve old signals");

    // Make sure the old set doesn't contain the signal, otherwise the following
    // test don't make sense.
    assert!(
        !old_signal_set.contains(SIGNAL),
        "the {SIGNAL:?} signal is already blocked, please change to a \
             different one"
    );

    // Now block the signal.
    let mut signal_set = SigSet::empty();
    signal_set.add(SIGNAL);
    sigprocmask(SigmaskHow::SIG_BLOCK, Some(&signal_set), None)
        .expect("expect to be able to block signals");

    // And test it again, to make sure the change was effective.
    old_signal_set.clear();
    sigprocmask(SigmaskHow::SIG_BLOCK, None, Some(&mut old_signal_set))
        .expect("expect to be able to retrieve old signals");
    assert!(
        old_signal_set.contains(SIGNAL),
        "expected the {SIGNAL:?} to be blocked"
    );

    // Reset the signal.
    sigprocmask(SigmaskHow::SIG_UNBLOCK, Some(&signal_set), None)
        .expect("expect to be able to block signals");
}

static SIGNALED: AtomicBool = AtomicBool::new(false);

extern "C" fn test_sigaction_handler(signal: libc::c_int) {
    let signal = Signal::try_from(signal).unwrap();
    SIGNALED.store(signal == Signal::SIGINT, Ordering::Relaxed);
}

#[cfg(not(target_os = "redox"))]
extern "C" fn test_sigaction_action(
    _: libc::c_int,
    _: *mut libc::siginfo_t,
    _: *mut libc::c_void,
) {
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_signal_sigaction() {
    let _m = crate::SIGNAL_MTX.lock();

    let action_handler = SigHandler::SigAction(test_sigaction_action);
    assert_eq!(
        unsafe { signal(Signal::SIGINT, action_handler) }.unwrap_err(),
        Errno::ENOTSUP
    );
}

#[test]
fn test_signal() {
    let _m = crate::SIGNAL_MTX.lock();

    unsafe { signal(Signal::SIGINT, SigHandler::SigIgn) }.unwrap();
    raise(Signal::SIGINT).unwrap();
    assert_eq!(
        unsafe { signal(Signal::SIGINT, SigHandler::SigDfl) }.unwrap(),
        SigHandler::SigIgn
    );

    let handler = SigHandler::Handler(test_sigaction_handler);
    assert_eq!(
        unsafe { signal(Signal::SIGINT, handler) }.unwrap(),
        SigHandler::SigDfl
    );
    raise(Signal::SIGINT).unwrap();
    assert!(SIGNALED.load(Ordering::Relaxed));

    #[cfg(not(solarish))]
    assert_eq!(
        unsafe { signal(Signal::SIGINT, SigHandler::SigDfl) }.unwrap(),
        handler
    );

    // System V based OSes (e.g. illumos and Solaris) always resets the
    // disposition to SIG_DFL prior to calling the signal handler
    #[cfg(solarish)]
    assert_eq!(
        unsafe { signal(Signal::SIGINT, SigHandler::SigDfl) }.unwrap(),
        SigHandler::SigDfl
    );

    // Restore default signal handler
    unsafe { signal(Signal::SIGINT, SigHandler::SigDfl) }.unwrap();
}

#[test]
fn test_contains() {
    let mut mask = SigSet::empty();
    mask.add(SIGUSR1);

    assert!(mask.contains(SIGUSR1));
    assert!(!mask.contains(SIGUSR2));

    let all = SigSet::all();
    assert!(all.contains(SIGUSR1));
    assert!(all.contains(SIGUSR2));
}

#[test]
fn test_clear() {
    let mut set = SigSet::all();
    set.clear();
    for signal in Signal::iterator() {
        assert!(!set.contains(signal));
    }
}

#[test]
fn test_from_str_round_trips() {
    for signal in Signal::iterator() {
        assert_eq!(signal.as_ref().parse::<Signal>().unwrap(), signal);
        assert_eq!(signal.to_string().parse::<Signal>().unwrap(), signal);
    }
}

#[test]
fn test_from_str_invalid_value() {
    let errval = Err(Errno::EINVAL);
    assert_eq!("NOSIGNAL".parse::<Signal>(), errval);
    assert_eq!("kill".parse::<Signal>(), errval);
    assert_eq!("9".parse::<Signal>(), errval);
}

#[test]
fn test_extend() {
    let mut one_signal = SigSet::empty();
    one_signal.add(SIGUSR1);

    let mut two_signals = SigSet::empty();
    two_signals.add(SIGUSR2);
    two_signals.extend(&one_signal);

    assert!(two_signals.contains(SIGUSR1));
    assert!(two_signals.contains(SIGUSR2));
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_thread_signal_set_mask() {
    thread::spawn(|| {
        let prev_mask = SigSet::thread_get_mask()
            .expect("Failed to get existing signal mask!");

        let mut test_mask = prev_mask;
        test_mask.add(SIGUSR1);

        test_mask.thread_set_mask().expect("assertion failed");
        let new_mask =
            SigSet::thread_get_mask().expect("Failed to get new mask!");

        assert!(new_mask.contains(SIGUSR1));
        assert!(!new_mask.contains(SIGUSR2));

        prev_mask
            .thread_set_mask()
            .expect("Failed to revert signal mask!");
    })
    .join()
    .unwrap();
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_thread_signal_block() {
    thread::spawn(|| {
        let mut mask = SigSet::empty();
        mask.add(SIGUSR1);

        mask.thread_block().expect("assertion failed");

        assert!(SigSet::thread_get_mask().unwrap().contains(SIGUSR1));
    })
    .join()
    .unwrap();
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_thread_signal_unblock() {
    thread::spawn(|| {
        let mut mask = SigSet::empty();
        mask.add(SIGUSR1);

        mask.thread_unblock().expect("assertion failed");

        assert!(!SigSet::thread_get_mask().unwrap().contains(SIGUSR1));
    })
    .join()
    .unwrap();
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_thread_signal_swap() {
    thread::spawn(|| {
        let mut mask = SigSet::empty();
        mask.add(SIGUSR1);
        mask.thread_block().unwrap();

        assert!(SigSet::thread_get_mask().unwrap().contains(SIGUSR1));

        let mut mask2 = SigSet::empty();
        mask2.add(SIGUSR2);

        let oldmask = mask2.thread_swap_mask(SigmaskHow::SIG_SETMASK).unwrap();

        assert!(oldmask.contains(SIGUSR1));
        assert!(!oldmask.contains(SIGUSR2));

        assert!(SigSet::thread_get_mask().unwrap().contains(SIGUSR2));
    })
    .join()
    .unwrap();
}

#[test]
fn test_from_and_into_iterator() {
    let sigset = SigSet::from_iter(vec![Signal::SIGUSR1, Signal::SIGUSR2]);
    let signals = sigset.into_iter().collect::<Vec<Signal>>();
    assert_eq!(signals, [Signal::SIGUSR1, Signal::SIGUSR2]);
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_sigaction() {
    let _m = crate::SIGNAL_MTX.lock();
    thread::spawn(|| {
        extern "C" fn test_sigaction_handler(_: libc::c_int) {}
        extern "C" fn test_sigaction_action(
            _: libc::c_int,
            _: *mut libc::siginfo_t,
            _: *mut libc::c_void,
        ) {
        }

        let handler_sig = SigHandler::Handler(test_sigaction_handler);

        let flags =
            SaFlags::SA_ONSTACK | SaFlags::SA_RESTART | SaFlags::SA_SIGINFO;

        let mut mask = SigSet::empty();
        mask.add(SIGUSR1);

        let action_sig = SigAction::new(handler_sig, flags, mask);

        assert_eq!(
            action_sig.flags(),
            SaFlags::SA_ONSTACK | SaFlags::SA_RESTART
        );
        assert_eq!(action_sig.handler(), handler_sig);

        mask = action_sig.mask();
        assert!(mask.contains(SIGUSR1));
        assert!(!mask.contains(SIGUSR2));

        let handler_act = SigHandler::SigAction(test_sigaction_action);
        let action_act = SigAction::new(handler_act, flags, mask);
        assert_eq!(action_act.handler(), handler_act);

        let action_dfl = SigAction::new(SigHandler::SigDfl, flags, mask);
        assert_eq!(action_dfl.handler(), SigHandler::SigDfl);

        let action_ign = SigAction::new(SigHandler::SigIgn, flags, mask);
        assert_eq!(action_ign.handler(), SigHandler::SigIgn);
    })
    .join()
    .unwrap();
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_sigwait() {
    thread::spawn(|| {
        let mut mask = SigSet::empty();
        mask.add(SIGUSR1);
        mask.add(SIGUSR2);
        mask.thread_block().unwrap();

        raise(SIGUSR1).unwrap();
        assert_eq!(mask.wait().unwrap(), SIGUSR1);
    })
    .join()
    .unwrap();
}

#[cfg(any(
    bsd,
    linux_android,
    solarish,
    target_os = "haiku",
    target_os = "hurd",
    target_os = "aix",
    target_os = "fuchsia"
))]
#[test]
fn test_sigsuspend() {
    // This test change signal handler
    let _m = crate::SIGNAL_MTX.lock();
    static SIGNAL_RECIEVED: AtomicBool = AtomicBool::new(false);
    extern "C" fn test_sigsuspend_handler(_: libc::c_int) {
        assert!(!SIGNAL_RECIEVED.swap(true, Ordering::SeqCst));
    }
    thread::spawn(|| {
        const SIGNAL: Signal = Signal::SIGUSR1;

        // Add signal mask to this thread
        let mut signal_set = SigSet::empty();
        signal_set.add(SIGNAL);
        signal_set.thread_block().unwrap();

        // Set signal handler and save old one.
        let act = SigAction::new(
            SigHandler::Handler(test_sigsuspend_handler),
            SaFlags::empty(),
            SigSet::empty(),
        );
        let old_act = unsafe { sigaction(SIGNAL, &act) }
            .expect("expect to be able to set new action and get old action");

        raise(SIGNAL).expect("expect be able to send signal");
        // Now `SIGNAL` was sended but it is blocked.
        let mut not_wait_set = SigSet::all();
        not_wait_set.remove(SIGNAL);
        // signal handler must run in SigSet::suspend()
        assert!(!SIGNAL_RECIEVED.load(Ordering::SeqCst));
        not_wait_set.suspend().unwrap();
        assert!(SIGNAL_RECIEVED.load(Ordering::SeqCst));

        // Restore the signal handler.
        unsafe { sigaction(SIGNAL, &old_act) }
            .expect("expect to be able to restore old action ");
    })
    .join()
    .unwrap();
}

#[test]
fn test_from_sigset_t_unchecked() {
    let src_set = SigSet::empty();
    let set = unsafe { SigSet::from_sigset_t_unchecked(*src_set.as_ref()) };

    for signal in Signal::iterator() {
        assert!(!set.contains(signal));
    }

    let src_set = SigSet::all();
    let set = unsafe { SigSet::from_sigset_t_unchecked(*src_set.as_ref()) };

    for signal in Signal::iterator() {
        assert!(set.contains(signal));
    }
}

#[test]
fn test_eq_empty() {
    let set0 = SigSet::empty();
    let set1 = SigSet::empty();
    assert_eq!(set0, set1);
}

#[test]
fn test_eq_all() {
    let set0 = SigSet::all();
    let set1 = SigSet::all();
    assert_eq!(set0, set1);
}

#[test]
fn test_hash_empty() {
    use std::collections::hash_map::DefaultHasher;

    let set0 = SigSet::empty();
    let mut h0 = DefaultHasher::new();
    set0.hash(&mut h0);

    let set1 = SigSet::empty();
    let mut h1 = DefaultHasher::new();
    set1.hash(&mut h1);

    assert_eq!(h0.finish(), h1.finish());
}

#[test]
fn test_hash_all() {
    use std::collections::hash_map::DefaultHasher;

    let set0 = SigSet::all();
    let mut h0 = DefaultHasher::new();
    set0.hash(&mut h0);

    let set1 = SigSet::all();
    let mut h1 = DefaultHasher::new();
    set1.hash(&mut h1);

    assert_eq!(h0.finish(), h1.finish());
}
