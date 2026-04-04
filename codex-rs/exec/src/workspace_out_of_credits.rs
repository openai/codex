use codex_app_server_protocol::CodexErrorInfo;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::TurnError;
use codex_backend_client::Client as BackendClient;
use codex_backend_client::WorkspaceOutOfCreditsNotificationStatus;
use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::config::Config;
use codex_protocol::account::PlanType;
use codex_protocol::protocol::RateLimitSnapshot;
use std::io;
use std::io::IsTerminal;

const REQUEST_PROMPT: &str =
    "Workspace out of credits. Request more from your workspace owner? [y/N]";
const REQUEST_SENT_MESSAGE: &str = "Request sent!";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PromptDecisionInput {
    candidate_usage_limit_failure: bool,
    human_output: bool,
    stdin_is_tty: bool,
    stderr_is_tty: bool,
    auth_supports_request: bool,
}

pub(crate) fn notification_is_usage_limit_failure(
    notification: &ServerNotification,
    thread_id: &str,
    turn_id: &str,
) -> bool {
    match notification {
        ServerNotification::Error(payload) => {
            payload.thread_id == thread_id
                && payload.turn_id == turn_id
                && !payload.will_retry
                && turn_error_is_usage_limit(&payload.error)
        }
        ServerNotification::TurnCompleted(payload) => {
            payload.thread_id == thread_id
                && payload.turn.id == turn_id
                && payload.turn.status == codex_app_server_protocol::TurnStatus::Failed
                && payload
                    .turn
                    .error
                    .as_ref()
                    .is_some_and(turn_error_is_usage_limit)
        }
        _ => false,
    }
}

pub(crate) async fn maybe_prompt_for_workspace_out_of_credits(
    config: &Config,
    candidate_usage_limit_failure: bool,
    human_output: bool,
) -> anyhow::Result<()> {
    let stdin_is_tty = io::stdin().is_terminal();
    let stderr_is_tty = io::stderr().is_terminal();
    let initial_decision = PromptDecisionInput {
        candidate_usage_limit_failure,
        human_output,
        stdin_is_tty,
        stderr_is_tty,
        auth_supports_request: false,
    };
    if !terminal_prompt_prerequisites_met(initial_decision) {
        return Ok(());
    }

    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        /*enable_codex_api_key_env*/ true,
        config.cli_auth_credentials_store_mode,
    );
    let auth = auth_manager.auth().await;
    let auth_supports_request = auth_supports_workspace_request(auth.as_ref());
    let decision = PromptDecisionInput {
        auth_supports_request,
        ..initial_decision
    };
    if !decision.auth_supports_request {
        return Ok(());
    }

    let Some(auth) = auth else {
        return Ok(());
    };
    let client = BackendClient::from_auth(config.chatgpt_base_url.clone(), &auth)?;
    let rate_limits = client.get_rate_limits().await?;
    if !should_offer_workspace_out_of_credits_prompt(decision, Some(&rate_limits)) {
        return Ok(());
    }

    if !prompt_for_workspace_request()? {
        return Ok(());
    }

    let response = client.post_workspace_out_of_credits_notification().await?;
    match response.status {
        WorkspaceOutOfCreditsNotificationStatus::Sent
        | WorkspaceOutOfCreditsNotificationStatus::CooldownActive => {
            eprintln!("{REQUEST_SENT_MESSAGE}");
        }
    }
    Ok(())
}

fn turn_error_is_usage_limit(error: &TurnError) -> bool {
    matches!(
        error.codex_error_info,
        Some(CodexErrorInfo::UsageLimitExceeded)
    )
}

fn terminal_prompt_prerequisites_met(input: PromptDecisionInput) -> bool {
    input.candidate_usage_limit_failure
        && input.human_output
        && input.stdin_is_tty
        && input.stderr_is_tty
}

fn auth_supports_workspace_request(auth: Option<&CodexAuth>) -> bool {
    auth.is_some_and(|auth| auth.is_chatgpt_auth() && auth.get_account_id().is_some())
}

fn should_offer_workspace_out_of_credits_prompt(
    input: PromptDecisionInput,
    snapshot: Option<&RateLimitSnapshot>,
) -> bool {
    terminal_prompt_prerequisites_met(input)
        && input.auth_supports_request
        && snapshot.is_some_and(rate_limits_confirm_workspace_out_of_credits)
}

fn rate_limits_confirm_workspace_out_of_credits(snapshot: &RateLimitSnapshot) -> bool {
    if snapshot.plan_type != Some(PlanType::SelfServeBusinessUsageBased) {
        return false;
    }

    let Some(credits) = snapshot.credits.as_ref() else {
        return false;
    };
    if credits.unlimited {
        return false;
    }

    credits
        .balance
        .as_deref()
        .and_then(parse_credit_balance)
        .is_some_and(|balance| balance <= 0.0)
}

fn parse_credit_balance(balance: &str) -> Option<f64> {
    balance.trim().parse::<f64>().ok()
}

fn prompt_for_workspace_request() -> io::Result<bool> {
    eprintln!("{REQUEST_PROMPT}");

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(parse_affirmative_response(&input))
}

