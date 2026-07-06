use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_utils_pty::ProcessDriver;
use codex_utils_pty::spawn_from_driver;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use super::StartupGuard;
use super::carbonyl_args;
use crate::network::BrowserNetworkPolicy;
use crate::session::RenderMode;

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
fn carbonyl_uses_only_the_inherited_devtools_pipe() {
    let args = carbonyl_args(
        "/tmp/profile",
        &BrowserNetworkPolicy::Direct,
        RenderMode::NativeText,
    );

    assert!(args.contains(&"--remote-debugging-pipe".to_string()));
    assert!(
        !args
            .iter()
            .any(|arg| arg.starts_with("--remote-debugging-address="))
    );
    assert!(
        !args
            .iter()
            .any(|arg| arg.starts_with("--remote-debugging-port="))
    );
}
