#![allow(clippy::expect_used)]

use anyhow::Context;
use anyhow::Result;
use core_test_support::responses::ev_apply_patch_call;
use core_test_support::test_codex::ApplyPatchModelOutput;
use pretty_assertions::assert_eq;
use regex_lite::Regex;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use test_case::test_case;
use walkdir::WalkDir;

use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;

#[derive(Clone, Copy)]
enum ApplyPatchExpectation {
    Success,
    Failure,
}

fn scenarios_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../apply-patch/tests/fixtures/scenarios")
}

fn scenario_expectation(name: &str) -> ApplyPatchExpectation {
    match name {
        "005_rejects_empty_patch"
        | "006_reports_missing_context"
        | "007_rejects_missing_file_delete"
        | "008_rejects_empty_update_hunk"
        | "009_requires_existing_file_for_update"
        | "012_delete_directory_fails"
        | "013_rejects_invalid_hunk_header"
        | "015_failure_after_partial_success_leaves_changes" => ApplyPatchExpectation::Failure,
        _ => ApplyPatchExpectation::Success,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
async fn test_apply_patch_scenario(output_type: ApplyPatchModelOutput) -> Result<()> {
    let root = scenarios_root();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name_os_str = entry.file_name();
        let name = name_os_str
            .to_str()
            .with_context(|| format!("invalid UTF-8 scenario name in {}", path.display()))?;
        let expectation = scenario_expectation(name);
        run_scenario(&path, name, expectation, output_type).await?;
    }
    Ok(())
}

async fn run_scenario(
    scenario_dir: &Path,
    scenario_name: &str,
    expectation: ApplyPatchExpectation,
    output_type: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness_with(|builder| builder.with_model("gpt-5.1-codex"))
        .await
        .with_context(|| format!("scenario {scenario_name}: building apply patch harness"))?;

    let input_dir = scenario_dir.join("input");
    if input_dir.is_dir() {
        for entry in WalkDir::new(&input_dir) {
            let entry = entry.with_context(|| {
                format!("scenario {scenario_name}: walking input directory entry",)
            })?;
            if !entry.file_type().is_file() {
                continue;
            }

            let rel = entry.path().strip_prefix(&input_dir).with_context(|| {
                format!(
                    "scenario {scenario_name}: stripping input prefix from {}",
                    entry.path().display()
                )
            })?;
            let dest_path = harness.path(rel);
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "scenario {scenario_name}: creating parent directory {}",
                        parent.display()
                    )
                })?;
            }
            fs::copy(entry.path(), &dest_path).with_context(|| {
                format!(
                    "scenario {scenario_name}: copying input fixture {} -> {}",
                    entry.path().display(),
                    dest_path.display()
                )
            })?;
        }
    }

    let patch_path = scenario_dir.join("patch.txt");
    let patch = fs::read_to_string(&patch_path)
        .with_context(|| format!("scenario {scenario_name}: reading {}", patch_path.display()))?;

    let call_id = format!("apply-patch-{scenario_name}");
    mount_apply_patch(&harness, &call_id, &patch, "done", output_type).await;
    harness
        .submit(&patch)
        .await
        .with_context(|| format!("scenario {scenario_name}: submitting patch"))?;

    let out = harness.apply_patch_output(&call_id, output_type).await;

    assert_tool_call_output_expectation(scenario_name, &out, expectation)?;
    assert_output_directory_expectation(&harness, scenario_name, scenario_dir)
        .await
        .with_context(|| format!("scenario {scenario_name}: asserting output directory",))?;
    Ok(())
}

async fn apply_patch_harness_with(
    configure: impl FnOnce(TestCodexBuilder) -> TestCodexBuilder,
) -> Result<TestCodexHarness> {
    let builder = configure(test_codex()).with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    TestCodexHarness::with_builder(builder).await
}