fn parse_affirmative_response(input: &str) -> bool {
    let answer = input.trim();
    answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::ErrorNotification;
    use codex_app_server_protocol::Turn;
    use codex_app_server_protocol::TurnCompletedNotification;
    use codex_app_server_protocol::TurnStatus;
    use codex_protocol::protocol::CreditsSnapshot;
    use pretty_assertions::assert_eq;

    fn zero_credit_snapshot() -> RateLimitSnapshot {
        RateLimitSnapshot {
            limit_id: Some("codex".to_string()),
            limit_name: None,
            primary: None,
            secondary: None,
            credits: Some(CreditsSnapshot {
                has_credits: false,
                unlimited: false,
                balance: Some("0".to_string()),
            }),
            plan_type: Some(PlanType::SelfServeBusinessUsageBased),
        }
    }

    fn usage_limit_turn_error() -> TurnError {
        TurnError {
            message: "You've hit your usage limit.".to_string(),
            codex_error_info: Some(CodexErrorInfo::UsageLimitExceeded),
            additional_details: None,
        }
    }

    #[test]
    fn error_notification_marks_usage_limit_failure() {
        let notification = ServerNotification::Error(ErrorNotification {
            error: usage_limit_turn_error(),
            will_retry: false,
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
        });

        assert!(notification_is_usage_limit_failure(
            &notification,
            "thread-1",
            "turn-1"
        ));
    }

    #[test]
    fn unrelated_notification_does_not_mark_usage_limit_failure() {
        let notification = ServerNotification::Error(ErrorNotification {
            error: TurnError {
                message: "network failed".to_string(),
                codex_error_info: Some(CodexErrorInfo::ResponseStreamDisconnected {
                    http_status_code: Some(503),
                }),
                additional_details: None,
            },
            will_retry: false,
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
        });

        assert!(!notification_is_usage_limit_failure(
            &notification,
            "thread-1",
            "turn-1"
        ));
    }

    #[test]
    fn failed_turn_marks_usage_limit_failure() {
        let notification = ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: Turn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: TurnStatus::Failed,
                error: Some(usage_limit_turn_error()),
            },
        });

        assert!(notification_is_usage_limit_failure(
            &notification,
            "thread-1",
            "turn-1"
        ));
    }

    #[test]
    fn prompt_decision_requires_ttys_and_human_output() {
        let snapshot = zero_credit_snapshot();
        let input = PromptDecisionInput {
            candidate_usage_limit_failure: true,
            human_output: false,
            stdin_is_tty: true,
            stderr_is_tty: true,
            auth_supports_request: true,
        };

        assert!(!should_offer_workspace_out_of_credits_prompt(
            input,
            Some(&snapshot)
        ));
    }

    #[test]
    fn prompt_decision_requires_supported_auth() {
        let snapshot = zero_credit_snapshot();
        let input = PromptDecisionInput {
            candidate_usage_limit_failure: true,
            human_output: true,
            stdin_is_tty: true,
            stderr_is_tty: true,
            auth_supports_request: false,
        };

        assert!(!should_offer_workspace_out_of_credits_prompt(
            input,
            Some(&snapshot)
        ));
    }

    #[test]
    fn workspace_out_of_credits_confirmation_requires_zero_balance() {
        assert!(rate_limits_confirm_workspace_out_of_credits(
            &zero_credit_snapshot()
        ));
    }

    #[test]
    fn workspace_out_of_credits_confirmation_rejects_missing_balance() {
        let mut snapshot = zero_credit_snapshot();
        snapshot.credits.as_mut().expect("credits").balance = None;

        assert!(!rate_limits_confirm_workspace_out_of_credits(&snapshot));
    }

    #[test]
    fn workspace_out_of_credits_confirmation_rejects_positive_balance() {
        let mut snapshot = zero_credit_snapshot();
        snapshot.credits.as_mut().expect("credits").balance = Some("12.5".to_string());

        assert!(!rate_limits_confirm_workspace_out_of_credits(&snapshot));
    }

    #[test]
    fn workspace_out_of_credits_confirmation_rejects_wrong_plan() {
        let mut snapshot = zero_credit_snapshot();
        snapshot.plan_type = Some(PlanType::Team);

        assert!(!rate_limits_confirm_workspace_out_of_credits(&snapshot));
    }

    #[test]
    fn workspace_out_of_credits_confirmation_rejects_unlimited_credits() {
        let mut snapshot = zero_credit_snapshot();
        snapshot.credits = Some(CreditsSnapshot {
            has_credits: true,
            unlimited: true,
            balance: Some("0".to_string()),
        });

        assert!(!rate_limits_confirm_workspace_out_of_credits(&snapshot));
    }

    #[test]
    fn chatgpt_auth_with_account_id_is_supported() {
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

        assert!(auth_supports_workspace_request(Some(&auth)));
    }

    #[test]
    fn api_key_auth_is_not_supported() {
        let auth = CodexAuth::from_api_key("test-key");

        assert!(!auth_supports_workspace_request(Some(&auth)));
    }

    #[test]
    fn affirmative_response_accepts_y_and_yes() {
        assert!(parse_affirmative_response("y"));
        assert!(parse_affirmative_response("YES"));
        assert!(!parse_affirmative_response(""));
        assert!(!parse_affirmative_response("n"));
    }

    #[test]
    fn prompt_decision_accepts_usage_limit_zero_credit_case() {
        let input = PromptDecisionInput {
            candidate_usage_limit_failure: true,
            human_output: true,
            stdin_is_tty: true,
            stderr_is_tty: true,
            auth_supports_request: true,
        };

        assert_eq!(
            should_offer_workspace_out_of_credits_prompt(input, Some(&zero_credit_snapshot())),
            true
        );
    }
}
