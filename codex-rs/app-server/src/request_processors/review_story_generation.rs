use std::collections::HashMap;
use std::sync::Arc;

use codex_app_server_protocol::ReviewStorySnapshot;
use codex_app_server_protocol::ReviewStorySnapshotStatus;
use codex_app_server_protocol::ReviewStorySnapshotUpdatedNotification;
use codex_app_server_protocol::ReviewStoryStepReadiness;
use codex_app_server_protocol::ServerNotification;
use codex_protocol::protocol::SubAgentSource;
use codex_rollout::state_db::StateDbHandle;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use serde::Deserialize;
use serde_json::json;

use crate::outgoing_message::OutgoingMessageSender;

use super::review_story_processor::snapshot_record;

const ENRICHMENT_BATCH_SIZE: usize = 2;
const ENRICHMENT_CONCURRENCY: usize = 2;

pub(super) fn spawn_enrichment(
    thread: Arc<codex_core::CodexThread>,
    snapshot: ReviewStorySnapshot,
    outline_degraded: bool,
    state_db: StateDbHandle,
    outgoing: Arc<OutgoingMessageSender>,
) {
    tokio::spawn(async move {
        run_enrichment(thread, snapshot, outline_degraded, state_db, outgoing).await;
    });
}

async fn run_enrichment(
    thread: Arc<codex_core::CodexThread>,
    mut snapshot: ReviewStorySnapshot,
    outline_degraded: bool,
    state_db: StateDbHandle,
    outgoing: Arc<OutgoingMessageSender>,
) {
    let batches = snapshot
        .steps
        .chunks(/*chunk_size*/ ENRICHMENT_BATCH_SIZE)
        .map(|steps| {
            steps
                .iter()
                .map(|step| step.step_id.clone())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for pending_batches in batches.chunks(/*chunk_size*/ ENRICHMENT_CONCURRENCY) {
        for batch in pending_batches {
            mark_batch_enriching(&mut snapshot, batch, outline_degraded);
            if !publish_snapshot(&state_db, &outgoing, &mut snapshot).await {
                return;
            }
        }

        let mut running = FuturesUnordered::new();
        for batch in pending_batches.iter().cloned() {
            let thread = Arc::clone(&thread);
            let prompt_snapshot = snapshot.clone();
            running.push(async move {
                let result = generate_batch_enrichment(&thread, &prompt_snapshot, &batch).await;
                (batch, result)
            });
        }

        while let Some((batch, result)) = running.next().await {
            apply_batch_result(&mut snapshot, &batch, result, outline_degraded);
            if !publish_snapshot(&state_db, &outgoing, &mut snapshot).await {
                return;
            }
        }
    }
}

async fn publish_snapshot(
    state_db: &StateDbHandle,
    outgoing: &OutgoingMessageSender,
    snapshot: &mut ReviewStorySnapshot,
) -> bool {
    snapshot.updated_at = chrono::Utc::now().timestamp();
    let record = match snapshot_record(snapshot) {
        Ok(record) => record,
        Err(err) => {
            tracing::warn!("failed to encode review story enrichment snapshot: {err:?}");
            return false;
        }
    };
    if let Err(err) = state_db.review_stories().upsert_snapshot(record).await {
        tracing::warn!("failed to store review story enrichment snapshot: {err}");
        return false;
    }
    outgoing
        .send_server_notification(ServerNotification::ReviewStorySnapshotUpdated(
            ReviewStorySnapshotUpdatedNotification {
                thread_id: snapshot.thread_id.clone(),
                snapshot: snapshot.clone(),
            },
        ))
        .await;
    true
}

fn mark_batch_enriching(
    snapshot: &mut ReviewStorySnapshot,
    batch: &[String],
    outline_degraded: bool,
) {
    for step in &mut snapshot.steps {
        if batch.contains(&step.step_id) {
            step.readiness = ReviewStoryStepReadiness::Enriching;
            step.error = None;
        }
    }
    update_status(snapshot, outline_degraded);
}

fn apply_batch_result(
    snapshot: &mut ReviewStorySnapshot,
    batch: &[String],
    result: Result<ModelEnrichmentOutput, String>,
    outline_degraded: bool,
) {
    let result = result.and_then(|output| enrichment_by_step_id(batch, output));
    match result {
        Ok(mut enrichments) => {
            for step in &mut snapshot.steps {
                if let Some(enrichment) = enrichments.remove(&step.step_id) {
                    step.goal = enrichment.goal;
                    step.summary = enrichment.summary;
                    step.dependency_rationale = enrichment.dependency_rationale;
                    step.review_focus = enrichment.review_focus;
                    step.readiness = ReviewStoryStepReadiness::Ready;
                    step.error = None;
                }
            }
        }
        Err(error) => {
            for step in &mut snapshot.steps {
                if batch.contains(&step.step_id) {
                    step.readiness = ReviewStoryStepReadiness::Failed;
                    step.error = Some(error.clone());
                }
            }
        }
    }
    update_status(snapshot, outline_degraded);
}

fn update_status(snapshot: &mut ReviewStorySnapshot, outline_degraded: bool) {
    if snapshot.steps.iter().any(|step| {
        matches!(
            step.readiness,
            ReviewStoryStepReadiness::Outline | ReviewStoryStepReadiness::Enriching
        )
    }) {
        snapshot.status = ReviewStorySnapshotStatus::Building;
    } else if outline_degraded
        || snapshot
            .steps
            .iter()
            .any(|step| step.readiness == ReviewStoryStepReadiness::Failed)
    {
        snapshot.status = ReviewStorySnapshotStatus::Partial;
    } else {
        snapshot.status = ReviewStorySnapshotStatus::Ready;
    }
}

async fn generate_batch_enrichment(
    thread: &codex_core::CodexThread,
    snapshot: &ReviewStorySnapshot,
    batch: &[String],
) -> Result<ModelEnrichmentOutput, String> {
    let prompt = enrichment_prompt(snapshot, batch);
    match thread
        .run_structured_model_task(
            prompt,
            ENRICHMENT_MODEL_INSTRUCTIONS.to_string(),
            enrichment_output_schema(),
            SubAgentSource::Other("review_story_enrichment".to_string()),
        )
        .await
    {
        Ok(Some(output_text)) => parse_enrichment_output(&output_text)
            .map_err(|err| format!("model returned invalid step enrichment: {err}")),
        Ok(None) => Err("model did not return step enrichment".to_string()),
        Err(err) => Err(format!("failed to enrich story step: {err}")),
    }
}

const ENRICHMENT_MODEL_INSTRUCTIONS: &str = r#"You enrich ordered review story steps for code changes.

Write detailed reviewer-facing explanations only for the supplied step ids. Preserve each step's anchored scope: explain its changed evidence, its purpose, and its dependency on nearby steps without inventing files, anchors, or behavior."#;

fn enrichment_prompt(snapshot: &ReviewStorySnapshot, batch: &[String]) -> String {
    let anchors = snapshot
        .anchors
        .iter()
        .map(|anchor| (anchor.anchor_id.as_str(), anchor))
        .collect::<HashMap<_, _>>();
    let mut prompt = format!(
        "Enrich the selected steps in this review story.\n\nStory title: {}\nOverview: {}\n\n",
        snapshot.title, snapshot.overview
    );
    for step in snapshot
        .steps
        .iter()
        .filter(|step| batch.contains(&step.step_id))
    {
        prompt.push_str(&format!(
            "---\nstepId: {}\ntitle: {}\noutline goal: {}\nanchorIds: {}\n",
            step.step_id,
            step.title,
            step.goal,
            step.anchor_ids.join(", ")
        ));
        for anchor_id in &step.anchor_ids {
            if let Some(anchor) = anchors.get(anchor_id.as_str()) {
                prompt.push_str(&format!(
                    "anchorId: {}\nfilePath: {}\ndiff:\n{}\n",
                    anchor.anchor_id,
                    anchor.file_path,
                    truncate_for_prompt(&anchor.diff, /*max_chars*/ 12_000)
                ));
            }
        }
    }
    prompt.push_str(
        "\nReturn JSON only, with exactly one enriched object for each requested stepId.",
    );
    truncate_for_prompt(&prompt, /*max_chars*/ 80_000)
}

fn truncate_for_prompt(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut truncated = text.chars().take(max_chars).collect::<String>();
    truncated.push_str("\n[truncated]");
    truncated
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelEnrichmentOutput {
    steps: Vec<ModelEnrichmentStep>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelEnrichmentStep {
    step_id: String,
    goal: String,
    summary: String,
    dependency_rationale: String,
    review_focus: Vec<String>,
}

fn enrichment_by_step_id(
    batch: &[String],
    output: ModelEnrichmentOutput,
) -> Result<HashMap<String, ModelEnrichmentStep>, String> {
    let mut enrichments = HashMap::new();
    for enrichment in output.steps {
        if !batch.contains(&enrichment.step_id) {
            return Err(format!(
                "model referenced unrequested step id: {}",
                enrichment.step_id
            ));
        }
        let step_id = enrichment.step_id.clone();
        if enrichments.insert(step_id.clone(), enrichment).is_some() {
            return Err(format!("model returned duplicate step id: {step_id}"));
        }
    }
    if enrichments.len() != batch.len() {
        return Err("model omitted one or more requested step ids".to_string());
    }
    Ok(enrichments)
}

fn enrichment_output_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "steps": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "stepId": { "type": "string" },
                        "goal": { "type": "string" },
                        "summary": { "type": "string" },
                        "dependencyRationale": { "type": "string" },
                        "reviewFocus": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["stepId", "goal", "summary", "dependencyRationale", "reviewFocus"]
                }
            }
        },
        "required": ["steps"]
    })
}

