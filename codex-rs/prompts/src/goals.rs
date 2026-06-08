use codex_protocol::protocol::ThreadGoal;
use codex_utils_template::Template;
use std::sync::LazyLock;

static CONTINUATION_PROMPT_TEMPLATE: LazyLock<Template> =
    LazyLock::new(
        || match Template::parse(include_str!("../templates/goals/continuation.md")) {
            Ok(template) => template,
            Err(err) => panic!("embedded goals/continuation.md template is invalid: {err}"),
        },
    );

static BUDGET_LIMIT_PROMPT_TEMPLATE: LazyLock<Template> =
    LazyLock::new(
        || match Template::parse(include_str!("../templates/goals/budget_limit.md")) {
            Ok(template) => template,
            Err(err) => panic!("embedded goals/budget_limit.md template is invalid: {err}"),
        },
    );

static OBJECTIVE_UPDATED_PROMPT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    match Template::parse(include_str!("../templates/goals/objective_updated.md")) {
        Ok(template) => template,
        Err(err) => {
            panic!("embedded goals/objective_updated.md template is invalid: {err}")
        }
    }
});

/// Builds the hidden prompt used to continue an active goal after the previous
/// turn completes.
pub fn continuation_prompt(goal: &ThreadGoal) -> String {
    let token_budget = goal
        .token_budget
        .map(|budget| budget.to_string())
        .unwrap_or_else(|| "none".to_string());
    let remaining_tokens = goal
        .token_budget
        .map(|budget| (budget - goal.tokens_used).max(0).to_string())
        .unwrap_or_else(|| "unbounded".to_string());
    let tokens_used = goal.tokens_used.to_string();
    let objective = escape_xml_text(&goal.objective);

    match CONTINUATION_PROMPT_TEMPLATE.render([
        ("objective", objective.as_str()),
        ("tokens_used", tokens_used.as_str()),
        ("token_budget", token_budget.as_str()),
        ("remaining_tokens", remaining_tokens.as_str()),
    ]) {
        Ok(prompt) => prompt,
        Err(err) => panic!("embedded goals/continuation.md template failed to render: {err}"),
    }
}

/// Builds the hidden prompt used to ask the model to wrap up after a goal
/// exhausts its budget.
pub fn budget_limit_prompt(goal: &ThreadGoal) -> String {
    let token_budget = goal
        .token_budget
        .map(|budget| budget.to_string())
        .unwrap_or_else(|| "none".to_string());
    let tokens_used = goal.tokens_used.to_string();
    let time_used_seconds = goal.time_used_seconds.to_string();
    let objective = escape_xml_text(&goal.objective);

    match BUDGET_LIMIT_PROMPT_TEMPLATE.render([
        ("objective", objective.as_str()),
        ("tokens_used", tokens_used.as_str()),
        ("time_used_seconds", time_used_seconds.as_str()),
        ("token_budget", token_budget.as_str()),
    ]) {
        Ok(prompt) => prompt,
        Err(err) => panic!("embedded goals/budget_limit.md template failed to render: {err}"),
    }
}

/// Builds the hidden prompt used after a user edits an active goal.
pub fn objective_updated_prompt(goal: &ThreadGoal) -> String {
    let token_budget = goal
        .token_budget
        .map(|budget| budget.to_string())
        .unwrap_or_else(|| "none".to_string());
    let remaining_tokens = goal
        .token_budget
        .map(|budget| (budget - goal.tokens_used).max(0).to_string())
        .unwrap_or_else(|| "unbounded".to_string());
    let tokens_used = goal.tokens_used.to_string();
    let objective = escape_xml_text(&goal.objective);

    match OBJECTIVE_UPDATED_PROMPT_TEMPLATE.render([
        ("objective", objective.as_str()),
        ("tokens_used", tokens_used.as_str()),
        ("token_budget", token_budget.as_str()),
        ("remaining_tokens", remaining_tokens.as_str()),
    ]) {
        Ok(prompt) => prompt,
        Err(err) => panic!("embedded goals/objective_updated.md template failed to render: {err}"),
    }
}

fn escape_xml_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
#[path = "goals_tests.rs"]
mod goals_tests;
