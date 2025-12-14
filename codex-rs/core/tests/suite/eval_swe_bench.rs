//! SWE-bench style evaluation tests.
//!
//! These tests evaluate the agent's ability to fix real-world style bugs,
//! inspired by SWE-bench benchmark tasks. Each test provides:
//! - A codebase with a bug
//! - An issue description
//! - Test files that verify the fix
//!
//! Run with reflection enabled to compare results:
//! ```sh
//! AZURE_OPENAI_API_KEY=<key> AZURE_OPENAI_BASE_URL=<url> \
//!     cargo test -p codex-core --test all eval_swe -- --ignored --nocapture
//! ```

use assert_cmd::prelude::*;
use std::fs;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use tempfile::TempDir;

fn require_azure_credentials() -> (String, String, String) {
    let api_key = std::env::var("AZURE_OPENAI_API_KEY")
        .expect("AZURE_OPENAI_API_KEY env var not set");
    let base_url = std::env::var("AZURE_OPENAI_BASE_URL")
        .expect("AZURE_OPENAI_BASE_URL env var not set");
    let model = std::env::var("AZURE_OPENAI_MODEL").unwrap_or_else(|_| "gpt-5-mini".to_string());
    (api_key, base_url, model)
}

/// Creates config.toml with optional reflection setting.
fn create_config(base_url: &str, model: &str, reflection_enabled: bool) -> String {
    let base_url = base_url.trim_end_matches('/');
    let base_url = if base_url.ends_with("/openai") {
        base_url.to_string()
    } else {
        format!("{}/openai", base_url)
    };

    let reflection_config = if reflection_enabled {
        r#"
[reflection]
enabled = true
max_attempts = 3

[features]
reflection = true
"#
    } else {
        ""
    };

    format!(
        r#"
model = "{model}"
model_provider = "azure-openai"
{reflection_config}
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

/// Result of an eval run.
#[derive(Debug)]
struct EvalResult {
    success: bool,
    reflection_used: bool,
    test_output: String,
    reflection_verdicts: Vec<String>,
}

/// Run an eval task and return the result.
fn run_eval_task(
    prompt: &str,
    setup_files: &[(&str, &str)],
    test_command: &str,
    reflection_enabled: bool,
) -> EvalResult {
    #![expect(clippy::unwrap_used)]

    let (api_key, base_url, model) = require_azure_credentials();

    let dir = TempDir::new().unwrap();
    let work_dir = dir.path();

    // Create setup files (buggy codebase)
    for (path, content) in setup_files {
        let file_path = work_dir.join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&file_path, content).unwrap();
    }

    // Create .codex directory with config
    let codex_home = work_dir.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    fs::write(
        codex_home.join("config.toml"),
        create_config(&base_url, &model, reflection_enabled),
    )
    .unwrap();

    // Run codex
    let mut cmd = Command::cargo_bin("codex").unwrap();
    cmd.current_dir(work_dir);
    cmd.env("AZURE_OPENAI_API_KEY", api_key);
    cmd.env("CODEX_HOME", &codex_home);
    cmd.env("RUST_LOG", "codex_core=info");

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

    // Tee helper
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

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    );

    // Extract reflection verdicts
    let reflection_verdicts: Vec<String> = combined
        .lines()
        .filter(|line| line.contains("Reflection verdict"))
        .map(|s| s.to_string())
        .collect();

    // Run the verification test
    let test_result = Command::new("bash")
        .arg("-c")
        .arg(test_command)
        .current_dir(work_dir)
        .output()
        .expect("failed to run test");

    let test_output = format!(
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&test_result.stdout),
        String::from_utf8_lossy(&test_result.stderr)
    );

    EvalResult {
        success: status.success() && test_result.status.success(),
        reflection_used: !reflection_verdicts.is_empty(),
        test_output,
        reflection_verdicts,
    }
}

// ============================================================================
// EVAL TASK 1: Off-by-one error in array processing
// ============================================================================

const TASK1_BUG_FILE: &str = r#"
def sum_first_n(arr, n):
    """Return sum of first n elements of arr."""
    total = 0
    for i in range(n + 1):  # BUG: should be range(n)
        total += arr[i]
    return total

