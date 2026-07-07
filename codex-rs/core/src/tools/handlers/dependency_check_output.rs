use crate::tools::context::FunctionToolOutput;
use codex_dependency_check::DependencyCheckRequest;
use codex_dependency_check::DependencyPolicyAction;
use codex_dependency_check::DependencyPolicyReport;
use codex_dependency_check::DependencyRisk;
use codex_dependency_check::DependencyRiskKind;
use codex_dependency_check::NpmGraphMismatch;
use codex_dependency_check::NpmInstalledGraphMismatch;
use codex_protocol::exec_output::ExecToolCallOutput;

pub(super) fn blocked_output(message: String) -> FunctionToolOutput {
    FunctionToolOutput::from_text(message, Some(false))
}

pub(super) fn command_failure_output(
    stage: &str,
    output: &ExecToolCallOutput,
) -> FunctionToolOutput {
    blocked_output(command_failure_message(stage, output))
}

pub(super) fn command_failure_message(stage: &str, output: &ExecToolCallOutput) -> String {
    format!(
        "Dependency Check stopped during {stage} with exit code {}. Lifecycle scripts were not enabled.\n{}",
        output.exit_code,
        output.aggregated_output.text.trim()
    )
}

pub(super) fn graph_mismatch_output(
    stage: &str,
    mismatch: &NpmGraphMismatch,
) -> FunctionToolOutput {
    blocked_output(format!(
        "Dependency Check stopped after {stage} and before lifecycle scripts because npm produced a different graph than the one checked by OSV: {mismatch}."
    ))
}

pub(super) fn installed_graph_mismatch_output(
    mismatch: &NpmInstalledGraphMismatch,
) -> FunctionToolOutput {
    blocked_output(format!(
        "Dependency Check stopped after the script-disabled install and before lifecycle scripts because npm installed a different artifact graph than the one checked by OSV: {mismatch}."
    ))
}

pub(super) fn format_blocked_policy(report: &DependencyPolicyReport) -> String {
    format!(
        "Dependency Check blocked before modifying the project or running package code. OSV reported malware in the resolved graph:\n{}",
        format_risks(&report.risks)
    )
}

pub(super) fn format_success(
    request: &DependencyCheckRequest,
    report: &DependencyPolicyReport,
    graph_packages: usize,
) -> String {
    let requested = request
        .dependencies
        .iter()
        .map(codex_dependency_check::DependencySpec::npm_specifier)
        .collect::<Vec<_>>()
        .join(", ");
    let warnings = if report.action == DependencyPolicyAction::Warn {
        format!(
            " OSV reported non-malware advisories that did not block the install:\n{}",
            format_risks(&report.risks)
        )
    } else {
        String::new()
    };
    format!(
        "Dependency Check installed {requested}. It checked {} unique package coordinates across {graph_packages} resolved artifacts, verified the project lock graph, matched the clean script-disabled install to the checked artifact graph, and completed npm rebuild.{warnings}",
        report.checked_packages
    )
}

fn format_risks(risks: &[DependencyRisk]) -> String {
    let mut lines = risks
        .iter()
        .take(20)
        .map(|risk| {
            let kind = match risk.kind {
                DependencyRiskKind::Malware => "malware",
                DependencyRiskKind::Vulnerability => "vulnerability",
            };
            match &risk.summary {
                Some(summary) => format!(
                    "- npm:{}@{} | {} | {}: {}",
                    risk.package_name, risk.package_version, risk.advisory_id, kind, summary
                ),
                None => format!(
                    "- npm:{}@{} | {} | {}",
                    risk.package_name, risk.package_version, risk.advisory_id, kind
                ),
            }
        })
        .collect::<Vec<_>>();
    if risks.len() > lines.len() {
        lines.push(format!(
            "- ... and {} more advisories",
            risks.len() - lines.len()
        ));
    }
    lines.join("\n")
}