fn parse_enrichment_output(text: &str) -> Result<ModelEnrichmentOutput, serde_json::Error> {
    if let Ok(output) = serde_json::from_str::<ModelEnrichmentOutput>(text) {
        return Ok(output);
    }
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}'))
        && start < end
        && let Some(slice) = text.get(start..=end)
    {
        return serde_json::from_str::<ModelEnrichmentOutput>(slice);
    }
    serde_json::from_str::<ModelEnrichmentOutput>(text)
}

#[cfg(test)]
mod tests {
    use codex_app_server_protocol::ReviewStoryAnchor;
    use codex_app_server_protocol::ReviewStoryAnchorKind;
    use codex_app_server_protocol::ReviewStoryStep;
    use codex_app_server_protocol::ReviewTarget;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn scheduling_changes_only_requested_steps_to_enriching() {
        let mut snapshot = sample_snapshot();

        mark_batch_enriching(
            &mut snapshot,
            &["step-1".to_string()],
            /*outline_degraded*/ false,
        );

        assert_eq!(snapshot.status, ReviewStorySnapshotStatus::Building);
        assert_eq!(
            snapshot.steps[0].readiness,
            ReviewStoryStepReadiness::Enriching
        );
        assert_eq!(
            snapshot.steps[1].readiness,
            ReviewStoryStepReadiness::Outline
        );
    }

