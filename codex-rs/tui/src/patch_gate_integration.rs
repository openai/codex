// src/patch_gate_integration.rs
//
// Integration layer between the Autopilot (Builder + Reviewer) and the PatchGate.
// It parses the Builder's Diff Envelope, runs verification/apply in a blocking
// task (spawn_blocking), and publishes concise badges to the TUI.
//
// This module does not persist reports itself — return value (ApplyReport)
// allows the caller to persist via RolloutRecorder.
//
// Author: Platform Architecture
// License: Apache-2.0

use std::path::Path;

use color_eyre::eyre::{Result, WrapErr};
use tokio::task;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::history_cell;

use crate::git_guard::{
    verify_and_apply_patch, ApplyReport, ChangeContract, WorktreePolicy,
};
use crate::patch_mode::enforce_diff_envelope_or_err;
use crate::metrics::{inc_rejections, get_rejections, set_apply_millis, get_ci_runs, get_apply_millis, Reason, Phase};

/// Run PatchGate for a Builder raw output that is expected to contain a Diff Envelope.
/// Emits a compact banner at start and a result badge at the end.
/// The `build_and_test` closure is optional; when provided and `require_tests=true`,
/// it will run both pre- and post-apply.
///
/// `commit_subject`: short, imperative sentence used in Conventional Commit subject.
#[allow(dead_code)]
pub(crate) async fn run_patch_gate_for_builder_output<F>(
    app_event_tx: AppEventSender,
    repo_path: &Path,
    builder_raw_output: &str,
    contract: &ChangeContract,
    commit_subject: &str,
    check_only: bool,
    build_and_test: Option<F>,
) -> Result<ApplyReport>
where
    F: Fn(&Path) -> color_eyre::eyre::Result<()> + Send + Sync + 'static,
{
    // 1) Parse & enforce envelope-only output
    let envelope = match enforce_diff_envelope_or_err(builder_raw_output) {
        Ok(env) => env,
        Err(err) => {
            // Surface error to TUI and propagate
            use ratatui::style::Stylize as _;
            // bump telemetry counter (P2-02 minimal hook)
            inc_rejections(Reason::Enforcement);
            let mut lines: Vec<ratatui::text::Line<'static>> = Vec::new();
            lines.push(ratatui::text::Line::from(vec![
                "🧩\u{200A}".into(),
                "PatchGate:".bold(),
                " ".into(),
                "REJECTED".red().bold(),
            ]));
            lines.push(ratatui::text::Line::from(format!(
                "invalid Builder output: {err}"
            )));
            lines.push(ratatui::text::Line::from(
                "hint: output must be a single Diff Envelope (see docs)",
            ));
            app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                history_cell::new_info_note(lines),
            )));
            return Err(err.wrap_err("failed to enforce builder diff envelope"));
        }
    };

    // 2) Banner: PatchGate running…
    {
        use ratatui::style::Stylize as _;
        let mut lines: Vec<ratatui::text::Line<'static>> = Vec::new();
        lines.push(ratatui::text::Line::from(vec![
            "🧩\u{200A}".into(),
            "PatchGate".bold().cyan(),
            " ".into(),
            "running…".dim(),
        ]));
        lines.push(
            ratatui::text::Line::from(
                format!(
                    "task={} • base_ref={} • budgets: files≤{:?} +≤{:?} −≤{:?}",
                    contract.task_id,
                    envelope.base_ref,
                    contract.max_files_changed,
                    contract.max_lines_added,
                    contract.max_lines_removed
                )
                .dim(),
            ),
        );
        // Show permissive/strict mode (record-only) if toggled.
        if crate::autopilot_prefs::patchgate_permissive() {
            use ratatui::style::Stylize as _;
            lines.push(ratatui::text::Line::from("mode=permissive".to_string().dim()));
        }
        app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
            history_cell::new_info_note(lines),
        )));
    }

    // 3) Execute blocking PatchGate in a dedicated thread.
    let repo = repo_path.to_path_buf();
    let contract_owned = contract.clone();
    let subject = commit_subject.to_string();

    let started = std::time::Instant::now();
    let report: ApplyReport = task::spawn_blocking(move || {
        let policy = if check_only {
            // Even for dry-run, prefer ephemeral to validate against base_ref;
            // will be cleaned up automatically.
            WorktreePolicy::EphemeralFromBaseRef {
                base_ref: envelope.base_ref.clone(),
                task_id: contract_owned.task_id.clone(),
            }
        } else {
            WorktreePolicy::EphemeralFromBaseRef {
                base_ref: envelope.base_ref.clone(),
                task_id: contract_owned.task_id.clone(),
            }
        };

        verify_and_apply_patch(
            &repo,
            &envelope,
            &contract_owned,
            &subject,
            check_only,
            policy,
            build_and_test,
        )
    })
    .await
    .wrap_err("PatchGate task join failure")??;
    let elapsed = started.elapsed();
    set_apply_millis(elapsed.as_millis() as u64);

    // 4) Publish result badge to TUI
    {
        use ratatui::style::Stylize as _;
        let status = if !report.contract_violations.is_empty() {
            "REJECTED".red().bold()
        } else if report.applied && report.committed {
            "COMMITTED".green().bold()
        } else if report.checked_ok {
            "DRY‑RUN OK".cyan().bold()
        } else {
            "UNKNOWN".dim()
        };

        let mut lines: Vec<ratatui::text::Line<'static>> = Vec::new();
        lines.push(ratatui::text::Line::from(vec![
            "🧩\u{200A}".into(),
            "PatchGate:".bold(),
            " ".into(),
            status,
        ]));

        // Stats
        let s = &report.stats;
        lines.push(
            ratatui::text::Line::from(
                format!("files={} +{} −{} • paths: {}", s.files_changed, s.lines_added, s.lines_removed, s.touched_paths.len())
                    .dim(),
            ),
        );

        // Violations (first 3 to keep concise)
        if !report.contract_violations.is_empty() {
            lines.push("violations:".bold().red().into());
            for v in report.contract_violations.iter().take(3) {
                lines.push(format!("- {v}").into());
            }
            if report.contract_violations.len() > 3 {
                lines.push(
                    format!("… and {} more", report.contract_violations.len() - 3)
                        .dim()
                        .into(),
                );
            }
        }

        // Commit SHA if any
        if let Some(sha) = &report.commit_sha {
            lines.push(format!("commit: {sha}").dim().into());
        }

        // Minimal metrics badge (P2-02)
        let rej = get_rejections(Reason::Enforcement);
        let pre = get_ci_runs(Phase::Pre);
        let post = get_ci_runs(Phase::Post);
        let ms = get_apply_millis();
        lines.push(
            format!(
                "rejections_total{{enforcement}}={} • patchgate_apply_seconds={} • ci_runs_total{{pre}}={} post={}",
                rej,
                (ms as f64) / 1000.0,
                pre,
                post
            )
            .dim()
            .into(),
        );

        app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
            history_cell::new_info_note(lines),
        )));
    }

    Ok(report)
}
