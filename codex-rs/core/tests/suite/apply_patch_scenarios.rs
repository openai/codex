#![allow(clippy::expect_used)]

use anyhow::Context;
use anyhow::Result;
use core_test_support::responses::ev_apply_patch_call;
use core_test_support::test_codex::ApplyPatchModelOutput;
use pretty_assertions::assert_eq;
use regex_lite::Regex;
use std::fs;
use test_case::test_case;

use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use std::collections::BTreeMap;
use walkdir::WalkDir;

const APPLY_PATCH_SCENARIOS: &[ApplyPatchScenario] = &[
    ApplyPatchScenario {
        name: "appends-trailing-newline",
        patch: "*** Begin Patch\n*** Update File: no_newline.txt\n@@\n-no newline at end\n+first line\n+second line\n*** End Patch",
        input_files: &[PatchFile {
            path: "no_newline.txt",
            content: "no newline at end",
        }],
        expectation: ApplyPatchExpectation::Success,
        expected_output_files: &[PatchFile {
            path: "no_newline.txt",
            content: "first line\nsecond line\n",
        }],
    },
    ApplyPatchScenario {
        name: "multiple-chunks",
        patch: "*** Begin Patch\n*** Update File: multi.txt\n@@\n-line2\n+changed2\n@@\n-line4\n+changed4\n*** End Patch",
        input_files: &[PatchFile {
            path: "multi.txt",
            content: "line1\nline2\nline3\nline4\n",
        }],
        expectation: ApplyPatchExpectation::Success,
        expected_output_files: &[PatchFile {
            path: "multi.txt",
            content: "line1\nchanged2\nline3\nchanged4\n",
        }],
    },
    ApplyPatchScenario {
        name: "moves-file-to-new-directory",
        patch: "*** Begin Patch\n*** Update File: old/name.txt\n*** Move to: renamed/dir/name.txt\n@@\n-old content\n+new content\n*** End Patch",
        input_files: &[PatchFile {
            path: "old/name.txt",
            content: "old content\n",
        }],
        expected_output_files: &[PatchFile {
            path: "renamed/dir/name.txt",
            content: "new content\n",
        }],
        expectation: ApplyPatchExpectation::Success,
    },
    ApplyPatchScenario {
        name: "cli-insert-only-hunk-modifies-file",
        patch: "*** Begin Patch\n*** Update File: insert_only.txt\n@@\n alpha\n+beta\n omega\n*** End Patch",
        input_files: &[PatchFile {
            path: "insert_only.txt",
            content: "alpha\nomega\n",
        }],
        expectation: ApplyPatchExpectation::Success,
        expected_output_files: &[PatchFile {
            path: "insert_only.txt",
            content: "alpha\nbeta\nomega\n",
        }],
    },
    ApplyPatchScenario {
        name: "cli-move-overwrites-existing-destination",
        patch: "*** Begin Patch\n*** Update File: old/name.txt\n*** Move to: renamed/dir/name.txt\n@@\n-old content\n+new content\n*** End Patch",
        input_files: &[PatchFile {
            path: "old/name.txt",
            content: "old content\n",
        }],
        expectation: ApplyPatchExpectation::Success,
        expected_output_files: &[PatchFile {
            path: "renamed/dir/name.txt",
            content: "new content\n",
        }],
    },
    ApplyPatchScenario {
        name: "cli-add-overwrites-existing-file",
        patch: "*** Begin Patch\n*** Add File: duplicate.txt\n+new content\n*** End Patch",
        input_files: &[PatchFile {
            path: "duplicate.txt",
            content: "old content\n",
        }],
        expectation: ApplyPatchExpectation::Success,
        expected_output_files: &[PatchFile {
            path: "duplicate.txt",
            content: "new content\n",
        }],
    },
    ApplyPatchScenario {
        name: "cli-rejects-invalid-hunk-header",
        patch: "*** Begin Patch\n*** Frobnicate File: foo\n*** End Patch",
        input_files: &[PatchFile {
            path: "foo.txt",
            content: "old content\n",
        }],
        expectation: ApplyPatchExpectation::Failure,
        expected_output_files: &[PatchFile {
            path: "foo.txt",
            content: "old content\n",
        }],
    },
    ApplyPatchScenario {
        name: "cli-reports-missing-context",
        patch: "*** Begin Patch\n*** Update File: modify.txt\n@@\n-missing\n+changed\n*** End Patch",
        input_files: &[PatchFile {
            path: "modify.txt",
            content: "line1\nline2\n",
        }],
        expectation: ApplyPatchExpectation::Failure,
        expected_output_files: &[PatchFile {
            path: "modify.txt",
            content: "line1\nline2\n",
        }],
    },
    ApplyPatchScenario {
        name: "cli-reports-missing-target-file",
        patch: "*** Begin Patch\n*** Update File: missing.txt\n@@\n-old\n+new\n*** End Patch",
        input_files: &[PatchFile {
            path: "modify.txt",
            content: "line1\nline2\n",
        }],
        expectation: ApplyPatchExpectation::Failure,
        expected_output_files: &[PatchFile {
            path: "modify.txt",
            content: "line1\nline2\n",
        }],
    },
    ApplyPatchScenario {
        name: "cli-rejects-empty-patch",
        patch: "*** Begin Patch\n*** End Patch",
        input_files: &[],
        expectation: ApplyPatchExpectation::Failure,
        expected_output_files: &[],
    },
    ApplyPatchScenario {
        name: "cli-rejects-empty-update-hunk",
        patch: "*** Begin Patch\n*** Update File: foo.txt\n*** End Patch",
        input_files: &[PatchFile {
            path: "foo.txt",
            content: "old content\n",
        }],
        expectation: ApplyPatchExpectation::Failure,
        expected_output_files: &[PatchFile {
            path: "foo.txt",
            content: "old content\n",
        }],
    },
    ApplyPatchScenario {
        name: "cli-requires-existing-file-for-update",
        patch: "*** Begin Patch\n*** Update File: missing.txt\n@@\n-old\n+new\n*** End Patch",
        input_files: &[],
        expectation: ApplyPatchExpectation::Failure,
        expected_output_files: &[],
    },
    ApplyPatchScenario {
        name: "cli-move-overwrites-existing-destination",
        patch: "*** Begin Patch\n*** Update File: old/name.txt\n*** Move to: renamed/dir/name.txt\n@@\n-old content\n+new content\n*** End Patch",
        input_files: &[PatchFile {
            path: "old/name.txt",
            content: "old content\n",
        }],
        expectation: ApplyPatchExpectation::Success,
        expected_output_files: &[PatchFile {
            path: "renamed/dir/name.txt",
            content: "new content\n",
        }],
    },
];

