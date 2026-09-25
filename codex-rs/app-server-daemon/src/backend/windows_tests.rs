use super::Process;
use pretty_assertions::assert_eq;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::os::windows::process::CommandExt;
use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows_sys::Win32::System::JobObjects::IsProcessInJob;
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
use windows_sys::Win32::System::Threading::TerminateProcess;

#[test]
fn detached_launch_preflight_rejects_restrictive_job() {
    const CHILD: &str = "CODEX_TEST_RESTRICTIVE_LAUNCH_JOB";
    let executable = std::env::current_exe().expect("test executable");
    if std::env::var_os(CHILD).is_some() {
        let job = unsafe { super::CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        assert_ne!(job, 0);
        let job = unsafe {
            <std::os::windows::io::OwnedHandle as std::os::windows::io::FromRawHandle>::from_raw_handle(job as _)
        };
        assert_ne!(
            unsafe {
                super::AssignProcessToJobObject(
                    job.as_raw_handle() as _,
                    super::GetCurrentProcess(),
                )
            },
            0
        );
        // A new job does not permit breakaway. Reject before any lifecycle mutation.
        assert!(super::ensure_detached_launch(&executable).is_err());
        return;
    }
    let output = std::process::Command::new(executable)
        .args([
            "--exact",
            "backend::windows::tests::detached_launch_preflight_rejects_restrictive_job",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .expect("isolated job test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}

#[test]
fn detached_launch_preflight_allows_residual_job() {
    const CHILD: &str = "CODEX_TEST_RESIDUAL_LAUNCH_JOB";
    let executable = std::env::current_exe().expect("test executable");
    if std::env::var_os(CHILD).is_some() {
        let mut jobs = Vec::new();
        for inner in [false, true] {
            let job = unsafe { super::CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            assert_ne!(job, 0);
            let job = unsafe { OwnedHandle::from_raw_handle(job as _) };
            if inner {
                let mut limits: super::JOBOBJECT_EXTENDED_LIMIT_INFORMATION =
                    unsafe { std::mem::zeroed() };
                limits.BasicLimitInformation.LimitFlags = super::JOB_OBJECT_LIMIT_BREAKAWAY_OK;
                assert_ne!(
                    unsafe {
                        super::SetInformationJobObject(
                            job.as_raw_handle() as _,
                            super::JobObjectExtendedLimitInformation,
                            (&limits as *const super::JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                            std::mem::size_of_val(&limits) as u32,
                        )
                    },
                    0
                );
            }
            assert_ne!(
                unsafe {
                    super::AssignProcessToJobObject(
                        job.as_raw_handle() as _,
                        super::GetCurrentProcess(),
                    )
                },
                0
            );
            jobs.push(job);
        }

        let mut probe = std::process::Command::new(&executable)
            .creation_flags(
                super::CREATE_SUSPENDED
                    | super::DETACHED_PROCESS
                    | super::CREATE_BREAKAWAY_FROM_JOB,
            )
            .spawn()
            .expect("breakaway probe");
        let mut in_outer = 0;
        let mut in_inner = 0;
        let outer_checked = unsafe {
            IsProcessInJob(
                probe.as_raw_handle() as _,
                jobs[0].as_raw_handle() as _,
                &mut in_outer,
            )
        };
        let inner_checked = unsafe {
            IsProcessInJob(
                probe.as_raw_handle() as _,
                jobs[1].as_raw_handle() as _,
                &mut in_inner,
            )
        };
        probe.kill().expect("terminate probe");
        probe.wait().expect("reap probe");
        assert_ne!(outer_checked, 0);
        assert_ne!(inner_checked, 0);
        assert_ne!(in_outer, 0, "probe must retain its outer job");
        assert_eq!(in_inner, 0, "probe must leave its inner job");
        super::ensure_detached_launch(&executable).expect("residual job must be accepted");

        if let Err(err) = super::ensure_not_elevated() {
            assert!(err.to_string().contains("non-elevated terminal"));
            return;
        }
        let temp = tempfile::tempdir().expect("temp dir");
        let script = temp.path().join("codex.cmd");
        std::fs::write(
            &script,
            "@echo off\r\nif \"%3\"==\"--help\" exit /b 1\r\necho ready > \"%~dp0ready\"\r\nfor /l %%i in (1,1,10000000) do @rem\r\n",
        )
        .expect("write daemon fixture");
        let backend = crate::backend::pid::PidBackend::new(
            script,
            temp.path().join("state").join("daemon.pid"),
            /*remote_control_enabled*/ false,
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let pid = runtime
            .block_on(backend.start())
            .expect("daemon launch")
            .expect("new daemon pid");
        let process = Process::open(pid)
            .expect("open daemon")
            .expect("live daemon");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !temp.path().join("ready").exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let ready = temp.path().join("ready").exists();
        let outer_checked = unsafe {
            IsProcessInJob(
                process.0.as_raw_handle() as _,
                jobs[0].as_raw_handle() as _,
                &mut in_outer,
            )
        };
        let inner_checked = unsafe {
            IsProcessInJob(
                process.0.as_raw_handle() as _,
                jobs[1].as_raw_handle() as _,
                &mut in_inner,
            )
        };
        process.terminate().expect("terminate daemon fixture");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while process.is_running().expect("daemon liveness") && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(!process.is_running().expect("daemon liveness"));
        assert!(ready, "daemon fixture did not start");
        assert_ne!(outer_checked, 0);
        assert_ne!(inner_checked, 0);
        assert_ne!(in_outer, 0, "daemon must retain its outer job");
        assert_eq!(in_inner, 0, "daemon must leave its inner job");
        return;
    }
    let output = std::process::Command::new(executable)
        .args([
            "--exact",
            "backend::windows::tests::detached_launch_preflight_allows_residual_job",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .expect("isolated job test");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}

#[tokio::test]
async fn identity_queries_do_not_require_termination_access() {
    let mut child = tokio::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Sleep 60",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .kill_on_drop(true)
        .spawn()
        .expect("child");
    let process = Process::open(child.id().expect("pid"))
        .expect("query handle")
        .expect("live process");
    assert!(!process.start_time().expect("creation time").is_empty());
    assert!(process.is_running().expect("liveness"));
    // Check the rights on the actual query handle, independent of privileges
    // that could let the caller reopen the process with termination access.
    assert_eq!(
        unsafe {
            TerminateProcess(process.0.as_raw_handle() as _, /*uexitcode*/ 1)
        },
        0
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(ERROR_ACCESS_DENIED as i32)
    );
    assert!(
        process
            .is_running()
            .expect("query must not terminate child")
    );
    child
        .kill()
        .await
        .expect("cleanup through original spawn handle");
}