def get_last_element(arr):
    """Return the last element of arr."""
    return arr[len(arr)]  # BUG: should be arr[len(arr) - 1] or arr[-1]
"#;

const TASK1_TEST_FILE: &str = r#"
import pytest
from array_utils import sum_first_n, get_last_element

def test_sum_first_n():
    arr = [1, 2, 3, 4, 5]
    assert sum_first_n(arr, 3) == 6  # 1 + 2 + 3
    assert sum_first_n(arr, 1) == 1
    assert sum_first_n(arr, 5) == 15

def test_get_last_element():
    assert get_last_element([1, 2, 3]) == 3
    assert get_last_element([10]) == 10
    assert get_last_element(['a', 'b', 'c']) == 'c'
"#;

const TASK1_ISSUE: &str = r#"Fix the off-by-one errors in array_utils.py

The file array_utils.py has two functions with index errors:

1. `sum_first_n(arr, n)` - Should return sum of first n elements, but it's including one extra element
2. `get_last_element(arr)` - Should return the last element, but it's causing an IndexError

Please fix these bugs so that test_array_utils.py passes.

After fixing, run: pytest test_array_utils.py -v
"#;

#[ignore]
#[test]
fn eval_task1_offbyone_with_reflection() {
    if std::env::var("AZURE_OPENAI_API_KEY").is_err() {
        eprintln!("skipping eval — Azure credentials not set");
        return;
    }

    let result = run_eval_task(
        TASK1_ISSUE,
        &[
            ("array_utils.py", TASK1_BUG_FILE),
            ("test_array_utils.py", TASK1_TEST_FILE),
        ],
        "pytest test_array_utils.py -v",
        true, // reflection enabled
    );

    println!("\n=== EVAL TASK 1 (with reflection) ===");
    println!("Success: {}", result.success);
    println!("Reflection used: {}", result.reflection_used);
    println!("Reflection verdicts: {:?}", result.reflection_verdicts);
    println!("Test output:\n{}", result.test_output);

    assert!(result.success, "Task 1 failed with reflection enabled");
}

#[ignore]
#[test]
fn eval_task1_offbyone_without_reflection() {
    if std::env::var("AZURE_OPENAI_API_KEY").is_err() {
        eprintln!("skipping eval — Azure credentials not set");
        return;
    }

    let result = run_eval_task(
        TASK1_ISSUE,
        &[
            ("array_utils.py", TASK1_BUG_FILE),
            ("test_array_utils.py", TASK1_TEST_FILE),
        ],
        "pytest test_array_utils.py -v",
        false, // reflection disabled
    );

    println!("\n=== EVAL TASK 1 (without reflection) ===");
    println!("Success: {}", result.success);
    println!("Reflection used: {}", result.reflection_used);
    println!("Test output:\n{}", result.test_output);

    // Don't assert - just report for comparison
    println!(
        "Result: {}",
        if result.success { "PASS" } else { "FAIL" }
    );
}

// ============================================================================
// EVAL TASK 2: Logic error in string processing
// ============================================================================

const TASK2_BUG_FILE: &str = r#"
def is_palindrome(s):
    """Check if string is a palindrome (case-insensitive, ignoring spaces)."""
    # BUG: doesn't handle case or spaces
    return s == s[::-1]

def count_words(text):
    """Count number of words in text."""
    # BUG: doesn't handle multiple spaces or leading/trailing spaces
    return len(text.split(' '))
"#;

const TASK2_TEST_FILE: &str = r#"
import pytest
from string_utils import is_palindrome, count_words

def test_is_palindrome():
    assert is_palindrome("racecar") == True
    assert is_palindrome("A man a plan a canal Panama") == True
    assert is_palindrome("Race Car") == True
    assert is_palindrome("hello") == False

def test_count_words():
    assert count_words("hello world") == 2
    assert count_words("  hello   world  ") == 2
    assert count_words("one") == 1
    assert count_words("a b c d e") == 5
"#;

const TASK2_ISSUE: &str = r#"Fix logic errors in string_utils.py

The string_utils.py file has two functions with logic bugs:

1. `is_palindrome(s)` - Should check if string is palindrome, ignoring case and spaces
   Currently fails on "Race Car" and "A man a plan a canal Panama"

2. `count_words(text)` - Should count words, handling multiple spaces correctly
   Currently gives wrong count for "  hello   world  "

