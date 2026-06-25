#![cfg(target_os = "windows")]

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::time::Duration;
use std::time::Instant;
use tempfile::TempDir;

static WINDOWS_PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

pub(super) fn windows_process_test_guard() -> MutexGuard<'static, ()> {
    WINDOWS_PROCESS_TEST_LOCK
        .lock()
        .expect("Windows sandbox process test lock poisoned")
}

pub(super) fn windows_powershell_path() -> PathBuf {
    let system_root = std::env::var_os("SystemRoot").expect("SystemRoot should be set on Windows");
    let path = PathBuf::from(system_root).join("System32\\WindowsPowerShell\\v1.0\\powershell.exe");
    assert!(
        path.is_file(),
        "Windows PowerShell is required for job lifecycle tests: {}",
        path.display()
    );
    path
}

fn powershell_single_quoted(value: &Path) -> String {
    value.display().to_string().replace('\'', "''")
}

fn powershell_encoded_command(script: &str) -> String {
    let bytes = script
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    BASE64_STANDARD.encode(bytes)
}

pub(super) struct GrandchildFixture {
    _test_dir: TempDir,
    ticks_path: PathBuf,
    ready_path: PathBuf,
    pub(super) command: Vec<String>,
}

pub(super) fn grandchild_fixture(
    cwd: &Path,
    powershell: &Path,
    root_tail: &str,
) -> GrandchildFixture {
    let test_dir = tempfile::tempdir_in(cwd).expect("create grandchild test directory");
    let ticks_path = test_dir.path().join("ticks.txt");
    let ready_path = test_dir.path().join("ready.txt");
    let child_script = format!(
        "while ($true) {{ [IO.File]::AppendAllText('{}', 'x'); Start-Sleep -Milliseconds 25 }}",
        powershell_single_quoted(&ticks_path)
    );
    let root_script = format!(
        "$child = Start-Process -PassThru -FilePath '{}' -ArgumentList @('-NoProfile', '-EncodedCommand', '{}'); [IO.File]::WriteAllText('{}', [string]$child.Id); {root_tail}",
        powershell_single_quoted(powershell),
        powershell_encoded_command(&child_script),
        powershell_single_quoted(&ready_path),
    );
    GrandchildFixture {
        _test_dir: test_dir,
        ticks_path,
        ready_path,
        command: vec![
            powershell.display().to_string(),
            "-NoProfile".to_string(),
            "-EncodedCommand".to_string(),
            powershell_encoded_command(&root_script),
        ],
    }
}

pub(super) fn wait_for_grandchild(fixture: &GrandchildFixture) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while (!fixture.ready_path.exists()
        || fs::metadata(&fixture.ticks_path)
            .map(|meta| meta.len())
            .unwrap_or(0)
            < 3)
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        fixture.ready_path.exists(),
        "root did not report child startup"
    );
    assert!(
        fs::metadata(&fixture.ticks_path)
            .map(|meta| meta.len())
            .unwrap_or(0)
            >= 3,
        "grandchild did not write ticks"
    );
}

pub(super) fn assert_grandchild_stopped(fixture: &GrandchildFixture) {
    let length_after_exit = fs::metadata(&fixture.ticks_path)
        .expect("ticks file after root exit")
        .len();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        fs::metadata(&fixture.ticks_path)
            .expect("ticks file after stability wait")
            .len(),
        length_after_exit,
        "grandchild kept writing after the session job closed"
    );
}

#[derive(Clone, Copy)]
pub(super) enum SessionMode {
    Pipe,
    Tty,
}

impl SessionMode {
    pub(super) fn tty(self) -> bool {
        matches!(self, Self::Tty)
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Pipe => "pipe",
            Self::Tty => "tty",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum SessionEnding {
    ExplicitTermination,
    RootExit,
}

impl SessionEnding {
    pub(super) fn root_tail(self) -> &'static str {
        match self {
            Self::ExplicitTermination => "Start-Sleep -Seconds 30",
            Self::RootExit => "Start-Sleep -Milliseconds 500",
        }
    }
}
