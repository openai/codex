use std::collections::HashMap;

use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadDependencyEnvSetParams;
use codex_app_server_protocol::ThreadDependencyEnvSetResponse;
use codex_app_server_protocol::ThreadEnvContainsParams;
use codex_app_server_protocol::ThreadEnvContainsResponse;
use codex_login::CreatedApiKey;
use codex_login::OPENAI_API_KEY_ENV_VAR;
use codex_login::PendingCreateApiKey;
use codex_login::start_create_api_key as start_create_api_key_flow;
use codex_protocol::ThreadId;
use ratatui::style::Stylize;
use ratatui::text::Line;
use uuid::Uuid;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::clipboard_text;
use crate::history_cell;
use crate::history_cell::PlainHistoryCell;

pub(crate) fn start_command(
    app_event_tx: AppEventSender,
    request_handle: AppServerRequestHandle,
    thread_id: ThreadId,
) {
    tokio::spawn(async move {
        let cell =
            start_create_api_key_command(thread_id, request_handle, app_event_tx.clone()).await;
        app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(cell)));
    });
}

async fn start_create_api_key_command(
    thread_id: ThreadId,
    request_handle: AppServerRequestHandle,
    app_event_tx: AppEventSender,
) -> PlainHistoryCell {
    match is_openai_api_key_set_in_session(thread_id, &request_handle).await {
        Ok(true) => return existing_api_key_message(),
        Ok(false) => {}
        Err(err) => {
            return history_cell::new_error_event(format!(
                "Failed to check API key environment: {err}"
            ));
        }
    }

    let session = match start_create_api_key_flow() {
        Ok(session) => session,
        Err(err) => {
            return history_cell::new_error_event(format!(
                "Failed to start API key creation: {err}"
            ));
        }
    };
    let browser_opened = session.open_browser();
    let start_message =
        continue_in_browser_message(session.auth_url(), session.callback_port(), browser_opened);
    app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(start_message)));

    complete_command(session, thread_id, request_handle).await
}

async fn is_openai_api_key_set_in_session(
    thread_id: ThreadId,
    request_handle: &AppServerRequestHandle,
) -> Result<bool, String> {
    let request = ClientRequest::ThreadEnvContains {
        request_id: create_api_key_request_id(),
        params: ThreadEnvContainsParams {
            thread_id: thread_id.to_string(),
            key: OPENAI_API_KEY_ENV_VAR.to_string(),
        },
    };
    let response = request_handle
        .request_typed::<ThreadEnvContainsResponse>(request)
        .await
        .map_err(|err| err.to_string())?;

    Ok(response.contains)
}

fn existing_api_key_message() -> PlainHistoryCell {
    history_cell::new_info_event(
        format!(
            "{OPENAI_API_KEY_ENV_VAR} is already set in this Codex session; skipping API key creation."
        ),
        Some(format!(
            "Unset {OPENAI_API_KEY_ENV_VAR} and run /create-api-key again if you want Codex to create a different key."
        )),
    )
}

fn continue_in_browser_message(
    auth_url: &str,
    callback_port: u16,
    browser_opened: bool,
) -> PlainHistoryCell {
    let mut lines = vec![
        vec![
            "• ".dim(),
            "Finish API key creation via your browser.".into(),
        ]
        .into(),
        "".into(),
    ];

    if browser_opened {
        lines.push(
            "  Codex tried to open this link for you."
                .dark_gray()
                .into(),
        );
    } else {
        lines.push(
            "  Codex couldn't auto-open your browser, but the API key creation flow is still waiting."
                .dark_gray()
                .into(),
        );
    }
    lines.push("".into());
    lines.push("  Open the following link to authenticate:".into());
    lines.push("".into());
    lines.push(Line::from(vec![
        "  ".into(),
        auth_url.to_string().cyan().underlined(),
    ]));
    lines.push("".into());
    lines.push(
        format!(
            "  Codex will display the new {OPENAI_API_KEY_ENV_VAR} here and copy it to your clipboard."
        )
        .dark_gray()
        .into(),
    );
    lines.push("".into());
    lines.push(
        format!(
            "  On a remote or headless machine, forward localhost:{callback_port} back to this Codex host before opening the link."
        )
        .dark_gray()
        .into(),
    );

    PlainHistoryCell::new(lines)
}

async fn complete_command(
    session: PendingCreateApiKey,
    thread_id: ThreadId,
    request_handle: AppServerRequestHandle,
) -> PlainHistoryCell {
    let created = match session.finish().await {
        Ok(created) => created,
        Err(err) => {
            return history_cell::new_error_event(format!("API key creation failed: {err}"));
        }
    };
    let copy_result = clipboard_text::copy_text_to_clipboard(&created.project_api_key);
    let session_env_result =
        apply_api_key_to_current_session(&created.project_api_key, thread_id, request_handle).await;

    success_cell(&created, copy_result, session_env_result)
}