Fix these bugs so that test_string_utils.py passes.

After fixing, run: pytest test_string_utils.py -v
"#;

#[ignore]
#[test]
fn eval_task2_string_logic_with_reflection() {
    if std::env::var("AZURE_OPENAI_API_KEY").is_err() {
        eprintln!("skipping eval — Azure credentials not set");
        return;
    }

    let result = run_eval_task(
        TASK2_ISSUE,
        &[
            ("string_utils.py", TASK2_BUG_FILE),
            ("test_string_utils.py", TASK2_TEST_FILE),
        ],
        "pytest test_string_utils.py -v",
        true,
    );

    println!("\n=== EVAL TASK 2 (with reflection) ===");
    println!("Success: {}", result.success);
    println!("Reflection used: {}", result.reflection_used);
    println!("Reflection verdicts: {:?}", result.reflection_verdicts);
    println!("Test output:\n{}", result.test_output);

    assert!(result.success, "Task 2 failed with reflection enabled");
}

#[ignore]
#[test]
fn eval_task2_string_logic_without_reflection() {
    if std::env::var("AZURE_OPENAI_API_KEY").is_err() {
        eprintln!("skipping eval — Azure credentials not set");
        return;
    }

    let result = run_eval_task(
        TASK2_ISSUE,
        &[
            ("string_utils.py", TASK2_BUG_FILE),
            ("test_string_utils.py", TASK2_TEST_FILE),
        ],
        "pytest test_string_utils.py -v",
        false,
    );

    println!("\n=== EVAL TASK 2 (without reflection) ===");
    println!("Success: {}", result.success);
    println!("Reflection used: {}", result.reflection_used);
    println!("Test output:\n{}", result.test_output);

    println!(
        "Result: {}",
        if result.success { "PASS" } else { "FAIL" }
    );
}

// ============================================================================
// EVAL TASK 3: Missing edge case handling
// ============================================================================

const TASK3_BUG_FILE: &str = r#"
def safe_divide(a, b):
    """Safely divide a by b, return None on error."""
    # BUG: doesn't handle division by zero
    return a / b

def find_max(numbers):
    """Find maximum value in list."""
    # BUG: doesn't handle empty list
    max_val = numbers[0]
    for n in numbers[1:]:
        if n > max_val:
            max_val = n
    return max_val

def get_element_at(arr, index):
    """Get element at index, return None if out of bounds."""
    # BUG: doesn't handle negative indices or out of bounds
    return arr[index]
"#;

const TASK3_TEST_FILE: &str = r#"
import pytest
from math_utils import safe_divide, find_max, get_element_at

def test_safe_divide():
    assert safe_divide(10, 2) == 5.0
    assert safe_divide(0, 5) == 0.0
    assert safe_divide(10, 0) is None  # Division by zero
    assert safe_divide(10, 3) == pytest.approx(3.333, rel=0.01)

def test_find_max():
    assert find_max([1, 5, 3, 9, 2]) == 9
    assert find_max([42]) == 42
    assert find_max([-5, -1, -10]) == -1
    assert find_max([]) is None  # Empty list

def test_get_element_at():
    arr = [10, 20, 30]
    assert get_element_at(arr, 0) == 10
    assert get_element_at(arr, 2) == 30
    assert get_element_at(arr, 5) is None  # Out of bounds
    assert get_element_at(arr, -1) is None  # Negative index
    assert get_element_at([], 0) is None  # Empty array
"#;

const TASK3_ISSUE: &str = r#"Fix missing edge case handling in math_utils.py

The math_utils.py has three functions that don't handle edge cases properly:

1. `safe_divide(a, b)` - Should return None when dividing by zero, but crashes instead

2. `find_max(numbers)` - Should return None for empty list, but crashes with IndexError

3. `get_element_at(arr, index)` - Should return None for:
   - Out of bounds indices
   - Negative indices
   - Empty arrays
   Currently crashes instead

Fix these edge cases so that test_math_utils.py passes.

After fixing, run: pytest test_math_utils.py -v
"#;

