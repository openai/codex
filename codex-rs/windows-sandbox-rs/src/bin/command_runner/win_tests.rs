use super::RunnerJobController;
use super::spawn_input_loop;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

struct RecordingJobController {
    terminated: Arc<AtomicBool>,
}

impl RunnerJobController for RecordingJobController {
    fn terminate_job(&self) {
        self.terminated.store(true, Ordering::SeqCst);
    }
}

#[test]
fn control_pipe_eof_terminates_child_job() {
    let terminated = Arc::new(AtomicBool::new(false));
    let input_thread = spawn_input_loop(
        tempfile::tempfile().expect("create empty control-pipe stand-in"),
        /*stdin_handle*/ None,
        Arc::new(Mutex::new(None)),
        RecordingJobController {
            terminated: Arc::clone(&terminated),
        },
        /*log_dir*/ None,
    );

    input_thread.join().expect("join runner input loop");
    assert!(
        terminated.load(Ordering::SeqCst),
        "EOF from the parent transport must terminate the child job"
    );
}