async fn apply_api_key_to_current_session(
    api_key: &str,
    thread_id: ThreadId,
    request_handle: AppServerRequestHandle,
) -> Result<(), String> {
    let request = ClientRequest::ThreadDependencyEnvSet {
        request_id: create_api_key_request_id(),
        params: ThreadDependencyEnvSetParams {
            thread_id: thread_id.to_string(),
            values: HashMap::from([(OPENAI_API_KEY_ENV_VAR.to_string(), api_key.to_string())]),
        },
    };
    request_handle
        .request_typed::<ThreadDependencyEnvSetResponse>(request)
        .await
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn success_cell(
    created: &CreatedApiKey,
    copy_result: Result<(), String>,
    session_env_result: Result<(), String>,
) -> PlainHistoryCell {
    let organization = created
        .organization_title
        .clone()
        .unwrap_or_else(|| created.organization_id.clone());
    let project = created
        .default_project_title
        .clone()
        .unwrap_or_else(|| created.default_project_id.clone());
    let masked_api_key = mask_api_key(&created.project_api_key);
    let copy_status = match copy_result {
        Ok(()) => "I copied the full key to your clipboard.".to_string(),
        Err(err) => format!("Could not copy the key to your clipboard: {err}."),
    };
    let session_env_status = match session_env_result {
        Ok(()) => format!(
            "I also set {OPENAI_API_KEY_ENV_VAR} in this Codex session for future commands."
        ),
        Err(err) => {
            format!("Could not set {OPENAI_API_KEY_ENV_VAR} in this Codex session: {err}")
        }
    };

    PlainHistoryCell::new(vec![
        vec![
            "• ".dim(),
            format!("Created an API key for {organization} / {project}: {masked_api_key}.").into(),
        ]
        .into(),
        vec!["  ".into(), format!("{copy_status} {session_env_status}").into()].into(),
        "".into(),
        vec![
            "  ".into(),
            "To create more keys or monitor usage, go to platform.openai.com.".dark_gray(),
        ]
        .into(),
        "".into(),
        vec![
            "  ".into(),
            "You can start building with the OpenAI API with limited usage of gpt-5.4-nano. To use more models, add credits on platform.openai.com."
                .dark_gray(),
        ]
        .into(),
    ])
}

fn mask_api_key(api_key: &str) -> String {
    const UNMASKED_PREFIX_LEN: usize = 8;
    const UNMASKED_SUFFIX_LEN: usize = 4;

    if api_key.len() <= UNMASKED_PREFIX_LEN + UNMASKED_SUFFIX_LEN {
        return api_key.to_string();
    }

    format!(
        "{}...{}",
        &api_key[..UNMASKED_PREFIX_LEN],
        &api_key[api_key.len() - UNMASKED_SUFFIX_LEN..]
    )
}

fn create_api_key_request_id() -> RequestId {
    RequestId::String(Uuid::new_v4().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_cell::HistoryCell;
    use pretty_assertions::assert_eq;

    #[test]
    fn success_cell_renders_expected_copy() {
        let cell = success_cell(
            &CreatedApiKey {
                organization_id: "org-default".to_string(),
                organization_title: Some("Default Org".to_string()),
                default_project_id: "proj-default".to_string(),
                default_project_title: Some("Default Project".to_string()),
                project_api_key: "sk-proj-1234567890".to_string(),
            },
            Ok(()),
            Ok(()),
        );

        assert_eq!(
            render_cell(&cell),
            "• Created an API key for Default Org / Default Project: sk-proj-...7890.\n  I copied the full key to your clipboard. I also set OPENAI_API_KEY in this Codex session for future commands.\n\n  To create more keys or monitor usage, go to platform.openai.com.\n\n  You can start building with the OpenAI API with limited usage of gpt-5.4-nano. To use more models, add credits on platform.openai.com."
        );
    }

    #[test]
    fn success_cell_renders_clipboard_failure() {
        let cell = success_cell(
            &CreatedApiKey {
                organization_id: "org-default".to_string(),
                organization_title: None,
                default_project_id: "proj-default".to_string(),
                default_project_title: None,
                project_api_key: "sk-proj-1234567890".to_string(),
            },
            Err("clipboard unavailable".to_string()),
            Ok(()),
        );

        assert_eq!(
            render_cell(&cell),
            "• Created an API key for org-default / proj-default: sk-proj-...7890.\n  Could not copy the key to your clipboard: clipboard unavailable. I also set OPENAI_API_KEY in this Codex session for future commands.\n\n  To create more keys or monitor usage, go to platform.openai.com.\n\n  You can start building with the OpenAI API with limited usage of gpt-5.4-nano. To use more models, add credits on platform.openai.com."
        );
    }

    #[test]
    fn continue_in_browser_message_mentions_manual_fallback() {
        let cell = continue_in_browser_message(
            "https://auth.example.test/oauth",
            /*callback_port*/ 5000,
            /*browser_opened*/ false,
        );

        assert_eq!(
            render_cell(&cell),
            "• Finish API key creation via your browser.\n\n  Codex couldn't auto-open your browser, but the API key creation flow is still waiting.\n\n  Open the following link to authenticate:\n\n  https://auth.example.test/oauth\n\n  Codex will display the new OPENAI_API_KEY here and copy it to your clipboard.\n\n  On a remote or headless machine, forward localhost:5000 back to this Codex host before opening the link."
        );
    }

    #[test]
    fn mask_api_key_preserves_short_values() {
        assert_eq!(mask_api_key("sk-short"), "sk-short");
    }

    #[test]
    fn mask_api_key_redacts_middle() {
        assert_eq!(mask_api_key("sk-proj-1234567890"), "sk-proj-...7890");
    }

    fn render_cell(cell: &PlainHistoryCell) -> String {
        cell.display_lines(120)
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
}
