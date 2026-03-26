use std::collections::HashMap;

use codex_core::auth::read_openai_api_key_from_env;
use codex_login::CreatedApiKey;
use codex_login::OPENAI_API_KEY_ENV_VAR;
use codex_login::PendingCreateApiKey;
use codex_login::start_create_api_key as start_create_api_key_flow;
use codex_protocol::ThreadId;
use ratatui::style::Stylize;
use ratatui::text::Line;
use tokio::sync::oneshot;

use super::ChatWidget;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::clipboard_text;
use crate::history_cell;
use crate::history_cell::PlainHistoryCell;

impl ChatWidget {
    pub(crate) fn start_create_api_key(&mut self) {
        let Some(thread_id) = self.thread_id() else {
            self.add_error_message("No active Codex thread for API key creation.".to_string());
            return;
        };

        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let cell = start_create_api_key_command(thread_id, app_event_tx.clone()).await;
            app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(cell)));
        });
    }
}

async fn start_create_api_key_command(
    thread_id: ThreadId,
    app_event_tx: AppEventSender,
) -> PlainHistoryCell {
    match is_openai_api_key_set_in_session(thread_id, app_event_tx.clone()).await {
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

    complete_command(session, thread_id, app_event_tx).await
}

async fn is_openai_api_key_set_in_session(
    thread_id: ThreadId,
    app_event_tx: AppEventSender,
) -> Result<bool, String> {
    if read_openai_api_key_from_env().is_some() {
        return Ok(true);
    }

    let (result_tx, result_rx) = oneshot::channel();
    app_event_tx.send(AppEvent::GetDependencyEnv {
        thread_id,
        result_tx,
    });

    let dependency_env = match result_rx.await {
        Ok(result) => result?,
        Err(err) => {
            return Err(format!(
                "dependency env read response channel closed before completion: {err}"
            ));
        }
    };

    Ok(dependency_env.contains_key(OPENAI_API_KEY_ENV_VAR))
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
        format!("  Codex will display the new {OPENAI_API_KEY_ENV_VAR} here and copy it to your clipboard.")
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
    app_event_tx: AppEventSender,
) -> PlainHistoryCell {
    let provisioned = match session.finish().await {
        Ok(provisioned) => provisioned,
        Err(err) => {
            return history_cell::new_error_event(format!("API key creation failed: {err}"));
        }
    };
    let copy_result = clipboard_text::copy_text_to_clipboard(&provisioned.project_api_key);
    let session_env_result =
        apply_api_key_to_current_session(&provisioned.project_api_key, thread_id, app_event_tx)
            .await;

    success_cell(&provisioned, copy_result, session_env_result)
}

async fn apply_api_key_to_current_session(
    api_key: &str,
    thread_id: ThreadId,
    app_event_tx: AppEventSender,
) -> Result<(), String> {
    let (result_tx, result_rx) = oneshot::channel();
    app_event_tx.send(AppEvent::SetDependencyEnv {
        thread_id,
        values: HashMap::from([(OPENAI_API_KEY_ENV_VAR.to_string(), api_key.to_string())]),
        result_tx,
    });

    match result_rx.await {
        Ok(result) => result,
        Err(err) => Err(format!(
            "dependency env update response channel closed before completion: {err}"
        )),
    }
}

fn success_cell(
    provisioned: &CreatedApiKey,
    copy_result: Result<(), String>,
    session_env_result: Result<(), String>,
) -> PlainHistoryCell {
    let organization = provisioned
        .organization_title
        .clone()
        .unwrap_or_else(|| provisioned.organization_id.clone());
    let project = provisioned
        .default_project_title
        .clone()
        .unwrap_or_else(|| provisioned.default_project_id.clone());
    let masked_api_key = mask_api_key(&provisioned.project_api_key);
    let copy_status = match copy_result {
        Ok(()) => "I copied the full key to your clipboard.".to_string(),
        Err(err) => format!("Could not copy the key to your clipboard: {err}"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_cell::HistoryCell;
    use insta::assert_snapshot;

    #[test]
    fn success_cell_snapshot() {
        let cell = success_cell(
            &CreatedApiKey {
                organization_id: "org-default".to_string(),
                organization_title: Some("Default Org".to_string()),
                default_project_id: "proj-default".to_string(),
                default_project_title: Some("Default Project".to_string()),
                project_api_key: "sk-proj-123".to_string(),
            },
            Ok(()),
            Ok(()),
        );

        assert_snapshot!(render_cell(&cell));
    }

    #[test]
    fn success_cell_snapshot_when_clipboard_copy_fails() {
        let cell = success_cell(
            &CreatedApiKey {
                organization_id: "org-default".to_string(),
                organization_title: None,
                default_project_id: "proj-default".to_string(),
                default_project_title: None,
                project_api_key: "sk-proj-123".to_string(),
            },
            Err("clipboard unavailable".to_string()),
            Err("dependency env unavailable".to_string()),
        );

        assert_snapshot!(render_cell(&cell));
    }

    #[test]
    fn continue_in_browser_message_snapshot() {
        let cell = continue_in_browser_message(
            "https://auth.openai.com/oauth/authorize?client_id=abc",
            /*callback_port*/ 5000,
            /*browser_opened*/ false,
        );

        assert_snapshot!(render_cell(&cell));
    }

    #[test]
    fn existing_api_key_message_mentions_openai_api_key() {
        let cell = existing_api_key_message();

        assert_eq!(
            render_cell(&cell),
            "• OPENAI_API_KEY is already set in this Codex session; skipping API key creation. Unset OPENAI_API_KEY and run /create-api-key again if you want Codex to create a different key."
        );
    }

    #[test]
    fn continue_in_browser_message_always_includes_the_auth_url() {
        let cell = continue_in_browser_message(
            "https://auth.example.com/oauth/authorize?state=abc",
            5000,
            /*browser_opened*/ false,
        );

        assert!(render_cell(&cell).contains("https://auth.example.com/oauth/authorize?state=abc"));
    }

    #[test]
    fn mask_api_key_preserves_prefix_and_suffix() {
        assert_eq!(mask_api_key("sk-proj-1234567890"), "sk-proj-...7890");
    }

    fn render_cell(cell: &PlainHistoryCell) -> String {
        cell.display_lines(120)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
