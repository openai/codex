use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use codex_app_server_client::InProcessAppServerRequester;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SkillsListParams;
use codex_app_server_protocol::SkillsListResponse;
use codex_protocol::protocol::ListSkillsResponseEvent;
use codex_protocol::protocol::SkillErrorInfo;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

static NEXT_APP_SERVER_REQUEST_ID: AtomicI64 = AtomicI64::new(1);

pub(super) fn errors_for_cwd(
    cwd: &Path,
    response: &ListSkillsResponseEvent,
) -> Vec<SkillErrorInfo> {
    response
        .skills
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

pub(super) fn request_app_server_skills_list(
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
            .map(into_core_skills_list_response_event)
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

fn into_core_skills_list_response_event(response: SkillsListResponse) -> ListSkillsResponseEvent {
    ListSkillsResponseEvent {
        skills: response
            .data
            .into_iter()
            .map(into_core_skills_list_entry)
            .collect(),
    }
}

/// Convert an app-server protocol skills entry into the core protocol
/// equivalent so the existing `ChatWidget` skills UI can consume it
/// unchanged. Each nested type has a corresponding `into_core_*` helper
/// below; these are intentionally field-by-field rather than derived to
/// keep the two protocol crates decoupled and make field divergence a
/// compile error.
fn into_core_skills_list_entry(
    entry: codex_app_server_protocol::SkillsListEntry,
) -> codex_protocol::protocol::SkillsListEntry {
    codex_protocol::protocol::SkillsListEntry {
        cwd: entry.cwd,
        skills: entry
            .skills
            .into_iter()
            .map(into_core_skill_metadata)
            .collect(),
        errors: entry
            .errors
            .into_iter()
            .map(into_core_skill_error_info)
            .collect(),
    }
}

fn into_core_skill_metadata(
    skill: codex_app_server_protocol::SkillMetadata,
) -> codex_protocol::protocol::SkillMetadata {
    codex_protocol::protocol::SkillMetadata {
        name: skill.name,
        description: skill.description,
        short_description: skill.short_description,
        interface: skill.interface.map(into_core_skill_interface),
        dependencies: skill.dependencies.map(into_core_skill_dependencies),
        path: skill.path,
        scope: into_core_skill_scope(skill.scope),
        enabled: skill.enabled,
    }
}

fn into_core_skill_scope(
    scope: codex_app_server_protocol::SkillScope,
) -> codex_protocol::protocol::SkillScope {
    match scope {
        codex_app_server_protocol::SkillScope::User => codex_protocol::protocol::SkillScope::User,
        codex_app_server_protocol::SkillScope::Repo => codex_protocol::protocol::SkillScope::Repo,
        codex_app_server_protocol::SkillScope::System => {
            codex_protocol::protocol::SkillScope::System
        }
        codex_app_server_protocol::SkillScope::Admin => codex_protocol::protocol::SkillScope::Admin,
    }
}

fn into_core_skill_interface(
    interface: codex_app_server_protocol::SkillInterface,
) -> codex_protocol::protocol::SkillInterface {
    codex_protocol::protocol::SkillInterface {
        display_name: interface.display_name,
        short_description: interface.short_description,
        icon_small: interface.icon_small,
        icon_large: interface.icon_large,
        brand_color: interface.brand_color,
        default_prompt: interface.default_prompt,
    }
}

fn into_core_skill_dependencies(
    deps: codex_app_server_protocol::SkillDependencies,
) -> codex_protocol::protocol::SkillDependencies {
    codex_protocol::protocol::SkillDependencies {
        tools: deps
            .tools
            .into_iter()
            .map(into_core_skill_tool_dependency)
            .collect(),
    }
}

fn into_core_skill_tool_dependency(
    tool: codex_app_server_protocol::SkillToolDependency,
) -> codex_protocol::protocol::SkillToolDependency {
    codex_protocol::protocol::SkillToolDependency {
        r#type: tool.r#type,
        value: tool.value,
        description: tool.description,
        transport: tool.transport,
        command: tool.command,
        url: tool.url,
    }
}

fn into_core_skill_error_info(
    error: codex_app_server_protocol::SkillErrorInfo,
) -> codex_protocol::protocol::SkillErrorInfo {
    codex_protocol::protocol::SkillErrorInfo {
        path: error.path,
        message: error.message,
    }
}
