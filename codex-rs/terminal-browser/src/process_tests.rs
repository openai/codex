use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_utils_pty::ProcessDriver;
use codex_utils_pty::spawn_from_driver;
use pretty_assertions::assert_eq;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use super::StartupGuard;
use crate::devtools::prepare_devtools_active_port;

#[tokio::test]
async fn canceled_startup_terminates_the_process_and_aborts_output() {
    let (writer_tx, _writer_rx) = mpsc::channel(/*buffer*/ 1);
    let (_output_tx, output_rx) = broadcast::channel(/*capacity*/ 1);
    let (_exit_tx, exit_rx) = oneshot::channel();
    let terminated = Arc::new(AtomicBool::new(/*v*/ false));
    let terminate_flag = terminated.clone();
    let spawned = spawn_from_driver(ProcessDriver {
        writer_tx,
        stdout_rx: output_rx,
        stderr_rx: None,
        exit_rx,
        terminator: Some(Box::new(move || {
            terminate_flag.store(/*val*/ true, Ordering::SeqCst);
        })),
        writer_handle: None,
        resizer: None,
    });
    let process = Arc::new(spawned.session);
    let process_slot = Mutex::new(Some(process.clone()));
    let output_task = tokio::spawn(std::future::pending::<()>());
    let output_abort = output_task.abort_handle();
    let mut startup = StartupGuard::new(&process_slot, process, /*profile_lock*/ None);
    startup.set_output_task(output_task);

    drop(startup);
    tokio::task::yield_now().await;

    assert!(terminated.load(Ordering::SeqCst));
    assert!(output_abort.is_finished());
    assert!(process_slot.lock().expect("process slot").is_none());
}

#[test]
fn stale_devtools_active_port_is_removed_before_spawn() {
    let profile = tempfile::tempdir().expect("profile directory");
    let active_port = profile.path().join("DevToolsActivePort");
    std::fs::write(&active_port, "9222\n").expect("stale active port");

    prepare_devtools_active_port(profile.path()).expect("prepare active port");

    assert!(!active_port.exists());
    prepare_devtools_active_port(profile.path()).expect("missing active port is valid");
}

#[test]
fn non_file_devtools_active_port_is_rejected() {
    let profile = tempfile::tempdir().expect("profile directory");
    let active_port = profile.path().join("DevToolsActivePort");
    std::fs::create_dir(&active_port).expect("active port directory");

    let error = prepare_devtools_active_port(profile.path())
        .expect_err("active port directory must be rejected");

    assert_eq!(
        error.to_string(),
        "refusing non-file Carbonyl DevToolsActivePort"
    );
}

#[cfg(unix)]
#[test]
fn symbolic_link_devtools_active_port_is_rejected() {
    use std::os::unix::fs::symlink;

    let profile = tempfile::tempdir().expect("profile directory");
    let target = profile.path().join("target");
    let active_port = profile.path().join("DevToolsActivePort");
    std::fs::write(&target, "9222\n").expect("target active port");
    symlink(&target, &active_port).expect("active port symlink");

    let error = prepare_devtools_active_port(profile.path())
        .expect_err("active port symlink must be rejected");

    assert_eq!(
        error.to_string(),
        "refusing symbolic-link Carbonyl DevToolsActivePort"
    );
    assert_eq!(
        std::fs::read_to_string(target).expect("target remains"),
        "9222\n"
    );
}