#[derive(Clone, Copy)]
enum ApplyPatchExpectation {
    Success,
    Failure,
}

#[derive(Clone, Copy)]
struct PatchFile {
    path: &'static str,
    content: &'static str,
}

struct ApplyPatchScenario {
    name: &'static str,
    patch: &'static str,
    input_files: &'static [PatchFile],
    expectation: ApplyPatchExpectation,
    expected_output_files: &'static [PatchFile],
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
async fn test_apply_patch_scenario(output_type: ApplyPatchModelOutput) -> Result<()> {
    for scenario in APPLY_PATCH_SCENARIOS {
        run_scenario(scenario, output_type).await?;
    }
    Ok(())
}

async fn run_scenario(
    scenario: &ApplyPatchScenario,
    output_type: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness_with(|builder| builder.with_model("gpt-5.1-codex"))
        .await
        .with_context(|| format!("scenario {}: building apply patch harness", scenario.name))?;

    for PatchFile { path, content } in scenario.input_files {
        let input_path = harness.path(path);
        fs::create_dir_all(
            input_path.parent().unwrap_or_else(|| {
                panic!("scenario {}: parent directory of {path}", scenario.name)
            }),
        )?;

        fs::write(input_path, content)
            .with_context(|| format!("scenario {}: writing input fixture {path}", scenario.name))?;
    }

    let call_id = format!("apply-patch-{}", scenario.name);
    mount_apply_patch(&harness, &call_id, scenario.patch, "done", output_type).await;
    harness
        .submit(scenario.patch)
        .await
        .with_context(|| format!("scenario {}: submitting patch", scenario.name))?;

    let out = harness.apply_patch_output(&call_id, output_type).await;

    assert_tool_call_output_expectation(scenario.name, &out, scenario.expectation)?;
    assert_output_directory_expectation(&harness, scenario)
        .await
        .with_context(|| format!("scenario {}: asserting output directory", scenario.name))?;
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
    scenario: &ApplyPatchScenario,
) -> Result<()> {
    eprintln!(
        "asserting output directory expectation for {}",
        scenario.name
    );
    let mut actual_files = BTreeMap::new();

    for entry in WalkDir::new(harness.path("")) {
        let entry = entry.with_context(|| {
            format!("scenario {}: walking output directory entry", scenario.name)
        })?;
        if !entry.file_type().is_file() {
            continue;
        }

        let contents = fs::read_to_string(harness.path(entry.path())).with_context(|| {
            format!(
                "scenario {}: reading {}",
                scenario.name,
                entry.path().display()
            )
        })?;

        let relative_path = entry
            .path()
            .strip_prefix(harness.path(""))
            .unwrap_or_else(|_| panic!("scenario {}: relative path", scenario.name))
            .to_string_lossy()
            .to_string();

        actual_files.insert(relative_path, contents);
    }

    let expected_files = scenario
        .expected_output_files
        .iter()
        .map(|PatchFile { path, content }| ((*path).to_string(), (*content).to_string()))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(
        expected_files, actual_files,
        "scenario {}: expected and actual files do not match",
        scenario.name
    );
    Ok(())
}
