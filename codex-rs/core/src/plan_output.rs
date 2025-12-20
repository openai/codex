use codex_protocol::plan_tool::StepStatus;
use codex_protocol::protocol::PlanOutputEvent;

pub(crate) fn render_approved_plan_body(out: &PlanOutputEvent) -> String {
    let mut body = String::new();
    let title = out.title.trim();
    body.push_str(&format!("Title: {title}\n"));
    let summary = out.summary.trim();
    if !summary.is_empty() {
        body.push_str(&format!("Summary: {summary}\n"));
    }
    let explanation = out.plan.explanation.as_deref().unwrap_or_default().trim();
    if !explanation.is_empty() {
        body.push_str("Explanation:\n");
        body.push_str(explanation);
        body.push('\n');
    }
    body.push_str("Steps:\n");
    if out.plan.plan.is_empty() {
        body.push_str("- (no steps provided)\n");
    } else {
        for item in &out.plan.plan {
            let status = step_status_label(&item.status);
            let step = item.step.trim();
            body.push_str(&format!("- [{status}] {step}\n"));
        }
    }
    body
}

pub(crate) fn render_approved_plan_markdown(out: &PlanOutputEvent) -> String {
    let mut markdown = String::new();
    let title = out.title.trim();
    markdown.push_str(&format!("# {title}\n\n"));

    let summary = out.summary.trim();
    if !summary.is_empty() {
        markdown.push_str(&format!("{summary}\n\n"));
    }

    let explanation = out.plan.explanation.as_deref().unwrap_or_default().trim();
    if !explanation.is_empty() {
        markdown.push_str("## Explanation\n");
        markdown.push_str(explanation);
        markdown.push_str("\n\n");
    }

    markdown.push_str("## Steps\n");
    if out.plan.plan.is_empty() {
        markdown.push_str("- (no steps provided)\n");
    } else {
        for item in &out.plan.plan {
            let status = step_status_label(&item.status);
            let step = item.step.trim();
            markdown.push_str(&format!("- [{status}] {step}\n"));
        }
    }

    markdown
}

pub(crate) fn render_approved_plan_transcript(out: &PlanOutputEvent) -> String {
    let body = render_approved_plan_body(out);
    format!("Approved plan:\n{body}")
}

pub(crate) fn render_approved_plan_developer_prelude(out: &PlanOutputEvent) -> String {
    let body = render_approved_plan_body(out);
    format!("## Approved Plan (Pinned)\nExecute the approved plan below.\n\n{body}")
}

fn step_status_label(status: &StepStatus) -> &'static str {
    match status {
        StepStatus::Pending => "pending",
        StepStatus::InProgress => "in_progress",
        StepStatus::Completed => "completed",
    }
}
