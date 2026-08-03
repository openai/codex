use super::collect_output_until_exit;
use super::combine_spawned_output;
use super::find_python;
use super::wait_for_output_contains;
use crate::TerminalSize;
use crate::spawn_pipe_process_no_stdin;
use crate::spawn_pty_process;
use std::collections::HashMap;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use winapi::um::jobapi::IsProcessInJob;
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;

const READY_MARKER: &str = "__CODEX_CHILD_READY__";
const VALUE_MARKER: &str = "__CODEX_CHILD_VALUE__";

struct WindowsShell {
    name: &'static str,
    program: String,
    args: Vec<String>,
    child_command: String,
}

fn find_powershell() -> Option<String> {
    ["pwsh.exe", "powershell.exe"]
        .into_iter()
        .find_map(|candidate| {
            std::process::Command::new(candidate)
                .args(["-NoLogo", "-NoProfile", "-Command", "exit 0"])
                .status()
                .ok()
                .filter(std::process::ExitStatus::success)
                .map(|_| candidate.to_string())
        })
}

fn utf8_hex(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

async fn wait_for_path(path: &Path, timeout: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if path.exists() {
            return true;
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return false;
        }
        tokio::time::sleep(remaining.min(Duration::from_millis(25))).await;
    }
}

async fn assert_terminate_kills_descendant(
    backend: &str,
    python: &str,
    env: &HashMap<String, String>,
) -> anyhow::Result<()> {
    let marker = std::env::temp_dir().join(format!(
        "codex-job-descendant-{backend}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    let child_code = format!(
        "import pathlib,time; print('{READY_MARKER}',flush=True); time.sleep(1); pathlib.Path(bytes.fromhex('{}').decode()).write_text('survived')",
        utf8_hex(&marker.to_string_lossy())
    );
    // Exercise descendants created after the best-effort pipe assignment,
    // without making the test depend on winning the intentionally accepted race.
    let code = format!(
        "import subprocess,sys,time; time.sleep(0.5); code=bytes.fromhex('{}').decode(); subprocess.Popen([sys.executable,'-u','-c',code]); time.sleep(60)",
        utf8_hex(&child_code)
    );
    let args = vec!["-u".to_string(), "-c".to_string(), code];
    let spawned = if backend == "pipe" {
        spawn_pipe_process_no_stdin(python, &args, Path::new("."), env, /*arg0*/ &None, &[]).await?
    } else {
        spawn_pty_process(
            python,
            &args,
            Path::new("."),
            env,
            /*arg0*/ &None,
            TerminalSize::default(),
            &[],
        )
        .await?
    };
    let (session, mut output_rx, exit_rx) = combine_spawned_output(spawned);
    wait_for_output_contains(&mut output_rx, READY_MARKER, /*timeout_ms*/ 10_000).await?;
    session.request_terminate();
    let (_, exit_code) = collect_output_until_exit(output_rx, exit_rx, /*timeout_ms*/ 10_000).await;
    assert_ne!(
        exit_code, -1,
        "{backend} root did not exit after termination"
    );
    tokio::time::sleep(Duration::from_secs(2)).await;
    let survived = marker.exists();
    if survived {
        std::fs::remove_file(&marker)?;
    }
    assert!(!survived, "{backend} descendant survived termination");
    Ok(())
}

async fn assert_normal_exit_preserves_descendant(
    backend: &str,
    python: &str,
    env: &HashMap<String, String>,
) -> anyhow::Result<()> {
    let marker_base = std::env::temp_dir().join(format!(
        "codex-job-natural-exit-{backend}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    let ready_marker = marker_base.with_extension("ready");
    let survival_marker = marker_base.with_extension("survived");
    let child_code = format!(
        "import pathlib,time; pathlib.Path(bytes.fromhex('{}').decode()).write_text('ready'); time.sleep(1); pathlib.Path(bytes.fromhex('{}').decode()).write_text('survived')",
        utf8_hex(&ready_marker.to_string_lossy()),
        utf8_hex(&survival_marker.to_string_lossy())
    );
    let code = format!(
        "import pathlib,subprocess,sys,time; code=bytes.fromhex('{}').decode(); ready=pathlib.Path(bytes.fromhex('{}').decode()); subprocess.Popen([sys.executable,'-u','-c',code],stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=subprocess.DETACHED_PROCESS|subprocess.CREATE_NEW_PROCESS_GROUP); deadline=time.time()+10\nwhile not ready.exists() and time.time()<deadline: time.sleep(.05)\nsys.exit(0 if ready.exists() else 2)",
        utf8_hex(&child_code),
        utf8_hex(&ready_marker.to_string_lossy())
    );
    let args = vec!["-u".to_string(), "-c".to_string(), code];
    let spawned = if backend == "pipe" {
        spawn_pipe_process_no_stdin(python, &args, Path::new("."), env, /*arg0*/ &None, &[]).await?
    } else {
        spawn_pty_process(
            python,
            &args,
            Path::new("."),
            env,
            /*arg0*/ &None,
            TerminalSize::default(),
            &[],
        )
        .await?
    };
    let (session, output_rx, exit_rx) = combine_spawned_output(spawned);
    let (_, exit_code) = collect_output_until_exit(output_rx, exit_rx, /*timeout_ms*/ 10_000).await;
    assert_eq!(exit_code, 0, "{backend} root did not exit normally");
    drop(session);

    let survived = wait_for_path(&survival_marker, Duration::from_secs(10)).await;
    let _ = std::fs::remove_file(ready_marker);
    let _ = std::fs::remove_file(survival_marker);
    assert!(survived, "{backend} descendant did not survive normal exit");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn terminate_kills_descendants_for_best_effort_pipe_and_atomic_conpty() -> anyhow::Result<()>
{
    let Some(python) = find_python() else {
        eprintln!("python not found; skipping Windows process-tree termination test");
        return Ok(());
    };
    let env: HashMap<String, String> = std::env::vars().collect();
    assert_terminate_kills_descendant("pipe", &python, &env).await?;
    assert_terminate_kills_descendant("ConPTY", &python, &env).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn normal_exit_preserves_descendants_for_pipe_and_conpty() -> anyhow::Result<()> {
    let Some(python) = find_python() else {
        eprintln!("python not found; skipping Windows process-tree natural-exit test");
        return Ok(());
    };
    let env: HashMap<String, String> = std::env::vars().collect();
    assert_normal_exit_preserves_descendant("pipe", &python, &env).await?;
    assert_normal_exit_preserves_descendant("ConPTY", &python, &env).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contained_spawn_owns_immediate_descendant() -> anyhow::Result<()> {
    let Some(python) = find_python() else {
        eprintln!("python not found; skipping Windows contained-spawn test");
        return Ok(());
    };

    let mut command = Command::new(&python);
    command
        .args([
            "-u",
            "-c",
            "import subprocess,sys; child=subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)']); print(child.pid,flush=True); child.wait()",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let job = crate::JobObject::create()?;
    let mut root = job.spawn_contained(&mut command)?;
    let stdout = root
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("missing contained process stdout"))?;
    let mut stdout = BufReader::new(stdout);
    let mut child_pid = String::new();
    tokio::time::timeout(Duration::from_secs(10), stdout.read_line(&mut child_pid)).await??;
    let child_pid: u32 = child_pid.trim().parse()?;

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, child_pid) };
    anyhow::ensure!(!process.is_null(), "failed to open immediate child process");
    let process = unsafe { OwnedHandle::from_raw_handle(process.cast()) };
    let mut in_job = 0;
    let checked = unsafe {
        IsProcessInJob(
            process.as_raw_handle().cast(),
            job.as_raw_handle().cast(),
            &mut in_job,
        )
    };
    anyhow::ensure!(checked != 0, "failed to inspect child Job Object");
    anyhow::ensure!(in_job != 0, "immediate child escaped its Job Object");

    job.terminate()?;
    tokio::time::timeout(Duration::from_secs(10), root.wait()).await??;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conpty_delivers_input_to_foreground_children() -> anyhow::Result<()> {
    let Some(python) = find_python() else {
        eprintln!("python not found; skipping ConPTY input test");
        return Ok(());
    };
    let code = format!(
        "print('__CODEX_CHILD_'+'READY__', flush=True); value=input(); print('{VALUE_MARKER}'+value.encode('utf-8').hex(), flush=True)"
    );
    let expected = "cafeé 漢字";
    let expected_marker = format!("{VALUE_MARKER}{}", utf8_hex(expected));
    let mut shells = vec![WindowsShell {
        name: "cmd",
        program: std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string()),
        args: vec!["/D".to_string(), "/Q".to_string()],
        child_command: format!("\"{}\" -u -c \"{code}\"", python.replace('"', "\"\"")),
    }];
    if let Some(program) = find_powershell() {
        shells.push(WindowsShell {
            name: "PowerShell",
            program,
            args: vec!["-NoLogo".to_string(), "-NoProfile".to_string()],
            child_command: format!("& '{}' -u -c \"{code}\"", python.replace('\'', "''")),
        });
    }
    let env: HashMap<String, String> = std::env::vars().collect();

    for shell in shells {
        let spawned = spawn_pty_process(
            &shell.program,
            &shell.args,
            Path::new("."),
            &env,
            /*arg0*/ &None,
            TerminalSize::default(),
            &[],
        )
        .await?;
        let (session, mut output_rx, exit_rx) = combine_spawned_output(spawned);
        let writer = session.writer_sender();
        writer
            .send(format!("{}\n", shell.child_command).into_bytes())
            .await?;
        wait_for_output_contains(&mut output_rx, READY_MARKER, /*timeout_ms*/ 10_000)
            .await
            .map_err(|err| anyhow::anyhow!("{} child did not become ready: {err}", shell.name))?;

        writer
            .send(format!("{expected}X\u{8}\n").into_bytes())
            .await?;
        let mut output =
            wait_for_output_contains(&mut output_rx, &expected_marker, /*timeout_ms*/ 10_000)
                .await
                .map_err(|err| {
                    anyhow::anyhow!("{} child received incorrect input: {err}", shell.name)
                })?;

        writer.send(b"exit 0\n".to_vec()).await?;
        let (remaining, exit_code) =
            collect_output_until_exit(output_rx, exit_rx, /*timeout_ms*/ 10_000).await;
        output.extend_from_slice(&remaining);

        assert_eq!(
            exit_code,
            0,
            "{} did not exit cleanly: {:?}",
            shell.name,
            String::from_utf8_lossy(&output)
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conpty_ctrl_c_interrupts_powershell_foreground_child() -> anyhow::Result<()> {
    let Some(program) = find_powershell() else {
        return Ok(());
    };
    let args = vec!["-NoLogo".to_string(), "-NoProfile".to_string()];
    let env: HashMap<String, String> = std::env::vars().collect();
    let spawned = spawn_pty_process(
        &program,
        &args,
        Path::new("."),
        &env,
        /*arg0*/ &None,
        TerminalSize::default(),
        &[],
    )
    .await?;
    let (session, mut output_rx, exit_rx) = combine_spawned_output(spawned);
    let writer = session.writer_sender();
    writer.send(b"ping.exe -4 -t localhost\n".to_vec()).await?;
    wait_for_output_contains(&mut output_rx, "127.0.0.1", /*timeout_ms*/ 10_000).await?;

    writer.send(vec![0x03]).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    writer.send(b"cmd.exe /D /C ver\n".to_vec()).await?;
    let mut output = wait_for_output_contains(
        &mut output_rx,
        "Microsoft Windows",
        /*timeout_ms*/ 10_000,
    )
    .await?;

    writer.send(b"exit 0\n".to_vec()).await?;
    let (remaining, exit_code) =
        collect_output_until_exit(output_rx, exit_rx, /*timeout_ms*/ 10_000).await;
    output.extend_from_slice(&remaining);
    assert_eq!(
        exit_code,
        0,
        "PowerShell did not resume after Ctrl-C: {:?}",
        String::from_utf8_lossy(&output)
    );
    Ok(())
}
