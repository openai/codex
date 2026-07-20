use crate::config::Config;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_analytics::InvocationType;
use codex_analytics::SkillInvocation;
use codex_analytics::build_track_events_context;
use codex_extension_api::SkillInvocationInput;
use codex_extension_api::SkillInvocationKind;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::PluginSkillRoot;

pub use codex_core_skills::SkillError;
pub use codex_core_skills::SkillLoadOutcome;
pub use codex_core_skills::SkillRenderReport;
pub use codex_core_skills::SkillsLoadInput;
pub use codex_core_skills::SkillsService;
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
pub use codex_core_skills::model;
pub use codex_core_skills::remote;
pub use codex_core_skills::render;
pub use codex_core_skills::render::SkillRenderSideEffects;
pub use codex_core_skills::service;
pub use codex_core_skills::system;
pub use codex_skills::SkillMetadata;
pub use codex_skills::SkillPolicy;

pub(crate) fn skills_load_input_from_config(
    config: &Config,
    effective_skill_roots: Vec<PluginSkillRoot>,
) -> SkillsLoadInput {
    SkillsLoadInput::new(
        config.cwd.clone(),
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
    let Some(candidate) = detect_implicit_skill_invocation_for_command(
        turn_context.turn_skills.snapshot.outcome(),
        command,
        workdir,
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

    for contributor in sess.services.extensions.skill_invocation_contributors() {
        contributor
            .on_skill_invocation(SkillInvocationInput {
                session_store: &sess.services.session_extension_data,
                thread_store: &sess.services.thread_extension_data,
                turn_store: turn_context.extension_data.as_ref(),
                turn_id: turn_context.sub_id.as_str(),
                skill_resource: skill_path.as_ref(),
                kind: SkillInvocationKind::Implicit,
            })
            .await;
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
                sess.thread_id.to_string(),
                turn_context.sub_id.clone(),
                turn_context.originator.clone(),
            ),
            vec![invocation],
        );
}
