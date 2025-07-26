// Minimal integration test for --concurrent background spawning.
// Verifies that invoking the top-level CLI with --concurrent records a task entry
// in CODEX_HOME/tasks.jsonl and that multiple invocations append distinct task_ids.

use std::fs;
use std::io::Write;
use std::process::Command;
use std::time::{Duration, Instant};

use tempfile::TempDir;

// Skip helper when sandbox network disabled (mirrors existing tests' behavior).
fn network_disabled() -> bool {
    std::env::var(codex_core::exec::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok()
}

#[test]
fn concurrent_creates_task_records() {
    if network_disabled() {
        eprintln!("Skipping concurrent_creates_task_records due to sandbox network-disabled env");
        return;
    }

    // Temp home (CODEX_HOME) and separate temp git repo.
    let home = TempDir::new().expect("temp home");
    let repo = TempDir::new().expect("temp repo");

    // Initialize a minimal git repository (needed for --concurrent worktree logic).
    assert!(Command::new("git").arg("init").current_dir(repo.path()).status().unwrap().success());
    fs::write(repo.path().join("README.md"), "# temp\n").unwrap();
    assert!(Command::new("git").arg("add").arg(".").current_dir(repo.path()).status().unwrap().success());
    assert!(Command::new("git")
        .args(["commit", "-m", "init"]) // may warn about user/email; allow non-zero if commit already exists
        .current_dir(repo.path())
        .status()
        .map(|s| s.success())
        .unwrap_or(true));

    // SSE fixture so the spawned background exec does not perform a real network call.
    let fixture = home.path().join("fixture.sse");
    let mut f = fs::File::create(&fixture).unwrap();
    writeln!(f, "data: {{\"choices\":[{{\"delta\":{{\"content\":\"ok\"}}}}]}}\n").unwrap();
    writeln!(f, "data: {{\"choices\":[{{\"delta\":{{}}}}]}}\n").unwrap();
    writeln!(f, "data: [DONE]\n").unwrap();

    // Helper to run one concurrent invocation with a given prompt.
    let run_once = |prompt: &str| {
        let mut cmd = Command::new("cargo");
        cmd.arg("run")
            .arg("-p")
            .arg("codex-cli")
            .arg("--quiet")
            .arg("--")
            .arg("--concurrent")
            .arg("--full-auto")
            .arg("-C")
            .arg(repo.path())
            .arg(prompt);
        cmd.env("CODEX_HOME", home.path())
            .env("OPENAI_API_KEY", "dummy")
            .env("CODEX_RS_SSE_FIXTURE", &fixture)
            .env("OPENAI_BASE_URL", "http://unused.local");
        let output = cmd.output().expect("spawn codex");
        assert!(output.status.success(), "concurrent codex run failed: stderr={}", String::from_utf8_lossy(&output.stderr));
    };

    run_once("Add a cat in ASCII");
    run_once("Add hello world comment");

    // Wait for tasks.jsonl to contain at least two lines with task records.
    let tasks_path = home.path().join("tasks.jsonl");
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut lines: Vec<String> = Vec::new();
    while Instant::now() < deadline {
        if tasks_path.exists() {
            let content = fs::read_to_string(&tasks_path).unwrap_or_default();
            lines = content.lines().filter(|l| !l.trim().is_empty()).map(|s| s.to_string()).collect();
            if lines.len() >= 2 { break; }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(lines.len() >= 2, "Expected at least 2 task records, got {}", lines.len());

    // Parse JSON and ensure distinct task_ids and prompts present.
    let mut task_ids = std::collections::HashSet::new();
    let mut saw_cat = false;
    let mut saw_hello = false;
    for line in &lines {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(tid) = val.get("task_id").and_then(|v| v.as_str()) { task_ids.insert(tid.to_string()); }
            if let Some(p) = val.get("prompt").and_then(|v| v.as_str()) {
                if p.contains("cat") { saw_cat = true; }
                if p.contains("hello") { saw_hello = true; }
            }
            assert_eq!(val.get("state").and_then(|v| v.as_str()), Some("started"), "task record missing started state");
        }
    }
    assert!(task_ids.len() >= 2, "Expected distinct task_ids, got {:?}", task_ids);
    assert!(saw_cat, "Did not find cat prompt in tasks.jsonl");
    assert!(saw_hello, "Did not find hello prompt in tasks.jsonl");
} 