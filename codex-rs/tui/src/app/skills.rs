use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use codex_app_server_client::InProcessAppServerRequester;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SkillErrorInfo;
use codex_app_server_protocol::SkillsListParams;
use codex_app_server_protocol::SkillsListResponse;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

static NEXT_APP_SERVER_REQUEST_ID: AtomicI64 = AtomicI64::new(1);

pub(super) fn errors_for_cwd(cwd: &Path, response: &SkillsListResponse) -> Vec<SkillErrorInfo> {
    response
        .data
        .iter()
        .find(|entry| entry.cwd.as_path() == cwd)
        .map(|entry| entry.errors.clone())
        .unwrap_or_default()
}

pub(super) fn effective_skills_list_cwds(cwds: Vec<PathBuf>, current_cwd: &Path) -> Vec<PathBuf> {
    if cwds.is_empty() {
        vec![current_cwd.to_path_buf()]
    } else {
        cwds
    }
}

pub(super) fn request_skills_list(
    app_event_tx: AppEventSender,
    client: InProcessAppServerRequester,
    cwds: Vec<PathBuf>,
    force_reload: bool,
    generation: u64,
) {
    tokio::spawn(async move {
        let requested_cwds = cwds.clone();

        let request_id =
            RequestId::Integer(NEXT_APP_SERVER_REQUEST_ID.fetch_add(1, Ordering::Relaxed));
        let result = client
            .request_typed(ClientRequest::SkillsList {
                request_id,
                params: SkillsListParams {
                    cwds,
                    force_reload,
                    per_cwd_extra_user_roots: None,
                },
            })
            .await
            .map_err(|err| format!("skills/list failed: {err}"));

        app_event_tx.send(AppEvent::SkillsListLoaded {
            requested_cwds,
            generation,
            result,
        });
    });
}

pub(super) fn emit_skill_load_warnings(app_event_tx: &AppEventSender, errors: &[SkillErrorInfo]) {
    if errors.is_empty() {
        return;
    }

    let error_count = errors.len();
    app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
        crate::history_cell::new_warning_event(format!(
            "Skipped loading {error_count} skill(s) due to invalid SKILL.md files."
        )),
    )));

    for error in errors {
        let path = error.path.display();
        let message = error.message.as_str();
        app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
            crate::history_cell::new_warning_event(format!("{path}: {message}")),
        )));
    }
}