pub async fn mount_apply_patch(
    harness: &TestCodexHarness,
    call_id: &str,
    patch: &str,
    assistant_msg: &str,
    output_type: ApplyPatchModelOutput,
) {
    mount_sse_sequence(
        harness.server(),
        apply_patch_responses(call_id, patch, assistant_msg, output_type),
    )
    .await;
}

fn apply_patch_responses(
    call_id: &str,
    patch: &str,
    assistant_msg: &str,
    output_type: ApplyPatchModelOutput,
) -> Vec<String> {
    vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_call(call_id, patch, output_type),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", assistant_msg),
            ev_completed("resp-2"),
        ]),
    ]
}

fn assert_tool_call_output_expectation(
    scenario_name: &str,
    output: &str,
    expectation: ApplyPatchExpectation,
) -> Result<()> {
    match expectation {
        ApplyPatchExpectation::Success => {
            let expected = r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
Success. Updated the following files:
.*?$";
            let regex = Regex::new(expected).with_context(|| {
                format!(
                    "scenario {scenario_name}: compiling regex for successful apply_patch output"
                )
            })?;
            assert!(
                regex.is_match(output),
                "scenario {scenario_name}: output did not match expected success pattern {expected:?}\noutput: {output}"
            );
        }
        ApplyPatchExpectation::Failure => {
            let expected = r"^(Failure|apply_patch verification failed|patch rejected)";
            let regex = Regex::new(expected).with_context(|| {
                format!("scenario {scenario_name}: compiling regex for failed apply_patch output")
            })?;
            assert!(
                regex.is_match(output),
                "scenario {scenario_name}: output did not match expected failure pattern {expected:?}\noutput: {output}"
            );
        }
    }
    Ok(())
}

/// Asserts that the harness `actual/`` directory exactly matches expected_output_files. Every file
/// in the actual/ directory must be present in the expected_output_files and the contents must
/// exactly match.
async fn assert_output_directory_expectation(
    harness: &TestCodexHarness,
    scenario_name: &str,
    scenario_dir: &Path,
) -> Result<()> {
    eprintln!("asserting output directory expectation for {scenario_name}");
    let mut actual_files = BTreeMap::new();

    for entry in WalkDir::new(harness.path("")) {
        let entry = entry.with_context(|| {
            format!("scenario {scenario_name}: walking output directory entry",)
        })?;
        if !entry.file_type().is_file() {
            continue;
        }

        let contents = fs::read_to_string(harness.path(entry.path())).with_context(|| {
            format!(
                "scenario {scenario_name}: reading {}",
                entry.path().display()
            )
        })?;

        let relative_path = entry
            .path()
            .strip_prefix(harness.path(""))
            .with_context(|| format!("scenario {scenario_name}: relative path"))?
            .to_string_lossy()
            .to_string();

        actual_files.insert(relative_path, contents);
    }

    if scenario_name == "015_failure_after_partial_success_leaves_changes" {
        assert!(
            actual_files.is_empty(),
            "scenario {scenario_name}: expected no files to be created on failure"
        );
        return Ok(());
    }

    let expected_root = scenario_dir.join("expected");
    let mut expected_files = BTreeMap::new();
    if expected_root.is_dir() {
        for entry in WalkDir::new(&expected_root) {
            let entry = entry.with_context(|| {
                format!("scenario {scenario_name}: walking expected directory entry",)
            })?;
            if !entry.file_type().is_file() {
                continue;
            }

            let contents = fs::read_to_string(entry.path()).with_context(|| {
                format!(
                    "scenario {scenario_name}: reading expected {}",
                    entry.path().display()
                )
            })?;

            let relative_path = entry
                .path()
                .strip_prefix(&expected_root)
                .with_context(|| format!("scenario {scenario_name}: expected relative path"))?
                .to_string_lossy()
                .to_string();

            expected_files.insert(relative_path, contents);
        }
    }

    assert_eq!(
        expected_files, actual_files,
        "scenario {}: expected and actual files do not match",
        scenario_name
    );
    Ok(())
}
