/// Integration tests for GitHub and Slack
///
/// Tests webhook triggers, notifications, and bot interactions
use codex_core::integrations::github::CreatePrRequest;
/// Integration tests for GitHub and Slack
///
/// Tests webhook triggers, notifications, and bot interactions
use codex_core::integrations::github::GitHubIntegration;
/// Integration tests for GitHub and Slack
///
/// Tests webhook triggers, notifications, and bot interactions
use codex_core::integrations::github::ReviewComment;
/// Integration tests for GitHub and Slack
///
/// Tests webhook triggers, notifications, and bot interactions
use codex_core::integrations::github::ReviewSeverity;
use codex_core::integrations::slack::SlackIntegration;
use codex_core::integrations::slack::SlackMessage;
use codex_core::integrations::webhook::WebhookConfig;
use codex_core::integrations::webhook::WebhookEvent;
use codex_core::integrations::webhook::WebhookHandler;
use codex_core::integrations::webhook::WebhookPayload;
use pretty_assertions::assert_eq;
use std::collections::HashMap;

#[tokio::test]
async fn test_github_pr_creation() {
    let gh = GitHubIntegration::new("zapabob/codex".to_string(), None);

    let request = CreatePrRequest {
        title: "feat: Add sub-agent support".to_string(),
        body: "This PR adds sub-agent delegation support.".to_string(),
        head: "feature/sub-agents".to_string(),
        base: "main".to_string(),
        draft: false,
    };

    let pr = gh.create_pr(request).await.unwrap();

    assert_eq!(pr.title, "feat: Add sub-agent support");
    assert_eq!(pr.state, "open");
    assert!(pr.number > 0);
}

#[tokio::test]
async fn test_github_review_comment() {
    let gh = GitHubIntegration::new("zapabob/codex".to_string(), None);

    let comment = ReviewComment {
        path: "src/agents/runtime.rs".to_string(),
        line: 42,
        body: "Consider using Arc instead of Rc here.".to_string(),
        severity: ReviewSeverity::Medium,
    };

    gh.add_review_comment(1, comment).await.unwrap();
}

#[tokio::test]
async fn test_slack_notification() {
    let slack = SlackIntegration::new(
        Some("https://hooks.slack.com/services/TEST/TEST/TEST".to_string()),
        None,
        "#codex".to_string(),
    );

    let message = SlackMessage {
        text: "✅ Sub-agent task completed!".to_string(),
        attachments: vec![],
    };

    // This will succeed with mock implementation
    slack.send_notification(message).await.unwrap();
}

#[tokio::test]
async fn test_slack_agent_progress() {
    let slack = SlackIntegration::new(
        Some("https://hooks.slack.com/services/TEST/TEST/TEST".to_string()),
        None,
        "#codex".to_string(),
    );

    slack
        .notify_agent_progress("test-gen", 0.75, "running")
        .await
        .unwrap();
}

#[tokio::test]
async fn test_slack_research_complete() {
    let slack = SlackIntegration::new(
        Some("https://hooks.slack.com/services/TEST/TEST/TEST".to_string()),
        None,
        "#research".to_string(),
    );

    slack
        .notify_research_complete(
            "Rust async patterns",
            "Research completed successfully with 10 sources.",
            &["artifacts/report.md".to_string()],
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn test_webhook_registration_and_trigger() {
    let mut handler = WebhookHandler::new();

    let config = WebhookConfig {
        url: "https://example.com/webhook".to_string(),
        events: vec![
            WebhookEvent::AgentCompleted,
            WebhookEvent::ResearchCompleted,
        ],
        headers: HashMap::new(),
        auth: None,
    };

    handler.register("test-webhook".to_string(), config);

    let mut data = HashMap::new();
    data.insert("agent".to_string(), serde_json::json!("test-agent"));
    data.insert("status".to_string(), serde_json::json!("success"));

    let payload = WebhookPayload {
        event: WebhookEvent::AgentCompleted,
        timestamp: "2025-10-10T18:00:00Z".to_string(),
        data,
    };

    handler.trigger("test-webhook", payload).await.unwrap();
}

#[tokio::test]
async fn test_webhook_event_filtering() {
    let mut handler = WebhookHandler::new();

    // Webhook for agent events only
    let agent_webhook = WebhookConfig {
        url: "https://example.com/agent-webhook".to_string(),
        events: vec![WebhookEvent::AgentStarted, WebhookEvent::AgentCompleted],
        headers: HashMap::new(),
        auth: None,
    };

    // Webhook for PR events only
    let pr_webhook = WebhookConfig {
        url: "https://example.com/pr-webhook".to_string(),
        events: vec![WebhookEvent::PrCreated, WebhookEvent::PrMerged],
        headers: HashMap::new(),
        auth: None,
    };

    handler.register("agent-webhook".to_string(), agent_webhook);
    handler.register("pr-webhook".to_string(), pr_webhook);

    // Trigger agent event
    let payload = WebhookPayload {
        event: WebhookEvent::AgentCompleted,
        timestamp: "2025-10-10T18:00:00Z".to_string(),
        data: HashMap::new(),
    };

    handler
        .trigger_for_event(WebhookEvent::AgentCompleted, payload)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_github_slack_integration_flow() {
    // Full integration flow: Agent → PR → Slack notification

    // 1. Create GitHub integration
    let gh = GitHubIntegration::new("zapabob/codex".to_string(), None);

    // 2. Create Slack integration
    let slack = SlackIntegration::new(
        Some("https://hooks.slack.com/services/TEST".to_string()),
        None,
        "#codex".to_string(),
    );

    // 3. Simulate agent completing task
    let agent_name = "test-gen";

    // 4. Create PR
    let pr_request = CreatePrRequest {
        title: format!("[{}] Add unit tests", agent_name),
        body: "Auto-generated tests by sub-agent.".to_string(),
        head: "codex/auto/tests".to_string(),
        base: "main".to_string(),
        draft: false,
    };

    let pr = gh.create_pr(pr_request).await.unwrap();

    // 5. Notify Slack
    slack
        .notify_pr_created(pr.number, &pr.url, agent_name)
        .await
        .unwrap();

    println!(
        "✅ Integration flow completed: Agent → PR #{} → Slack",
        pr.number
    );
}