    #[test]
    fn successful_batches_merge_without_losing_prior_completion() {
        let mut snapshot = sample_snapshot();
        mark_batch_enriching(
            &mut snapshot,
            &["step-1".to_string()],
            /*outline_degraded*/ false,
        );
        mark_batch_enriching(
            &mut snapshot,
            &["step-2".to_string()],
            /*outline_degraded*/ false,
        );

        apply_batch_result(
            &mut snapshot,
            &["step-2".to_string()],
            Ok(enrichment("step-2", "second rich summary")),
            /*outline_degraded*/ false,
        );
        apply_batch_result(
            &mut snapshot,
            &["step-1".to_string()],
            Ok(enrichment("step-1", "first rich summary")),
            /*outline_degraded*/ false,
        );

        assert_eq!(snapshot.status, ReviewStorySnapshotStatus::Ready);
        assert_eq!(snapshot.steps[0].summary, "first rich summary");
        assert_eq!(snapshot.steps[1].summary, "second rich summary");
        assert_eq!(snapshot.steps[0].readiness, ReviewStoryStepReadiness::Ready);
        assert_eq!(snapshot.steps[1].readiness, ReviewStoryStepReadiness::Ready);
    }

    #[test]
    fn failed_or_degraded_enrichment_finishes_partial() {
        let mut failed = sample_snapshot();
        mark_batch_enriching(
            &mut failed,
            &["step-1".to_string()],
            /*outline_degraded*/ false,
        );
        apply_batch_result(
            &mut failed,
            &["step-1".to_string()],
            Err("generation failed".to_string()),
            /*outline_degraded*/ false,
        );
        apply_batch_result(
            &mut failed,
            &["step-2".to_string()],
            Ok(enrichment("step-2", "ready")),
            /*outline_degraded*/ false,
        );
        assert_eq!(failed.status, ReviewStorySnapshotStatus::Partial);
        assert_eq!(failed.steps[0].readiness, ReviewStoryStepReadiness::Failed);

        let mut degraded = sample_snapshot();
        apply_batch_result(
            &mut degraded,
            &["step-1".to_string(), "step-2".to_string()],
            Ok(ModelEnrichmentOutput {
                steps: vec![
                    enriched_step("step-1", "first"),
                    enriched_step("step-2", "second"),
                ],
            }),
            /*outline_degraded*/ true,
        );
        assert_eq!(degraded.status, ReviewStorySnapshotStatus::Partial);
    }

