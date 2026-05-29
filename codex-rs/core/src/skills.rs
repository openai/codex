use crate::config::Config;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_analytics::InvocationType;
use codex_analytics::SkillInvocation;
use codex_analytics::build_track_events_context;
pub use codex_exec_server::EnvironmentPathRef;
use codex_exec_server::ExecutorFileSystem;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::PluginSkillRoot;
use std::sync::Arc;

pub use codex_core_skills::SkillError;
pub use codex_core_skills::SkillLoadOutcome;
pub use codex_core_skills::SkillMetadata;
pub use codex_core_skills::SkillPolicy;
pub use codex_core_skills::SkillRenderReport;
pub use codex_core_skills::SkillsLoadInput;
pub use codex_core_skills::SkillsManager;
pub use codex_core_skills::build_available_skills;
pub use codex_core_skills::build_skill_name_counts;
pub use codex_core_skills::config_rules;
pub use codex_core_skills::default_skill_metadata_budget;
pub use codex_core_skills::detect_implicit_skill_invocation_for_command;
pub use codex_core_skills::filter_skill_load_outcome_for_product;
pub use codex_core_skills::injection;
pub use codex_core_skills::injection::SkillInjections;
pub use codex_core_skills::injection::build_skill_injections;
pub use codex_core_skills::injection::collect_explicit_skill_mentions;
pub use codex_core_skills::loader;
pub use codex_core_skills::manager;
pub use codex_core_skills::model;
pub use codex_core_skills::remote;
pub use codex_core_skills::render;
pub use codex_core_skills::render::SkillRenderSideEffects;
pub use codex_core_skills::system;

pub(crate) fn skills_load_input_from_config(
    config: &Config,
    env_path: Option<EnvironmentPathRef>,
    local_file_system: Option<Arc<dyn ExecutorFileSystem>>,
    effective_skill_roots: Vec<PluginSkillRoot>,
) -> SkillsLoadInput {
    SkillsLoadInput::new(
        env_path,
        local_file_system,
        effective_skill_roots,
        config.config_layer_stack.clone(),
        config.bundled_skills_enabled(),
    )
}

pub(crate) async fn maybe_emit_implicit_skill_invocation(
    sess: &Session,
    turn_context: &TurnContext,
    command: &str,
    workdir: &AbsolutePathBuf,
) {
    let workdir = turn_context
        .environments
        .primary()
        .map(|environment| {
            EnvironmentPathRef::new(environment.environment.get_filesystem(), workdir.clone())
        })
        .unwrap_or_else(|| EnvironmentPathRef::local(workdir.clone()));
    let Some(candidate) = detect_implicit_skill_invocation_for_command(
        turn_context.turn_skills.outcome.as_ref(),
        command,
        &workdir,
    ) else {
        return;
    };
    let invocation = SkillInvocation {
        skill_name: candidate.name,
        skill_scope: candidate.scope,
        skill_path: candidate.path_to_skills_md.to_path_buf(),
        plugin_id: candidate.plugin_id,
        invocation_type: InvocationType::Implicit,
    };
    let skill_scope = match invocation.skill_scope {
        SkillScope::User => "user",
        SkillScope::Repo => "repo",
        SkillScope::System => "system",
        SkillScope::Admin => "admin",
    };
    let skill_path = invocation.skill_path.to_string_lossy();
    let skill_name = invocation.skill_name.clone();
    let seen_key = format!("{skill_scope}:{skill_path}:{skill_name}");
    let inserted = {
        let mut seen_skills = turn_context
            .turn_skills
            .implicit_invocation_seen_skills
            .lock()
            .await;
        seen_skills.insert(seen_key)
    };
    if !inserted {
        return;
    }

    turn_context.session_telemetry.counter(
        "codex.skill.injected",
        /*inc*/ 1,
        &[
            ("status", "ok"),
            ("skill", skill_name.as_str()),
            ("invoke_type", "implicit"),
        ],
    );
    sess.services
        .analytics_events_client
        .track_skill_invocations(
            build_track_events_context(
                turn_context.model_info.slug.clone(),
                sess.conversation_id.to_string(),
                turn_context.sub_id.clone(),
            ),
            vec![invocation],
        );
}
