use crate::config::types::WebhookFormat;
use serde_json::Value;
use serde_json::json;

use super::WebhookPayload;

pub(crate) fn format_payload(format: Option<WebhookFormat>, payload: &WebhookPayload) -> Value {
    match format {
        Some(WebhookFormat::Dingtalk) => dingtalk_text(payload),
        Some(WebhookFormat::Slack) => slack_text(payload),
        Some(WebhookFormat::Discord) => discord_text(payload),
        Some(WebhookFormat::Teams) => teams_text(payload),
        Some(WebhookFormat::Feishu) => feishu_text(payload),
        Some(WebhookFormat::Wecom) => wecom_text(payload),
        None => json!(payload),
    }
}

fn dingtalk_text(payload: &WebhookPayload) -> Value {
    let text = match payload {
        WebhookPayload::TaskCompleted {
            turn_id,
            task_kind,
            cwd,
            last_agent_message,
            ..
        } => {
            let summary = last_agent_message.as_deref().unwrap_or("无总结");
            format!("[codex] 任务完成 | turn={turn_id} | task={task_kind} | cwd={cwd} | {summary}")
        }
        WebhookPayload::TaskAborted {
            turn_id,
            task_kind,
            cwd,
            reason,
            ..
        } => format!(
            "[codex] 任务中止 | turn={turn_id} | task={task_kind} | cwd={cwd} | reason={reason:?}"
        ),
    };
    json!({ "msgtype": "text", "text": { "content": text } })
}

fn slack_text(payload: &WebhookPayload) -> Value {
    json!({ "text": plain_text(payload) })
}

fn teams_text(payload: &WebhookPayload) -> Value {
    json!({ "text": plain_text(payload) })
}

fn discord_text(payload: &WebhookPayload) -> Value {
    json!({ "content": plain_text(payload) })
}

fn feishu_text(payload: &WebhookPayload) -> Value {
    json!({ "msg_type": "text", "content": { "text": plain_text(payload) } })
}

fn wecom_text(payload: &WebhookPayload) -> Value {
    json!({ "msgtype": "text", "text": { "content": plain_text(payload) } })
}

fn plain_text(payload: &WebhookPayload) -> String {
    match payload {
        WebhookPayload::TaskCompleted {
            turn_id,
            task_kind,
            cwd,
            last_agent_message,
            ..
        } => {
            let summary = last_agent_message.as_deref().unwrap_or("no summary");
            format!(
                "[codex] task completed | turn={turn_id} | task={task_kind} | cwd={cwd} | {summary}"
            )
        }
        WebhookPayload::TaskAborted {
            turn_id,
            task_kind,
            cwd,
            reason,
            ..
        } => format!(
            "[codex] task aborted | turn={turn_id} | task={task_kind} | cwd={cwd} | reason={reason:?}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use crate::config::types::WebhookFormat;
    use crate::protocol::TurnAbortReason;
    use crate::webhook::WebhookPayload;
    use crate::webhook::WebhookTaskKind;
    use crate::webhook::formatter::format_payload;
    use codex_protocol::ConversationId;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn formats_default_payload_as_json() {
        let payload = WebhookPayload::TaskCompleted {
            thread_id: ConversationId::default(),
            turn_id: "turn-1".to_string(),
            task_kind: WebhookTaskKind::Review,
            cwd: "/workspace".to_string(),
            last_agent_message: Some("done".to_string()),
        };

        assert_eq!(format_payload(None, &payload), json!(payload));
    }

    #[test]
    fn formats_dingtalk_text_payload() {
        let payload = WebhookPayload::TaskAborted {
            thread_id: ConversationId::default(),
            turn_id: "turn-2".to_string(),
            task_kind: WebhookTaskKind::Compact,
            cwd: "/workspace".to_string(),
            reason: TurnAbortReason::Interrupted,
        };

        assert_eq!(
            format_payload(Some(WebhookFormat::Dingtalk), &payload),
            json!({
                "msgtype": "text",
                "text": {
                    "content": "[codex] 任务中止 | turn=turn-2 | task=compact | cwd=/workspace | reason=Interrupted"
                }
            })
        );
    }

    #[test]
    fn formats_slack_payload() {
        let payload = WebhookPayload::TaskCompleted {
            thread_id: ConversationId::default(),
            turn_id: "turn-3".to_string(),
            task_kind: WebhookTaskKind::Regular,
            cwd: "/repo".to_string(),
            last_agent_message: None,
        };

        assert_eq!(
            format_payload(Some(WebhookFormat::Slack), &payload),
            json!({
                "text": "[codex] task completed | turn=turn-3 | task=regular | cwd=/repo | no summary"
            })
        );
    }

    #[test]
    fn formats_discord_payload() {
        let payload = WebhookPayload::TaskCompleted {
            thread_id: ConversationId::default(),
            turn_id: "turn-4".to_string(),
            task_kind: WebhookTaskKind::Regular,
            cwd: "/repo".to_string(),
            last_agent_message: Some("hi".to_string()),
        };

        assert_eq!(
            format_payload(Some(WebhookFormat::Discord), &payload),
            json!({
                "content": "[codex] task completed | turn=turn-4 | task=regular | cwd=/repo | hi"
            })
        );
    }

    #[test]
    fn formats_teams_payload() {
        let payload = WebhookPayload::TaskAborted {
            thread_id: ConversationId::default(),
            turn_id: "turn-5".to_string(),
            task_kind: WebhookTaskKind::Review,
            cwd: "/repo".to_string(),
            reason: TurnAbortReason::ReviewEnded,
        };

        assert_eq!(
            format_payload(Some(WebhookFormat::Teams), &payload),
            json!({
                "text": "[codex] task aborted | turn=turn-5 | task=review | cwd=/repo | reason=ReviewEnded"
            })
        );
    }

    #[test]
    fn formats_feishu_payload() {
        let payload = WebhookPayload::TaskCompleted {
            thread_id: ConversationId::default(),
            turn_id: "turn-6".to_string(),
            task_kind: WebhookTaskKind::Compact,
            cwd: "/repo".to_string(),
            last_agent_message: Some("done".to_string()),
        };

        assert_eq!(
            format_payload(Some(WebhookFormat::Feishu), &payload),
            json!({
                "msg_type": "text",
                "content": {
                    "text": "[codex] task completed | turn=turn-6 | task=compact | cwd=/repo | done"
                }
            })
        );
    }

    #[test]
    fn formats_wecom_payload() {
        let payload = WebhookPayload::TaskCompleted {
            thread_id: ConversationId::default(),
            turn_id: "turn-7".to_string(),
            task_kind: WebhookTaskKind::Regular,
            cwd: "/repo".to_string(),
            last_agent_message: None,
        };

        assert_eq!(
            format_payload(Some(WebhookFormat::Wecom), &payload),
            json!({
                "msgtype": "text",
                "text": {
                    "content": "[codex] task completed | turn=turn-7 | task=regular | cwd=/repo | no summary"
                }
            })
        );
    }
}
