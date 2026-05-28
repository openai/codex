use codex_extension_api::HiddenContext;
use codex_extension_api::HiddenContextMarker;
use codex_extension_api::HiddenContextMarkerRegistration;
use codex_extension_api::ThreadIdleRequest;
use codex_protocol::protocol::ThreadGoal;
use codex_utils_template::Template;
use std::sync::LazyLock;

const GOAL_CONTEXT_MARKER: HiddenContextMarker =
    HiddenContextMarker::new("<goal_context>", "</goal_context>");

inventory::submit! {
    HiddenContextMarkerRegistration {
        marker: GOAL_CONTEXT_MARKER,
    }
}

static CONTINUATION_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        include_str!("../templates/goals/continuation.md"),
        "goals/continuation.md",
    )
});

static BUDGET_LIMIT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        include_str!("../templates/goals/budget_limit.md"),
        "goals/budget_limit.md",
    )
});

static OBJECTIVE_UPDATED_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        include_str!("../templates/goals/objective_updated.md"),
        "goals/objective_updated.md",
    )
});

fn parse_embedded_template(source: &'static str, template_name: &str) -> Template {
    match Template::parse(source.trim_end()) {
        Ok(template) => template,
        Err(err) => panic!("embedded template {template_name} is invalid: {err}"),
    }
}

fn render_embedded_template<const N: usize>(
    template: &Template,
    template_name: &str,
    variables: [(&str, &str); N],
) -> String {
    match template.render(variables) {
        Ok(rendered) => rendered,
        Err(err) => panic!("embedded template {template_name} values are invalid: {err}"),
    }
}

pub(crate) fn budget_limit_steering_context(goal: &ThreadGoal) -> HiddenContext {
    goal_context(budget_limit_prompt(goal))
}

pub(crate) fn continuation_steering_request(goal: &ThreadGoal) -> ThreadIdleRequest {
    ThreadIdleRequest::new(goal_context(continuation_prompt(goal)))
}

pub(crate) fn objective_updated_steering_context(goal: &ThreadGoal) -> HiddenContext {
    goal_context(objective_updated_prompt(goal))
}

fn goal_context(prompt: String) -> HiddenContext {
    HiddenContext::new(GOAL_CONTEXT_MARKER, prompt)
}

fn continuation_prompt(goal: &ThreadGoal) -> String {
    let objective = escape_xml_text(&goal.objective);
    let tokens_used = goal.tokens_used.to_string();
    let (token_budget, remaining_tokens) = match goal.token_budget {
        Some(token_budget) => (
            token_budget.to_string(),
            (token_budget - goal.tokens_used).max(0).to_string(),
        ),
        None => ("none".to_string(), "unknown".to_string()),
    };

    render_embedded_template(
        &CONTINUATION_TEMPLATE,
        "goals/continuation.md",
        [
            ("objective", objective.as_str()),
            ("tokens_used", tokens_used.as_str()),
            ("token_budget", token_budget.as_str()),
            ("remaining_tokens", remaining_tokens.as_str()),
        ],
    )
}

fn budget_limit_prompt(goal: &ThreadGoal) -> String {
    let objective = escape_xml_text(&goal.objective);
    let time_used_seconds = goal.time_used_seconds.to_string();
    let tokens_used = goal.tokens_used.to_string();
    let token_budget = goal
        .token_budget
        .map(|budget| budget.to_string())
        .unwrap_or_else(|| "none".to_string());

    render_embedded_template(
        &BUDGET_LIMIT_TEMPLATE,
        "goals/budget_limit.md",
        [
            ("objective", objective.as_str()),
            ("time_used_seconds", time_used_seconds.as_str()),
            ("tokens_used", tokens_used.as_str()),
            ("token_budget", token_budget.as_str()),
        ],
    )
}

fn objective_updated_prompt(goal: &ThreadGoal) -> String {
    let objective = escape_xml_text(&goal.objective);
    let tokens_used = goal.tokens_used.to_string();
    let (token_budget, remaining_tokens) = match goal.token_budget {
        Some(token_budget) => (
            token_budget.to_string(),
            (token_budget - goal.tokens_used).max(0).to_string(),
        ),
        None => ("none".to_string(), "unknown".to_string()),
    };

    render_embedded_template(
        &OBJECTIVE_UPDATED_TEMPLATE,
        "goals/objective_updated.md",
        [
            ("objective", objective.as_str()),
            ("tokens_used", tokens_used.as_str()),
            ("token_budget", token_budget.as_str()),
            ("remaining_tokens", remaining_tokens.as_str()),
        ],
    )
}

fn escape_xml_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