#[ignore]
#[test]
fn eval_task3_edge_cases_with_reflection() {
    if std::env::var("AZURE_OPENAI_API_KEY").is_err() {
        eprintln!("skipping eval — Azure credentials not set");
        return;
    }

    let result = run_eval_task(
        TASK3_ISSUE,
        &[
            ("math_utils.py", TASK3_BUG_FILE),
            ("test_math_utils.py", TASK3_TEST_FILE),
        ],
        "pytest test_math_utils.py -v",
        true,
    );

    println!("\n=== EVAL TASK 3 (with reflection) ===");
    println!("Success: {}", result.success);
    println!("Reflection used: {}", result.reflection_used);
    println!("Reflection verdicts: {:?}", result.reflection_verdicts);
    println!("Test output:\n{}", result.test_output);

    assert!(result.success, "Task 3 failed with reflection enabled");
}

#[ignore]
#[test]
fn eval_task3_edge_cases_without_reflection() {
    if std::env::var("AZURE_OPENAI_API_KEY").is_err() {
        eprintln!("skipping eval — Azure credentials not set");
        return;
    }

    let result = run_eval_task(
        TASK3_ISSUE,
        &[
            ("math_utils.py", TASK3_BUG_FILE),
            ("test_math_utils.py", TASK3_TEST_FILE),
        ],
        "pytest test_math_utils.py -v",
        false,
    );

    println!("\n=== EVAL TASK 3 (without reflection) ===");
    println!("Success: {}", result.success);
    println!("Reflection used: {}", result.reflection_used);
    println!("Test output:\n{}", result.test_output);

    println!(
        "Result: {}",
        if result.success { "PASS" } else { "FAIL" }
    );
}

/// Run all eval tasks and summarize results.
#[ignore]
#[test]
fn eval_summary() {
    if std::env::var("AZURE_OPENAI_API_KEY").is_err() {
        eprintln!("skipping eval — Azure credentials not set");
        return;
    }

    println!("\n========================================");
    println!("SWE-BENCH STYLE EVALUATION SUMMARY");
    println!("========================================\n");

    let tasks = [
        (
            "Task 1: Off-by-one errors",
            TASK1_ISSUE,
            vec![
                ("array_utils.py", TASK1_BUG_FILE),
                ("test_array_utils.py", TASK1_TEST_FILE),
            ],
            "pytest test_array_utils.py -v",
        ),
        (
            "Task 2: String logic errors",
            TASK2_ISSUE,
            vec![
                ("string_utils.py", TASK2_BUG_FILE),
                ("test_string_utils.py", TASK2_TEST_FILE),
            ],
            "pytest test_string_utils.py -v",
        ),
        (
            "Task 3: Missing edge cases",
            TASK3_ISSUE,
            vec![
                ("math_utils.py", TASK3_BUG_FILE),
                ("test_math_utils.py", TASK3_TEST_FILE),
            ],
            "pytest test_math_utils.py -v",
        ),
    ];

    let mut with_reflection_pass = 0;
    let mut without_reflection_pass = 0;

    for (name, issue, files, test_cmd) in tasks.iter() {
        println!("--- {} ---", name);

        // With reflection
        let result_with = run_eval_task(
            issue,
            &files.iter().map(|(a, b)| (*a, *b)).collect::<Vec<_>>(),
            test_cmd,
            true,
        );
        if result_with.success {
            with_reflection_pass += 1;
        }
        println!(
            "  With reflection:    {} (verdicts: {})",
            if result_with.success { "PASS" } else { "FAIL" },
            result_with.reflection_verdicts.len()
        );

        // Without reflection
        let result_without = run_eval_task(
            issue,
            &files.iter().map(|(a, b)| (*a, *b)).collect::<Vec<_>>(),
            test_cmd,
            false,
        );
        if result_without.success {
            without_reflection_pass += 1;
        }
        println!(
            "  Without reflection: {}",
            if result_without.success { "PASS" } else { "FAIL" }
        );
        println!();
    }

    println!("========================================");
    println!("RESULTS");
    println!("========================================");
    println!(
        "With reflection:    {}/{} tasks passed",
        with_reflection_pass,
        tasks.len()
    );
    println!(
        "Without reflection: {}/{} tasks passed",
        without_reflection_pass,
        tasks.len()
    );
    println!(
        "Improvement: {:+} tasks",
        with_reflection_pass as i32 - without_reflection_pass as i32
    );
}
