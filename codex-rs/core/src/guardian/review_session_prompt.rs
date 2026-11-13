//! Captures synchronous evidence once for the common preparation pass.
//! Reviewer-history token measurement is needed only for captured images.

use codex_guardian_context::ElasticSection;
use codex_guardian_reviewer::GuardianPromptItems;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::CatalogModel;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelOrigin;
use codex_tools::normalize_output_image_detail;
use std::borrow::Cow;
use tracing::warn;

use super::super::prompt::GuardianPromptMode;
use super::super::prompt::GuardianTranscriptHistory;
use super::super::prompt::collect_guardian_prompt_items_with_parent_turn;
use super::GuardianReviewSession;
use super::GuardianReviewSessionParams;
use crate::context_manager::ContextManager;
use crate::image_preparation::ImagePreparationMode;
use crate::image_preparation::resize_image;

pub(super) async fn build_guardian_prompt_items(
    params: &GuardianReviewSessionParams,
    reviewer: &GuardianReviewSession,
    model_info: &CatalogModel,
    reviewer_history: &ContextManager,
    history: GuardianTranscriptHistory<'_>,
    mode: GuardianPromptMode,
    reviewed_node_repl_evidence_sequence: u64,
) -> anyhow::Result<GuardianPromptItems> {
    let selected_tool_names = params
        .spawn_config
        .guardian_sync_elastic_budget
        .as_ref()
        .filter(|budget| {
            budget
                .sections
                .contains_key(&ElasticSection::TranscriptSelectedExecResults)
        })
        .map_or(&[][..], |budget| budget.selected_exec_tool_names.as_slice());
    let select_exec_output = |output: &ResponseItem| {
        params
            .parent_session
            .services
            .executed_tool_calls
            .exec_output_matches(output, selected_tool_names)
    };
    let collected = collect_guardian_prompt_items_with_parent_turn(
        params.parent_session.as_ref(),
        history,
        Some(&params.parent_context),
        params.reasons.clone(),
        params.request.clone(),
        (!selected_tool_names.is_empty()).then_some(&select_exec_output),
        reviewed_node_repl_evidence_sequence,
    )
    .await?;
    let context_window = model_info.resolved_context_window().map(|supported| {
        params
            .spawn_config
            .model_context_window
            .unwrap_or(supported)
            .min(supported)
            .saturating_mul(model_info.effective_context_window_percent.clamp(0, 100))
            / 100
    });
    let context_window =
        context_window.filter(|limit| *limit > 0 && model_info.origin != ModelOrigin::Synthetic);
    let image_mode = ImagePreparationMode::for_model(&params.spawn_config.features, model_info);
    let history_tokens = if context_window.is_some() && collected.sections.has_images() {
        let base_instructions = reviewer.session.get_base_instructions().await;
        reviewer_history
            .estimate_token_count_with_base_instructions(&base_instructions)
            .unwrap_or(i64::MAX)
            .max(reviewer.session.get_total_token_usage().await)
    } else {
        i64::MAX
    };
    let image_preparer = ReviewImagePreparer {
        model_info,
        mode: image_mode,
    };
    codex_guardian_reviewer::compose_guardian_prompt_with_preparation(
        collected.sections,
        &collected.profile,
        mode,
        collected.history_version,
        &collected.session_id,
        collected.node_repl_evidence_sequence,
        codex_guardian_context::SyncPreparation {
            budgeting: params.spawn_config.guardian_context_budgeting(),
            reviewer_history: reviewer_history.annotated_items(),
            context_window,
            history_tokens,
            supports_images: model_info.input_modalities.contains(&InputModality::Image),
            image_preparer: &image_preparer,
            estimate_tokens: crate::context_manager::estimate_item_token_count,
        },
    )
    .await
}

struct ReviewImagePreparer<'a> {
    model_info: &'a ModelInfo,
    mode: ImagePreparationMode,
}

impl codex_guardian_context::SyncImagePreparer for ReviewImagePreparer<'_> {
    fn prepare<'image>(
        &self,
        image: &'image ImageReference,
        detail: &mut Option<ImageDetail>,
    ) -> Option<Cow<'image, ImageReference>> {
        *detail = match normalize_output_image_detail(self.model_info, *detail) {
            _ if self.mode.unified_budget => Some(ImageDetail::Original),
            Some(ImageDetail::Low) => Some(ImageDetail::High),
            detail => detail,
        };
        match image {
            ImageReference::Inline { image_url } => {
                match resize_image(image_url, detail, self.mode) {
                    Ok(Some(prepared)) => Some(Cow::Owned(ImageReference::Inline {
                        image_url: prepared.into_data_url(),
                    })),
                    Ok(None) => Some(Cow::Borrowed(image)),
                    Err(error) => {
                        warn!(%error, "failed to prepare guardian review image");
                        None
                    }
                }
            }
            ImageReference::File { .. } => Some(Cow::Borrowed(image)),
        }
    }
}
