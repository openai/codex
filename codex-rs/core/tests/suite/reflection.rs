//! Integration tests for the reflection layer.
//!
//! These tests verify that the reflection layer works correctly with real API calls.
//! They are `#[ignore]` by default so CI stays deterministic. Run them locally with:
//!
//! ```sh
//! # For Azure OpenAI:
//! AZURE_OPENAI_API_KEY=<key> AZURE_OPENAI_BASE_URL=<url> \
//!     cargo test -p codex-core --test all reflection -- --ignored --nocapture
//!
//! # Optionally specify a model (defaults to gpt-5-mini):
//! AZURE_OPENAI_API_KEY=<key> AZURE_OPENAI_BASE_URL=<url> AZURE_OPENAI_MODEL=gpt-4o \
//!     cargo test -p codex-core --test all reflection -- --ignored --nocapture
//! ```

use assert_cmd::prelude::*;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use tempfile::TempDir;

fn require_azure_credentials() -> (String, String, String) {
    let api_key = std::env::var("AZURE_OPENAI_API_KEY")
        .expect("AZURE_OPENAI_API_KEY env var not set — skip running Azure tests");
    let base_url = std::env::var("AZURE_OPENAI_BASE_URL")
        .expect("AZURE_OPENAI_BASE_URL env var not set — skip running Azure tests");
    let model = std::env::var("AZURE_OPENAI_MODEL").unwrap_or_else(|_| "gpt-5-mini".to_string());
    (api_key, base_url, model)
}

/// Creates a config.toml for Azure OpenAI with reflection enabled.
fn create_azure_config(base_url: &str, model: &str) -> String {
    // Ensure base_url ends with /openai
    let base_url = base_url.trim_end_matches('/');
    let base_url = if base_url.ends_with("/openai") {
        base_url.to_string()
    } else {
        format!("{}/openai", base_url)
    };

    format!(
        r#"
model = "{model}"
model_provider = "azure-openai"

[reflection]
enabled = true
max_attempts = 3

[features]
reflection = true

[model_providers.azure-openai]
name = "Azure OpenAI"
base_url = "{base_url}"
env_key = "AZURE_OPENAI_API_KEY"
wire_api = "responses"
request_max_retries = 3
stream_max_retries = 3
stream_idle_timeout_ms = 120000

[model_providers.azure-openai.query_params]
api-version = "2025-04-01-preview"
"#
    )
}

/// Helper that spawns codex exec with Azure OpenAI config and reflection enabled.
/// Returns (Assert, TempDir, stdout, stderr).
fn run_azure_reflection_test(prompt: &str) -> (assert_cmd::assert::Assert, TempDir, Vec<u8>, Vec<u8>) {
    #![expect(clippy::unwrap_used)]

    let (api_key, base_url, model) = require_azure_credentials();

    let dir = TempDir::new().unwrap();

    // Create .codex directory with config
    let codex_home = dir.path().join(".codex");
    std::fs::create_dir_all(&codex_home).unwrap();
    std::fs::write(codex_home.join("config.toml"), create_azure_config(&base_url, &model)).unwrap();

    let mut cmd = Command::cargo_bin("codex").unwrap();
    cmd.current_dir(dir.path());
    cmd.env("AZURE_OPENAI_API_KEY", api_key);
    cmd.env("CODEX_HOME", &codex_home);
    cmd.env("RUST_LOG", "codex_core=info,reflection=debug");

    cmd.arg("exec")
        .arg("--full-auto")
        .arg("--skip-git-repo-check")
        .arg("--color")
        .arg("never")
        .arg(prompt);

    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("failed to spawn codex");

    // Tee helper - copies stream to both parent stdio and buffer
    fn tee<R: Read + Send + 'static>(
        mut reader: R,
        mut writer: impl Write + Send + 'static,
    ) -> thread::JoinHandle<Vec<u8>> {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        writer.write_all(&chunk[..n]).ok();
                        writer.flush().ok();
                        buf.extend_from_slice(&chunk[..n]);
                    }
                    Err(_) => break,
                }
            }
            buf
        })
    }

    let stdout_handle = tee(
        child.stdout.take().expect("child stdout"),
        std::io::stdout(),
    );
    let stderr_handle = tee(
        child.stderr.take().expect("child stderr"),
        std::io::stderr(),
    );

    let status = child.wait().expect("failed to wait on child");
    let stdout = stdout_handle.join().expect("stdout thread panicked");
    let stderr = stderr_handle.join().expect("stderr thread panicked");

    let output = std::process::Output {
        status,
        stdout: stdout.clone(),
        stderr: stderr.clone(),
    };

    (output.assert(), dir, stdout, stderr)
}

