use super::collect_split_output;
use super::windows_job_test_support::TestDirectory;
use crate::SpawnedProcess;
use crate::spawn_pipe_process;
use crate::spawn_pipe_process_no_stdin;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

struct PipeParityCase {
    name: &'static str,
    program: String,
    args: Vec<String>,
    cwd: PathBuf,
    env: HashMap<String, String>,
    stdin: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq, Eq)]
struct PipeResult {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_code: i32,
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

async fn run_tokio_pipe(case: &PipeParityCase) -> anyhow::Result<PipeResult> {
    let mut command = tokio::process::Command::new(&case.program);
    command
        .args(&case.args)
        .current_dir(&case.cwd)
        .env_clear()
        .envs(&case.env)
        .stdin(if case.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    if let Some(input) = &case.stdin {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("reference child has no stdin"))?;
        stdin.write_all(input).await?;
        stdin.shutdown().await?;
    }
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("reference child has no stdout"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("reference child has no stderr"))?;
    let stdout_task = tokio::spawn(async move {
        let mut output = Vec::new();
        stdout.read_to_end(&mut output).await.map(|_| output)
    });
    let stderr_task = tokio::spawn(async move {
        let mut output = Vec::new();
        stderr.read_to_end(&mut output).await.map(|_| output)
    });
    let timeout = tokio::time::Duration::from_secs(15);
    let status = tokio::time::timeout(timeout, child.wait())
        .await
        .map_err(|_| anyhow::anyhow!("reference process timed out"))??;
    let stdout = tokio::time::timeout(timeout, stdout_task)
        .await
        .map_err(|_| anyhow::anyhow!("reference stdout timed out"))???;
    let stderr = tokio::time::timeout(timeout, stderr_task)
        .await
        .map_err(|_| anyhow::anyhow!("reference stderr timed out"))???;
    Ok(PipeResult {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
    })
}

async fn run_raw_pipe(case: &PipeParityCase) -> anyhow::Result<PipeResult> {
    let arg0 = Some("this-arg0-must-be-ignored-on-windows".to_string());
    let spawned = if case.stdin.is_some() {
        spawn_pipe_process(&case.program, &case.args, &case.cwd, &case.env, &arg0).await?
    } else {
        spawn_pipe_process_no_stdin(&case.program, &case.args, &case.cwd, &case.env, &arg0).await?
    };
    let SpawnedProcess {
        session,
        stdout_rx,
        stderr_rx,
        exit_rx,
    } = spawned;
    if let Some(input) = &case.stdin {
        let writer = session.writer_sender();
        writer.send(input.clone()).await?;
        drop(writer);
        session.close_stdin();
    }
    let stdout_task = tokio::spawn(collect_split_output(stdout_rx));
    let stderr_task = tokio::spawn(collect_split_output(stderr_rx));
    let timeout = tokio::time::Duration::from_secs(15);
    let exit_code = tokio::time::timeout(timeout, exit_rx)
        .await
        .map_err(|_| anyhow::anyhow!("raw pipe process timed out"))?
        .unwrap_or(-1);
    let stdout = tokio::time::timeout(timeout, stdout_task)
        .await
        .map_err(|_| anyhow::anyhow!("raw pipe stdout timed out"))??;
    let stderr = tokio::time::timeout(timeout, stderr_task)
        .await
        .map_err(|_| anyhow::anyhow!("raw pipe stderr timed out"))??;
    Ok(PipeResult {
        stdout,
        stderr,
        exit_code,
    })
}

fn set_case_insensitive_env(environment: &mut HashMap<String, String>, key: &str, value: String) {
    environment.retain(|candidate, _| !candidate.eq_ignore_ascii_case(key));
    environment.insert(key.to_string(), value);
}

fn path_with_prefix(directory: &Path) -> anyhow::Result<String> {
    let mut paths = vec![directory.to_owned()];
    if let Some(parent_path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&parent_path));
    }
    Ok(std::env::join_paths(paths)?.to_string_lossy().into_owned())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raw_pipe_matches_tokio_command_for_windows_process_semantics() -> anyhow::Result<()> {
    let directory = TestDirectory::new("pipe-parity")?;
    let unicode_cwd = directory.join("cwd-漢字-é");
    fs::create_dir(&unicode_cwd)?;
    let command_interpreter = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
    let mut environment: HashMap<String, String> = std::env::vars().collect();
    set_case_insensitive_env(
        &mut environment,
        "CODEX_UNICODE_VALUE",
        "café-漢字".to_string(),
    );

    let batch_script = directory.join("args-probe.cmd");
    fs::write(
        &batch_script,
        "@echo off\r\nsetlocal DisableDelayedExpansion\r\necho arg1=[%~1]\r\necho arg2=[%~2]\r\necho arg3=[%~3]\r\necho arg4=[%~4]\r\necho arg5=[%~5]\r\necho arg6=[%~6]\r\necho env=[%CODEX_UNICODE_VALUE%]\r\necho cwd=[%CD%]\r\nexit /b 0\r\n",
    )?;

    let path_directory = directory.join("path-bin");
    fs::create_dir(&path_directory)?;
    let path_executable_name = format!("codex-path-probe-{}", std::process::id());
    let copied_executable = path_directory.join(format!("{path_executable_name}.exe"));
    fs::copy(&command_interpreter, &copied_executable)?;
    let path_batch_name = format!("codex-batch-probe-{}.cmd", std::process::id());
    fs::write(
        path_directory.join(&path_batch_name),
        "@echo off\r\necho child-path-batch\r\nexit /b 0\r\n",
    )?;
    let path_bat_name = format!("codex-bat-probe-{}.bat", std::process::id());
    fs::write(
        path_directory.join(&path_bat_name),
        "@echo off\r\necho child-path-bat\r\nexit /b 0\r\n",
    )?;
    let spaced_directory = directory.join("space bin");
    fs::create_dir(&spaced_directory)?;
    let spaced_executable = spaced_directory.join("probe executable.exe");
    fs::copy(&command_interpreter, &spaced_executable)?;
    let mut path_environment = environment.clone();
    set_case_insensitive_env(
        &mut path_environment,
        "PATH",
        path_with_prefix(&path_directory)?,
    );

    let mut cases = vec![
        PipeParityCase {
            name: "split output and exit 37",
            program: command_interpreter.clone(),
            args: vec![
                "/D".to_string(),
                "/Q".to_string(),
                "/C".to_string(),
                "(echo split-out)&(echo split-err 1>&2)&exit /b 37".to_string(),
            ],
            cwd: unicode_cwd.clone(),
            env: environment.clone(),
            stdin: None,
        },
        PipeParityCase {
            name: "closed stdin at process start",
            program: command_interpreter.clone(),
            args: vec![
                "/D".to_string(),
                "/Q".to_string(),
                "/C".to_string(),
                "(set /p line=)||(echo stdin-closed)".to_string(),
            ],
            cwd: unicode_cwd.clone(),
            env: environment.clone(),
            stdin: None,
        },
        PipeParityCase {
            name: "batch quoting and Unicode environment/cwd",
            program: batch_script.to_string_lossy().into_owned(),
            args: vec![
                String::new(),
                "two words".to_string(),
                "quote\"value".to_string(),
                "trailing\\".to_string(),
                "100%".to_string(),
                "漢字-é".to_string(),
            ],
            cwd: unicode_cwd.clone(),
            env: environment.clone(),
            stdin: None,
        },
        PipeParityCase {
            name: "extensionless exe from child PATH",
            program: path_executable_name,
            args: vec![
                "/D".to_string(),
                "/Q".to_string(),
                "/C".to_string(),
                "echo child-path-exe".to_string(),
            ],
            cwd: unicode_cwd.clone(),
            env: path_environment.clone(),
            stdin: None,
        },
        PipeParityCase {
            name: "batch file from child PATH",
            program: path_batch_name,
            args: Vec::new(),
            cwd: unicode_cwd.clone(),
            env: path_environment.clone(),
            stdin: None,
        },
        PipeParityCase {
            name: "bat file from child PATH",
            program: path_bat_name,
            args: Vec::new(),
            cwd: unicode_cwd.clone(),
            env: path_environment,
            stdin: None,
        },
        PipeParityCase {
            name: "absolute executable path containing spaces",
            program: spaced_executable.to_string_lossy().into_owned(),
            args: vec![
                "/D".to_string(),
                "/Q".to_string(),
                "/C".to_string(),
                "echo absolute-space-exe".to_string(),
            ],
            cwd: unicode_cwd.clone(),
            env: environment.clone(),
            stdin: None,
        },
    ];

    if let Some(powershell) = find_powershell() {
        let powershell_script = directory.join("args-probe.ps1");
        fs::write(
            &powershell_script,
            "$OutputEncoding = [Console]::OutputEncoding = [Text.UTF8Encoding]::new()\nforeach ($arg in $args) { [Console]::Out.WriteLine([Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes([string]$arg))) }\n[Console]::Out.WriteLine([Environment]::GetEnvironmentVariable('CODEX_UNICODE_VALUE'))\n[Console]::Out.WriteLine((Get-Location).Path)\nexit 0\n",
        )?;
        cases.push(PipeParityCase {
            name: "regular executable quoting",
            program: powershell,
            args: vec![
                "-NoLogo".to_string(),
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-File".to_string(),
                powershell_script.to_string_lossy().into_owned(),
                String::new(),
                "two words".to_string(),
                "quote\"value".to_string(),
                "trailing\\".to_string(),
                "漢字-é".to_string(),
            ],
            cwd: unicode_cwd,
            env: environment,
            stdin: None,
        });
    }

    for case in cases {
        let reference = run_tokio_pipe(&case).await?;
        let raw = run_raw_pipe(&case).await?;
        pretty_assertions::assert_eq!(raw, reference, "parity case: {}", case.name);
    }
    Ok(())
}