    fn enrichment(step_id: &str, summary: &str) -> ModelEnrichmentOutput {
        ModelEnrichmentOutput {
            steps: vec![enriched_step(step_id, summary)],
        }
    }

    fn enriched_step(step_id: &str, summary: &str) -> ModelEnrichmentStep {
        ModelEnrichmentStep {
            step_id: step_id.to_string(),
            goal: format!("goal {step_id}"),
            summary: summary.to_string(),
            dependency_rationale: "rationale".to_string(),
            review_focus: vec!["focus".to_string()],
        }
    }

    fn sample_snapshot() -> ReviewStorySnapshot {
        ReviewStorySnapshot {
            story_snapshot_id: "story-1".to_string(),
            thread_id: "thread-1".to_string(),
            title: "Story".to_string(),
            overview: "Overview".to_string(),
            target: ReviewTarget::UncommittedChanges,
            source_fingerprint: "sha256:one".to_string(),
            status: ReviewStorySnapshotStatus::Building,
            created_at: 1,
            updated_at: 1,
            previous_story_snapshot_id: None,
            stale: false,
            steps: vec![step("step-1", /*index*/ 1), step("step-2", /*index*/ 2)],
            anchors: vec![ReviewStoryAnchor {
                anchor_id: "anchor-1".to_string(),
                file_path: "src/lib.rs".to_string(),
                change_kind: ReviewStoryAnchorKind::Modified,
                summary: "Modified src/lib.rs".to_string(),
                diff: "+line".to_string(),
            }],
        }
    }

    fn step(step_id: &str, index: u32) -> ReviewStoryStep {
        ReviewStoryStep {
            step_id: step_id.to_string(),
            index,
            title: step_id.to_string(),
            goal: "outline goal".to_string(),
            summary: "outline summary".to_string(),
            dependency_rationale: "outline rationale".to_string(),
            anchor_ids: vec!["anchor-1".to_string()],
            review_focus: vec!["outline focus".to_string()],
            readiness: ReviewStoryStepReadiness::Outline,
            error: None,
        }
    }
}