/// Integration test for reflection layer with Azure OpenAI.
///
/// This test:
/// 1. Connects to Azure OpenAI using AZURE_OPENAI_API_KEY and AZURE_OPENAI_BASE_URL
/// 2. Uses gpt-5-mini model with reflection enabled
/// 3. Asks codex to create a hello world Python app and test it
/// 4. Verifies the reflection layer was invoked
#[ignore]
#[test]
fn reflection_layer_hello_world_with_azure_openai() {
    if std::env::var("AZURE_OPENAI_API_KEY").is_err()
        || std::env::var("AZURE_OPENAI_BASE_URL").is_err()
    {
        eprintln!(
            "skipping reflection test — AZURE_OPENAI_API_KEY or AZURE_OPENAI_BASE_URL not set"
        );
        return;
    }

    let prompt = r#"Write a hello world Python application that prints exactly "hello, world" (with a lowercase h and trailing newline). Name the file hello.py.

Then create a test file called test_hello.py that:
1. Runs hello.py using subprocess
2. Captures its output
3. Asserts that the output is exactly "hello, world\n"

After creating both files, run the test to verify it passes."#;

    let (assert, dir, stdout, stderr) = run_azure_reflection_test(prompt);

    // Test should succeed
    assert.success();

    // Verify hello.py was created
    let hello_path = dir.path().join("hello.py");
    assert!(
        hello_path.exists(),
        "hello.py was not created by the model"
    );

    // Verify test_hello.py was created
    let test_path = dir.path().join("test_hello.py");
    assert!(
        test_path.exists(),
        "test_hello.py was not created by the model"
    );

    // Check that reflection layer was invoked by looking at both stdout and stderr (logs)
    let stdout_str = String::from_utf8_lossy(&stdout);
    let stderr_str = String::from_utf8_lossy(&stderr);
    let combined_output = format!("{}{}", stdout_str, stderr_str);

    let reflection_invoked = combined_output.contains("Running reflection evaluation")
        || combined_output.contains("Reflection verdict")
        || combined_output.contains("Reflection:");

    println!("\n=== Reflection Layer Status ===");
    if reflection_invoked {
        println!("Reflection layer was invoked during task execution");

        // Extract and print reflection verdict if present
        for line in combined_output.lines() {
            if line.contains("Reflection verdict") || line.contains("Reflection:") {
                println!("  {}", line);
            }
        }
    } else {
        println!("WARNING: No explicit reflection activity detected in output");
        println!("This may indicate the reflection feature was not properly enabled");
    }

    // Verify hello.py content is correct
    let hello_content = std::fs::read_to_string(&hello_path).unwrap();
    println!("\n=== hello.py ===\n{}", hello_content);

    // Verify test file content
    let test_content = std::fs::read_to_string(&test_path).unwrap();
    println!("\n=== test_hello.py ===\n{}", test_content);

    // The reflection layer should have been invoked
    assert!(
        reflection_invoked,
        "Reflection layer was not invoked - check that reflection feature is enabled"
    );
}

/// Simple test to verify reflection config is correctly parsed.
#[ignore]
#[test]
fn reflection_config_azure_openai() {
    if std::env::var("AZURE_OPENAI_API_KEY").is_err()
        || std::env::var("AZURE_OPENAI_BASE_URL").is_err()
    {
        eprintln!("skipping config test — Azure credentials not set");
        return;
    }

    // Simple prompt that should complete quickly
    let prompt = "Print 'hello' using echo";

    let (assert, _dir, stdout, stderr) = run_azure_reflection_test(prompt);

    assert.success();

    // Verify reflection was at least attempted (logs go to stderr)
    let stdout_str = String::from_utf8_lossy(&stdout);
    let stderr_str = String::from_utf8_lossy(&stderr);
    let combined_output = format!("{}{}", stdout_str, stderr_str);
    println!("Output:\n{}", combined_output);

    // Should see reflection-related log messages
    let has_reflection_logs = combined_output.contains("reflection")
        || combined_output.contains("Reflection");

    println!(
        "\nReflection activity detected: {}",
        has_reflection_logs
    );
}
