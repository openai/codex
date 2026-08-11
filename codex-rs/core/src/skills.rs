use crate::config::Config;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_analytics::InvocationType;
use codex_analytics::SkillInvocation;
use codex_analytics::SkillInvocationLocation;
use codex_analytics::TrackEventsContext;
use codex_analytics::build_track_events_context;
use codex_extension_api::SkillInvocationInput;
use codex_extension_api::SkillInvocationKind;
use codex_otel::sanitize_metric_tag_value;
use codex_protocol::protocol::SkillScope;
use codex_skills::SkillMetadata;
use codex_skills_extension::HostSkillsLoadInput;
use codex_skills_extension::detect_implicit_skill_invocation;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::PluginSkillRoot;
use std::collections::HashSet;
use tokio::sync::Mutex;

#[derive(Debug, Default)]
struct ImplicitSkillInvocations(Mutex<HashSet<String>>);

pub(crate) fn skills_load_input_from_config(
    config: &Config,
    effective_skill_roots: Vec<PluginSkillRoot>,
) -> HostSkillsLoadInput {
    HostSkillsLoadInput::new(
        config.cwd.clone(),
        effective_skill_roots,
        config.config_layer_stack.clone(),
    )
}

pub(crate) fn emit_explicit_skill_invocations(
    sess: &Session,
    turn_context: &TurnContext,
    mentioned_skills: &[SkillMetadata],
    injected_skills: &[SkillMetadata],
    tracking: TrackEventsContext,
) {
    let injected_skill_paths = injected_skills
        .iter()
        .map(|skill| &skill.path_to_skills_md)
        .collect::<HashSet<_>>();
    for skill in mentioned_skills {
        let skill_name_tag = sanitize_metric_tag_value(skill.name.as_str());
        let status = if injected_skill_paths.contains(&skill.path_to_skills_md) {
            "ok"
        } else {
            "error"
        };
        turn_context.session_telemetry.counter(
            "codex.skill.injected",
            /*inc*/ 1,
            &[
                ("status", status),
                ("skill", skill_name_tag.as_str()),
                ("invoke_type", "explicit"),
            ],
        );
    }

    let invocations = injected_skills
        .iter()
        .map(|skill| SkillInvocation {
            skill_name: skill.name.clone(),
            location: SkillInvocationLocation::Host {
                path: skill.path_to_skills_md.to_path_buf(),
                scope: skill.scope,
            },
            plugin_id: skill.plugin_id.clone(),
            remote_plugin_id: skill.remote_plugin_id.clone(),
            invocation_type: InvocationType::Explicit,
        })
        .collect();
    sess.services
        .analytics_events_client
        .track_skill_invocations(tracking, invocations);
}

pub(crate) async fn maybe_emit_implicit_skill_invocation(
    sess: &Session,
    turn_context: &TurnContext,
    command: &str,
    workdir: &PathUri,
    native_workdir: Option<&AbsolutePathBuf>,
    environment_id: &str,
) {
    let Some(invocation) = detect_implicit_skill_invocation(
        turn_context.extension_data.as_ref(),
        environment_id,
        command,
        workdir,
        native_workdir,
    ) else {
        return;
    };
    let skill_name = invocation.skill_name.clone();
    let (skill_resource, seen_key) = match &invocation.location {
        SkillInvocationLocation::Host { path, scope } => {
            let skill_scope = match scope {
                SkillScope::User => "user",
                SkillScope::Repo => "repo",
                SkillScope::System => "system",
                SkillScope::Admin => "admin",
            };
            let skill_path = path.to_string_lossy().into_owned();
            let seen_key = format!("{skill_scope}:{skill_path}:{skill_name}");
            (skill_path, seen_key)
        }
        SkillInvocationLocation::Resource { id, .. } => (id.clone(), format!("resource:{id}")),
    };
    let inserted = {
        let skill_invocations = turn_context
            .extension_data
            .get_or_init(ImplicitSkillInvocations::default);
        let mut seen_skills = skill_invocations.0.lock().await;
        seen_skills.insert(seen_key)
    };
    if !inserted {
        return;
    }
    let skill_name_tag = sanitize_metric_tag_value(skill_name.as_str());

    for contributor in sess.services.extensions.skill_invocation_contributors() {
        contributor
            .on_skill_invocation(SkillInvocationInput {
                session_store: &sess.services.session_extension_data,
                thread_store: &sess.services.thread_extension_data,
                turn_store: turn_context.extension_data.as_ref(),
                turn_id: turn_context.sub_id.as_str(),
                skill_resource: skill_resource.as_str(),
                kind: SkillInvocationKind::Implicit,
            })
            .await;
    }

    turn_context.session_telemetry.counter(
        "codex.skill.injected",
        /*inc*/ 1,
        &[
            ("status", "ok"),
            ("skill", skill_name_tag.as_str()),
            ("invoke_type", "implicit"),
        ],
    );
    sess.services
        .analytics_events_client
        .track_skill_invocations(
            build_track_events_context(
                turn_context.model_info.slug.clone(),
                sess.thread_id.to_string(),
                turn_context.sub_id.clone(),
                turn_context.originator.clone(),
            ),
            vec![invocation],
        );
}
