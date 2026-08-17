use super::Context;
use super::OTelSdkResult;
use super::OtelProvider;
use super::SdkTracerProvider;
use super::Span;
use super::SpanData;
use super::SpanProcessor;
use pretty_assertions::assert_eq;
use std::io::ErrorKind;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::Command;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

#[cfg(any(target_os = "linux", target_os = "macos"))]
const GUARD_PAGE_FAILURE_CHILD_TEST: &str =
    "provider::shutdown_tests::bounded_shutdown_survives_worker_guard_page_failure_child";

#[cfg(target_os = "macos")]
static GUARD_PAGE_INJECTION_ENABLED: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "macos")]
static GUARD_PAGE_INJECTION_ARMED: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "macos")]
static GUARD_PAGE_INJECTION_OBSERVED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
#[unsafe(export_name = "mprotect")]
unsafe extern "C" fn fault_injected_mprotect(
    address: *mut libc::c_void,
    length: usize,
    protection: libc::c_int,
) -> libc::c_int {
    let original_symbol = unsafe { libc::dlsym(libc::RTLD_NEXT, c"mprotect".as_ptr()) };
    let original_mprotect: unsafe extern "C" fn(
        *mut libc::c_void,
        usize,
        libc::c_int,
    ) -> libc::c_int = unsafe { std::mem::transmute(original_symbol) };

    if GUARD_PAGE_INJECTION_ENABLED.load(Ordering::Relaxed)
        && protection == libc::PROT_NONE
        && length == unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize
    {
        let mut thread_name = [0; 64];
        let named_shutdown_worker = unsafe {
            libc::pthread_getname_np(
                libc::pthread_self(),
                thread_name.as_mut_ptr(),
                thread_name.len(),
            )
        } == 0
            && unsafe { std::ffi::CStr::from_ptr(thread_name.as_ptr()) }
                .to_bytes()
                .starts_with(b"codex-otel-shut");

        if named_shutdown_worker {
            GUARD_PAGE_INJECTION_OBSERVED.store(/*val*/ true, Ordering::Relaxed);
            if GUARD_PAGE_INJECTION_ARMED.load(Ordering::Relaxed) {
                unsafe { *libc::__error() = libc::ENOMEM };
                return -1;
            }
        }
    }

    unsafe { original_mprotect(address, length, protection) }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShutdownBehavior {
    Complete,
    WaitForRelease,
}

#[derive(Debug, Default)]
struct ShutdownState {
    force_flushes: AtomicUsize,
    shutdowns: AtomicUsize,
    released: Mutex<bool>,
    release_notification: Condvar,
    started: Mutex<Option<mpsc::Sender<()>>>,
    completed: Mutex<Option<mpsc::Sender<()>>>,
}

#[derive(Debug)]
struct ControlledSpanProcessor {
    behavior: ShutdownBehavior,
    state: Arc<ShutdownState>,
}

impl SpanProcessor for ControlledSpanProcessor {
    fn on_start(&self, _span: &mut Span, _context: &Context) {}

    fn on_end(&self, _span: SpanData) {}

    fn force_flush(&self) -> OTelSdkResult {
        self.state
            .force_flushes
            .fetch_add(/*val*/ 1, Ordering::Relaxed);
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        self.state.shutdowns.fetch_add(/*val*/ 1, Ordering::Relaxed);
        if let Some(started) = self.state.started.lock().expect("started lock").take() {
            let _ = started.send(());
        }

        if self.behavior == ShutdownBehavior::WaitForRelease {
            drop(
                self.state
                    .release_notification
                    .wait_while(
                        self.state.released.lock().expect("release lock"),
                        |released| !*released,
                    )
                    .expect("release notification"),
            );
        }

        if let Some(completed) = self.state.completed.lock().expect("completed lock").take() {
            let _ = completed.send(());
        }
        Ok(())
    }
}

struct TestProvider {
    provider: OtelProvider,
    state: Arc<ShutdownState>,
    started: mpsc::Receiver<()>,
    completed: mpsc::Receiver<()>,
}

fn test_provider(behavior: ShutdownBehavior) -> TestProvider {
    let (started_tx, started) = mpsc::channel();
    let (completed_tx, completed) = mpsc::channel();
    let state = Arc::new(ShutdownState {
        started: Mutex::new(Some(started_tx)),
        completed: Mutex::new(Some(completed_tx)),
        ..ShutdownState::default()
    });
    let processor = ControlledSpanProcessor {
        behavior,
        state: Arc::clone(&state),
    };
    let tracer_provider = SdkTracerProvider::builder()
        .with_span_processor(processor)
        .build();

    TestProvider {
        provider: OtelProvider {
            logger: None,
            tracer_provider: Some(tracer_provider),
            tracer: None,
            metrics: None,
            shutdown_started: AtomicBool::default(),
            shutdown_worker: None,
        },
        state,
        started,
        completed,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn bounded_shutdown_does_not_flush_when_worker_creation_fails() {
    let TestProvider {
        mut provider,
        state,
        started,
        completed,
    } = test_provider(ShutdownBehavior::Complete);

    let preparation = provider.prepare_shutdown_worker_with_spawner(|_startup| {
        Err(std::io::Error::new(
            ErrorKind::WouldBlock,
            "shutdown worker could not be created",
        ))
    });

    assert_eq!(
        preparation.as_ref().map_err(std::io::Error::kind),
        Err(ErrorKind::WouldBlock)
    );

    let result = provider
        .shutdown_with_timeout(Duration::from_secs(/*secs*/ 1))
        .await;
    assert_eq!(
        result.as_ref().map_err(std::io::Error::kind),
        Err(ErrorKind::NotConnected)
    );
    assert_eq!(state.shutdowns.load(Ordering::Relaxed), 0);
    assert_eq!(state.force_flushes.load(Ordering::Relaxed), 0);
    assert_eq!(started.try_recv(), Err(mpsc::TryRecvError::Empty));
    assert_eq!(completed.try_recv(), Err(mpsc::TryRecvError::Empty));
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn bounded_shutdown_survives_worker_guard_page_failure() {
    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time follows Unix epoch")
        .as_nanos();
    let temporary_directory = std::env::temp_dir().join(format!(
        "codex-otel-guard-page-{}-{unique_suffix}",
        std::process::id()
    ));
    std::fs::create_dir(&temporary_directory).expect("create fault injector directory");

    let observed_path = temporary_directory.join("guard_page_fault.observed");
    let mut subprocess = Command::new(std::env::current_exe().expect("current test binary"));
    subprocess
        .arg("--exact")
        .arg(GUARD_PAGE_FAILURE_CHILD_TEST)
        .arg("--ignored")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env("CODEX_OTEL_GUARD_PAGE_FAILURE_CHILD", "1")
        .env("CODEX_OTEL_GUARD_PAGE_FAILURE_OBSERVED", &observed_path);

    let output = subprocess
        .output()
        .expect("run guard-page failure subprocess");
    let injection_was_observed = observed_path.is_file();

    let _ = std::fs::remove_dir_all(&temporary_directory);
    assert!(
        output.status.success(),
        "bounded telemetry shutdown crashed when its worker guard page could not be allocated\n\
         status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        injection_was_observed,
        "guard-page fault injection never became active on {}-{}",
        std::env::consts::ARCH,
        std::env::consts::OS
    );
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
// The parent regression invokes this ignored test in a fresh subprocess with
// `--exact --ignored`, isolating fatal native-thread initialization failures.
#[ignore]
fn bounded_shutdown_survives_worker_guard_page_failure_child() {
    if std::env::var_os("CODEX_OTEL_GUARD_PAGE_FAILURE_CHILD").is_none() {
        return;
    }

    let TestProvider {
        mut provider,
        state,
        ..
    } = test_provider(ShutdownBehavior::Complete);

    #[cfg(target_os = "macos")]
    GUARD_PAGE_INJECTION_ENABLED.store(/*val*/ true, Ordering::Relaxed);

    provider
        .prepare_shutdown_worker()
        .expect("pre-initialize bounded shutdown worker");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("create current-thread runtime");

    #[cfg(target_os = "macos")]
    {
        assert!(
            GUARD_PAGE_INJECTION_OBSERVED.load(Ordering::Relaxed),
            "Rust mprotect interposer did not observe shutdown-worker guard-page setup"
        );
        GUARD_PAGE_INJECTION_ARMED.store(/*val*/ true, Ordering::Relaxed);
    }

    #[cfg(target_os = "linux")]
    {
        use seccompiler::BpfProgram;
        use seccompiler::SeccompAction;
        use seccompiler::SeccompCmpArgLen;
        use seccompiler::SeccompCmpOp;
        use seccompiler::SeccompCondition;
        use seccompiler::SeccompFilter;
        use seccompiler::SeccompRule;

        let page_size =
            usize::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }).expect("valid page size");
        let mapped_page = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                page_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                /*fd*/ -1,
                /*offset*/ 0,
            )
        };
        assert_ne!(
            mapped_page,
            libc::MAP_FAILED,
            "map a page to verify guard-page fault injection: {}",
            std::io::Error::last_os_error()
        );

        let protection_is_none = SeccompCondition::new(
            /*arg_index*/ 2,
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::Eq,
            libc::PROT_NONE as u64,
        )
        .expect("create guard-page seccomp condition");
        let rule =
            SeccompRule::new(vec![protection_is_none]).expect("create guard-page seccomp rule");
        let filter = SeccompFilter::new(
            std::collections::BTreeMap::from([(libc::SYS_mprotect, vec![rule])]),
            SeccompAction::Allow,
            SeccompAction::Errno(libc::ENOMEM as u32),
            std::env::consts::ARCH
                .try_into()
                .expect("supported seccomp architecture"),
        )
        .expect("create guard-page seccomp filter");
        let program: BpfProgram = filter
            .try_into()
            .expect("compile guard-page seccomp filter");
        seccompiler::apply_filter(&program).expect("install guard-page seccomp filter");

        let protection_result = unsafe { libc::mprotect(mapped_page, page_size, libc::PROT_NONE) };
        let protection_error = std::io::Error::last_os_error();
        assert_eq!(protection_result, -1);
        assert_eq!(protection_error.raw_os_error(), Some(libc::ENOMEM));
        assert_eq!(unsafe { libc::munmap(mapped_page, page_size) }, 0);
    }

    let observed_path = std::env::var_os("CODEX_OTEL_GUARD_PAGE_FAILURE_OBSERVED")
        .expect("guard-page fault observation path");
    std::fs::write(observed_path, "observed").expect("record guard-page fault injection");

    runtime
        .block_on(provider.shutdown_with_timeout(Duration::from_secs(/*secs*/ 1)))
        .expect("bounded telemetry shutdown should not create a new native thread");

    assert_eq!(state.shutdowns.load(Ordering::Relaxed), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn bounded_shutdown_times_out_without_blocking_the_runtime() {
    let TestProvider {
        mut provider,
        state,
        started,
        completed,
    } = test_provider(ShutdownBehavior::WaitForRelease);
    provider
        .prepare_shutdown_worker()
        .expect("pre-initialize bounded shutdown worker");

    let result = provider
        .shutdown_with_timeout(Duration::from_millis(/*millis*/ 50))
        .await;

    assert_eq!(
        result.as_ref().map_err(std::io::Error::kind),
        Err(ErrorKind::TimedOut)
    );
    started
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown worker started");
    assert_eq!(state.shutdowns.load(Ordering::Relaxed), 1);
    assert_eq!(state.force_flushes.load(Ordering::Relaxed), 0);

    *state.released.lock().expect("release lock") = true;
    state.release_notification.notify_one();
    completed
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown worker completed after release");
}

async fn assert_bounded_shutdown_completes() {
    let TestProvider {
        mut provider,
        state,
        started,
        completed,
    } = test_provider(ShutdownBehavior::Complete);
    provider
        .prepare_shutdown_worker()
        .expect("pre-initialize bounded shutdown worker");

    provider
        .shutdown_with_timeout(Duration::from_secs(/*secs*/ 1))
        .await
        .expect("healthy processor shuts down");

    started
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown worker started");
    completed
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown worker completed");
    assert_eq!(state.shutdowns.load(Ordering::Relaxed), 1);
    assert_eq!(state.force_flushes.load(Ordering::Relaxed), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn bounded_shutdown_completes_on_current_thread_runtime() {
    assert_bounded_shutdown_completes().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bounded_shutdown_completes_on_multi_thread_runtime() {
    assert_bounded_shutdown_completes().await;
}

#[test]
fn explicit_shutdown_and_drop_shut_down_exporters_once_without_force_flush() {
    let TestProvider {
        provider,
        state,
        started,
        completed,
    } = test_provider(ShutdownBehavior::Complete);

    provider.shutdown();
    provider.shutdown();
    drop(provider);

    started
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown started");
    completed
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown completed");
    assert_eq!(state.shutdowns.load(Ordering::Relaxed), 1);
    assert_eq!(state.force_flushes.load(Ordering::Relaxed), 0);
}

#[test]
fn drop_shuts_down_exporters_without_force_flush() {
    let TestProvider {
        provider,
        state,
        started,
        completed,
    } = test_provider(ShutdownBehavior::Complete);

    drop(provider);

    started
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown started");
    completed
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("shutdown completed");
    assert_eq!(state.shutdowns.load(Ordering::Relaxed), 1);
    assert_eq!(state.force_flushes.load(Ordering::Relaxed), 0);
}
