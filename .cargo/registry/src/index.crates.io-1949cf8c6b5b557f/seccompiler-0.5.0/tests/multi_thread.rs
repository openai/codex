#![allow(clippy::undocumented_unsafe_blocks)]

/// This test is in a separate top-level test file so that it is isolated from the other tests -
/// each file in the tests/ directory gets compiled to a separate binary and is run as a separate
/// process.
use std::collections::BTreeMap;

use std::sync::mpsc::sync_channel;
use std::thread;

use seccompiler::{
    apply_filter_all_threads, BpfProgram, SeccompAction, SeccompFilter, SeccompRule,
};
use std::env::consts::ARCH;

fn check_getpid_fails() {
    let pid = unsafe { libc::getpid() };
    let errno = std::io::Error::last_os_error().raw_os_error().unwrap();

    assert_eq!(pid, -1, "getpid should return -1 as set in SeccompFilter");
    assert_eq!(errno, 0, "there should be no errors");
}

#[test]
/// Test seccomp's TSYNC functionality, which syncs the current filter to all threads in the
/// process.
fn test_tsync() {
    // These channels will block on send until the receiver has called recv.
    let (setup_tx, setup_rx) = sync_channel::<()>(0);
    let (finish_tx, finish_rx) = sync_channel::<()>(0);

    // first check getpid is working
    let pid = unsafe { libc::getpid() };
    let errno = std::io::Error::last_os_error().raw_os_error().unwrap();

    assert!(pid > 0, "getpid should return the actual pid");
    assert_eq!(errno, 0, "there should be no errors");

    // create two threads, one which applies the filter to all threads and another which tries
    // to call getpid.
    let seccomp_thread = thread::spawn(move || {
        let rules = vec![(libc::SYS_getpid, vec![])];

        let rule_map: BTreeMap<i64, Vec<SeccompRule>> = rules.into_iter().collect();

        // Build seccomp filter only disallowing getpid
        let filter = SeccompFilter::new(
            rule_map,
            SeccompAction::Allow,
            SeccompAction::Errno(1u32),
            ARCH.try_into().unwrap(),
        )
        .unwrap();

        let filter: BpfProgram = filter.try_into().unwrap();
        apply_filter_all_threads(&filter).unwrap();

        // Verify seccomp is working in this thread
        check_getpid_fails();

        // seccomp setup done, let the other thread start
        setup_tx.send(()).unwrap();

        // don't close this thread until the other thread is done asserting. This way we can be
        // sure the thread that loaded the filter is definitely active when the other thread runs.
        finish_rx.recv().unwrap();
        println!("exit seccomp thread");
    });

    let test_thread = thread::spawn(move || {
        // wait until seccomp setup is done
        setup_rx.recv().unwrap();

        // Verify seccomp is working in this thread after disallowing it in other thread
        check_getpid_fails();

        // let other thread know we've passed
        finish_tx.send(()).unwrap();
        println!("exit io thread");
    });

    let seccomp_res = seccomp_thread.join();
    assert!(
        seccomp_res.is_ok(),
        "seccomp thread failed: {:?}",
        seccomp_res.unwrap_err()
    );
    let test_res = test_thread.join();
    assert!(
        test_res.is_ok(),
        "test thread failed: {:?}",
        test_res.unwrap_err()
    );

    // Verify seccomp is working in the parent thread as well
    check_getpid_fails();
}
