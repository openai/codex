use std::path::Path;
use std::sync::Arc;

use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::ReviewStoryAnchor;
use codex_app_server_protocol::ReviewStoryAnchorKind;
use codex_app_server_protocol::ReviewStoryListParams;
use codex_app_server_protocol::ReviewStoryListResponse;
use codex_app_server_protocol::ReviewStoryReadParams;
use codex_app_server_protocol::ReviewStoryReadResponse;
use codex_app_server_protocol::ReviewStorySnapshot;
use codex_app_server_protocol::ReviewStorySnapshotStatus;
use codex_app_server_protocol::ReviewStorySnapshotSummary;
use codex_app_server_protocol::ReviewStorySnapshotUpdatedNotification;
use codex_app_server_protocol::ReviewStoryStartParams;
use codex_app_server_protocol::ReviewStoryStartResponse;
use codex_app_server_protocol::ReviewStoryStep;
use codex_app_server_protocol::ReviewStoryStepReadiness;
use codex_app_server_protocol::ReviewTarget;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::ThreadManager;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SubAgentSource;
use codex_rollout::state_db::StateDbHandle;
use codex_state::ReviewStoryRecord;
use codex_state::ReviewStorySummaryRecord;
use serde::Deserialize;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashMap;
use std::collections::HashSet;
use tokio::process::Command;
use uuid::Uuid;

use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;

#[derive(Clone)]
pub(crate) struct ReviewStoryRequestProcessor {
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    state_db: Option<StateDbHandle>,
}

impl ReviewStoryRequestProcessor {
    pub(crate) fn new(
        thread_manager: Arc<ThreadManager>,
        outgoing: Arc<OutgoingMessageSender>,
        state_db: Option<StateDbHandle>,
    ) -> Self {
        Self {
            thread_manager,
            outgoing,
            state_db,
        }
    }

    pub(crate) async fn start(
        &self,
        _request_id: &ConnectionRequestId,
        params: ReviewStoryStartParams,
    ) -> Result<Option<ClientResponsePayload>, codex_app_server_protocol::JSONRPCErrorError> {
        let ReviewStoryStartParams { thread_id, target } = params;
        let (thread_uuid, thread) = self.load_thread(&thread_id).await?;
        let state_db = self.require_state_db()?;
        let cwd = thread.config_snapshot().await.cwd;
        let evidence = collect_evidence(cwd.as_path(), &target).await?;
        let model_story = generate_model_story(&thread, &target, &evidence).await;
        let story_snapshot_id = Uuid::now_v7().to_string();
        let now = chrono::Utc::now().timestamp();
        let snapshot = build_snapshot(
            story_snapshot_id.clone(),
            thread_id.clone(),
            target,
            evidence,
            model_story,
            now,
            /*previous_story_snapshot_id*/ None,
        );
        let record = snapshot_record(&snapshot)?;
        state_db
            .review_stories()
            .upsert_snapshot(record)
            .await
            .map_err(|err| internal_error(format!("failed to store review story: {err}")))?;

        self.outgoing
            .send_server_notification(ServerNotification::ReviewStorySnapshotUpdated(
                ReviewStorySnapshotUpdatedNotification {
                    thread_id: thread_uuid.to_string(),
                    snapshot: snapshot.clone(),
                },
            ))
            .await;

        let turn = build_story_turn(
            format!("review-story-{}", snapshot.story_snapshot_id),
            "Create a review story for the selected changes.",
        );
        Ok(Some(
            ReviewStoryStartResponse {
                turn,
                story_snapshot_id,
                snapshot,
            }
            .into(),
        ))
    }

    pub(crate) async fn read(
        &self,
        params: ReviewStoryReadParams,
    ) -> Result<Option<ClientResponsePayload>, codex_app_server_protocol::JSONRPCErrorError> {
        let ReviewStoryReadParams {
            thread_id,
            story_snapshot_id,
        } = params;
        let thread_id = parse_thread_id(&thread_id)?;
        let state_db = self.require_state_db()?;
        let record = state_db
            .review_stories()
            .get_snapshot(thread_id, &story_snapshot_id)
            .await
            .map_err(|err| internal_error(format!("failed to read review story: {err}")))?;
        let snapshot = record
            .map(|record| serde_json::from_value(record.snapshot_json))
            .transpose()
            .map_err(|err| internal_error(format!("failed to decode review story: {err}")))?;
        Ok(Some(ReviewStoryReadResponse { snapshot }.into()))
    }

    pub(crate) async fn list(
        &self,
        params: ReviewStoryListParams,
    ) -> Result<Option<ClientResponsePayload>, codex_app_server_protocol::JSONRPCErrorError> {
        let ReviewStoryListParams {
            thread_id,
            cursor,
            limit,
        } = params;
        let thread_id = parse_thread_id(&thread_id)?;
        let state_db = self.require_state_db()?;
        let (records, next_cursor) = state_db
            .review_stories()
            .list_snapshots(thread_id, cursor, limit)
            .await
            .map_err(|err| internal_error(format!("failed to list review stories: {err}")))?;
        let data = records
            .into_iter()
            .map(summary_from_record)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(ReviewStoryListResponse { data, next_cursor }.into()))
    }

    async fn load_thread(
        &self,
        thread_id: &str,
    ) -> Result<
        (ThreadId, Arc<codex_core::CodexThread>),
        codex_app_server_protocol::JSONRPCErrorError,
    > {
        let thread_id = parse_thread_id(thread_id)?;
        let thread = self
            .thread_manager
            .get_thread(thread_id)
            .await
            .map_err(|_| invalid_request(format!("thread not found: {thread_id}")))?;
        Ok((thread_id, thread))
    }

    fn require_state_db(
        &self,
    ) -> Result<&StateDbHandle, codex_app_server_protocol::JSONRPCErrorError> {
        self.state_db
            .as_ref()
            .ok_or_else(|| internal_error("review story storage is unavailable"))
    }
}

struct StoryEvidence {
    source_fingerprint: String,
    anchors: Vec<ReviewStoryAnchor>,
}

struct ModelStoryResult {
    output: Option<ModelReviewStoryOutput>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelReviewStoryOutput {
    title: String,
    overview: String,
    steps: Vec<ModelReviewStoryStep>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelReviewStoryStep {
    title: String,
    goal: String,
    summary: String,
    dependency_rationale: String,
    anchor_ids: Vec<String>,
    review_focus: Vec<String>,
}

async fn collect_evidence(
    cwd: &Path,
    target: &ReviewTarget,
) -> Result<StoryEvidence, codex_app_server_protocol::JSONRPCErrorError> {
    let diff = match target {
        ReviewTarget::UncommittedChanges => {
            git_output(
                cwd,
                &["diff", "--no-ext-diff", "--find-renames", "HEAD", "--"],
            )
            .await?
        }
        ReviewTarget::BaseBranch { branch } => {
            let range = format!("{branch}...HEAD");
            git_output(
                cwd,
                &[
                    "diff",
                    "--no-ext-diff",
                    "--find-renames",
                    range.as_str(),
                    "--",
                ],
            )
            .await?
        }
        ReviewTarget::Commit { sha, .. } => {
            git_output(
                cwd,
                &[
                    "show",
                    "--format=",
                    "--no-ext-diff",
                    "--find-renames",
                    sha,
                    "--",
                ],
            )
            .await?
        }
        ReviewTarget::Custom { .. } => {
            return Err(invalid_request(
                "reviewStory/start requires a concrete source: uncommitted changes, base branch, or commit",
            ));
        }
    };
    let source_fingerprint = source_fingerprint(target, &diff)?;
    let anchors = anchors_from_diff(&diff);
    Ok(StoryEvidence {
        source_fingerprint,
        anchors,
    })
}

async fn generate_model_story(
    thread: &codex_core::CodexThread,
    target: &ReviewTarget,
    evidence: &StoryEvidence,
) -> ModelStoryResult {
    if evidence.anchors.is_empty() {
        return ModelStoryResult {
            output: None,
            error: None,
        };
    }
    let prompt = review_story_prompt(target, evidence);
    let schema = review_story_output_schema();
    match thread
        .run_structured_model_task(
            prompt,
            REVIEW_STORY_MODEL_INSTRUCTIONS.to_string(),
            schema,
            SubAgentSource::Other("review_story".to_string()),
        )
        .await
    {
        Ok(Some(output_text)) => match parse_model_review_story_output(&output_text) {
            Ok(output) => ModelStoryResult {
                output: Some(output),
                error: None,
            },
            Err(err) => ModelStoryResult {
                output: None,
                error: Some(format!("model returned an invalid review story: {err}")),
            },
        },
        Ok(None) => ModelStoryResult {
            output: None,
            error: Some("model did not return a review story".to_string()),
        },
        Err(err) => ModelStoryResult {
            output: None,
            error: Some(format!("failed to generate review story: {err}")),
        },
    }
}

const REVIEW_STORY_MODEL_INSTRUCTIONS: &str = r#"You create review stories for code changes.

Group the supplied change anchors into a small sequence of cohesive logic steps. Each step should help a reviewer understand why the change exists, what changed, and how that step depends on earlier steps. Use only the provided anchor ids. Do not invent files, commits, or anchor ids. Prefer semantic dependency order over file order when the evidence supports it."#;

fn review_story_prompt(target: &ReviewTarget, evidence: &StoryEvidence) -> String {
    let mut prompt = String::new();
    prompt.push_str("Create a dependency-aware review story for these changes.\n\n");
    prompt.push_str("Target:\n");
    prompt.push_str(&target_description(target));
    prompt.push_str("\n\nSource fingerprint:\n");
    prompt.push_str(&evidence.source_fingerprint);
    prompt.push_str("\n\nAnchors:\n");
    for anchor in &evidence.anchors {
        prompt.push_str("\n---\n");
        prompt.push_str("anchorId: ");
        prompt.push_str(&anchor.anchor_id);
        prompt.push_str("\nfilePath: ");
        prompt.push_str(&anchor.file_path);
        prompt.push_str("\nchangeKind: ");
        prompt.push_str(anchor_kind_as_str(anchor.change_kind));
        prompt.push_str("\nsummary: ");
        prompt.push_str(&anchor.summary);
        prompt.push_str("\ndiff:\n");
        prompt.push_str(&truncate_for_prompt(
            &anchor.diff,
            /*max_chars*/ 12_000,
        ));
        prompt.push('\n');
    }
    prompt.push_str(
        "\nReturn JSON only. Every anchor id above should appear in exactly one step unless the diff is empty. Keep steps reviewer-sized and cohesive.",
    );
    truncate_for_prompt(&prompt, /*max_chars*/ 120_000)
}

fn target_description(target: &ReviewTarget) -> String {
    match target {
        ReviewTarget::UncommittedChanges => "uncommitted changes".to_string(),
        ReviewTarget::BaseBranch { branch } => format!("diff from base branch {branch}"),
        ReviewTarget::Commit { sha, title } => {
            let title = title.as_deref().unwrap_or("untitled commit");
            format!("commit {sha}: {title}")
        }
        ReviewTarget::Custom { instructions } => {
            format!("custom review target: {instructions}")
        }
    }
}

fn anchor_kind_as_str(kind: ReviewStoryAnchorKind) -> &'static str {
    match kind {
        ReviewStoryAnchorKind::Added => "added",
        ReviewStoryAnchorKind::Modified => "modified",
        ReviewStoryAnchorKind::Deleted => "deleted",
        ReviewStoryAnchorKind::Renamed => "renamed",
        ReviewStoryAnchorKind::Copied => "copied",
        ReviewStoryAnchorKind::Unknown => "unknown",
    }
}

fn truncate_for_prompt(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut truncated = text.chars().take(max_chars).collect::<String>();
    truncated.push_str("\n[truncated]");
    truncated
}

fn review_story_output_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "title": {
                "type": "string"
            },
            "overview": {
                "type": "string"
            },
            "steps": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "title": {
                            "type": "string"
                        },
                        "goal": {
                            "type": "string"
                        },
                        "summary": {
                            "type": "string"
                        },
                        "dependencyRationale": {
                            "type": "string"
                        },
                        "anchorIds": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        },
                        "reviewFocus": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        }
                    },
                    "required": [
                        "title",
                        "goal",
                        "summary",
                        "dependencyRationale",
                        "anchorIds",
                        "reviewFocus"
                    ]
                }
            }
        },
        "required": [
            "title",
            "overview",
            "steps"
        ]
    })
}

fn parse_model_review_story_output(
    text: &str,
) -> Result<ModelReviewStoryOutput, serde_json::Error> {
    if let Ok(output) = serde_json::from_str::<ModelReviewStoryOutput>(text) {
        return Ok(output);
    }
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}'))
        && start < end
        && let Some(slice) = text.get(start..=end)
    {
        return serde_json::from_str::<ModelReviewStoryOutput>(slice);
    }
    serde_json::from_str::<ModelReviewStoryOutput>(text)
}

async fn git_output(
    cwd: &Path,
    args: &[&str],
) -> Result<String, codex_app_server_protocol::JSONRPCErrorError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .map_err(|err| internal_error(format!("failed to run git: {err}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(invalid_request(format!("failed to read changes: {stderr}")));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn source_fingerprint(
    target: &ReviewTarget,
    diff: &str,
) -> Result<String, codex_app_server_protocol::JSONRPCErrorError> {
    let mut hasher = Sha256::new();
    let target_json = serde_json::to_vec(target)
        .map_err(|err| internal_error(format!("failed to encode review story target: {err}")))?;
    hasher.update(target_json);
    const FINGERPRINT_SEPARATOR: [u8; 1] = [0];
    hasher.update(FINGERPRINT_SEPARATOR);
    hasher.update(diff.as_bytes());
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn anchors_from_diff(diff: &str) -> Vec<ReviewStoryAnchor> {
    let mut anchors = Vec::new();
    let mut current_header: Option<String> = None;
    let mut current_diff = String::new();
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            if let Some(header) = current_header.take() {
                anchors.push(anchor_from_file_diff(anchors.len(), &header, &current_diff));
                current_diff.clear();
            }
            current_header = Some(line.to_string());
        }
        if current_header.is_some() {
            current_diff.push_str(line);
            current_diff.push('\n');
        }
    }
    if let Some(header) = current_header {
        anchors.push(anchor_from_file_diff(anchors.len(), &header, &current_diff));
    }
    anchors
}

fn anchor_from_file_diff(index: usize, header: &str, diff: &str) -> ReviewStoryAnchor {
    let file_path = file_path_from_diff_header(header).unwrap_or_else(|| "unknown".to_string());
    let change_kind = change_kind_from_file_diff(diff);
    let summary = match change_kind {
        ReviewStoryAnchorKind::Added => format!("Added {file_path}"),
        ReviewStoryAnchorKind::Deleted => format!("Deleted {file_path}"),
        ReviewStoryAnchorKind::Renamed => format!("Renamed {file_path}"),
        ReviewStoryAnchorKind::Copied => format!("Copied {file_path}"),
        ReviewStoryAnchorKind::Modified => format!("Modified {file_path}"),
        ReviewStoryAnchorKind::Unknown => format!("Changed {file_path}"),
    };
    ReviewStoryAnchor {
        anchor_id: format!("anchor-{}", index + 1),
        file_path,
        change_kind,
        summary,
        diff: diff.to_string(),
    }
}

fn file_path_from_diff_header(header: &str) -> Option<String> {
    let mut parts = header.split_whitespace();
    let _diff = parts.next()?;
    let _git = parts.next()?;
    let _old = parts.next()?;
    let new = parts.next()?;
    Some(new.strip_prefix("b/").unwrap_or(new).to_string())
}

fn change_kind_from_file_diff(diff: &str) -> ReviewStoryAnchorKind {
    if diff.contains("\nnew file mode ") {
        ReviewStoryAnchorKind::Added
    } else if diff.contains("\ndeleted file mode ") {
        ReviewStoryAnchorKind::Deleted
    } else if diff.contains("\nrename from ") {
        ReviewStoryAnchorKind::Renamed
    } else if diff.contains("\ncopy from ") {
        ReviewStoryAnchorKind::Copied
    } else if diff.is_empty() {
        ReviewStoryAnchorKind::Unknown
    } else {
        ReviewStoryAnchorKind::Modified
    }
}

fn build_snapshot(
    story_snapshot_id: String,
    thread_id: String,
    target: ReviewTarget,
    evidence: StoryEvidence,
    model_story: ModelStoryResult,
    created_at: i64,
    previous_story_snapshot_id: Option<String>,
) -> ReviewStorySnapshot {
    let fallback_title = title_for_target(&target);
    let (title, overview, steps, status) = if evidence.anchors.is_empty() {
        (
            fallback_title,
            "No changes were detected for this review story source.".to_string(),
            steps_from_anchors(&evidence.anchors),
            ReviewStorySnapshotStatus::Ready,
        )
    } else if let Some(output) = model_story.output {
        let (steps, validation_error) = steps_from_model_output(output.steps, &evidence.anchors);
        let status = if validation_error.is_some() {
            ReviewStorySnapshotStatus::Partial
        } else {
            ReviewStorySnapshotStatus::Ready
        };
        let overview = if let Some(error) = validation_error {
            format!("{}\n\n{error}", output.overview)
        } else {
            output.overview
        };
        (output.title, overview, steps, status)
    } else {
        let overview = model_story.error.map_or_else(
            || {
                format!(
                    "This story groups {} changed file(s) into review steps based on the local change map.",
                    evidence.anchors.len()
                )
            },
            |error| format!("Model story generation did not complete. Falling back to file-level steps. {error}"),
        );
        (
            fallback_title,
            overview,
            steps_from_anchors(&evidence.anchors),
            ReviewStorySnapshotStatus::Partial,
        )
    };
    ReviewStorySnapshot {
        story_snapshot_id,
        thread_id,
        title,
        overview,
        target,
        source_fingerprint: evidence.source_fingerprint,
        status,
        created_at,
        updated_at: created_at,
        previous_story_snapshot_id,
        stale: false,
        steps,
        anchors: evidence.anchors,
    }
}

fn title_for_target(target: &ReviewTarget) -> String {
    match target {
        ReviewTarget::UncommittedChanges => "Review story for uncommitted changes".to_string(),
        ReviewTarget::BaseBranch { branch } => format!("Review story against {branch}"),
        ReviewTarget::Commit { title, sha } => title
            .clone()
            .unwrap_or_else(|| format!("Review story for commit {sha}")),
        ReviewTarget::Custom { .. } => "Review story".to_string(),
    }
}

fn steps_from_anchors(anchors: &[ReviewStoryAnchor]) -> Vec<ReviewStoryStep> {
    if anchors.is_empty() {
        return vec![ReviewStoryStep {
            step_id: "step-1".to_string(),
            index: 1,
            title: "No changes".to_string(),
            goal: "Confirm that the selected source has no diff to review.".to_string(),
            summary: "The source currently produces an empty diff.".to_string(),
            dependency_rationale: "There are no file-level dependencies to order.".to_string(),
            anchor_ids: Vec::new(),
            review_focus: Vec::new(),
            readiness: ReviewStoryStepReadiness::Ready,
            error: None,
        }];
    }

    anchors
        .iter()
        .enumerate()
        .map(|(index, anchor)| ReviewStoryStep {
            step_id: format!("step-{}", index + 1),
            index: (index + 1) as u32,
            title: anchor.summary.clone(),
            goal: format!("Understand the purpose of changes in {}.", anchor.file_path),
            summary: summarize_anchor(anchor),
            dependency_rationale: "Placed according to the file-level evidence order from the diff; enrichment can refine this with semantic dependencies.".to_string(),
            anchor_ids: vec![anchor.anchor_id.clone()],
            review_focus: vec![format!("Check how {} fits the overall change story.", anchor.file_path)],
            readiness: ReviewStoryStepReadiness::Ready,
            error: None,
        })
        .collect()
}

fn steps_from_model_output(
    model_steps: Vec<ModelReviewStoryStep>,
    anchors: &[ReviewStoryAnchor],
) -> (Vec<ReviewStoryStep>, Option<String>) {
    let anchors_by_id = anchors
        .iter()
        .map(|anchor| (anchor.anchor_id.as_str(), anchor))
        .collect::<HashMap<_, _>>();
    let mut seen_anchor_ids = HashSet::new();
    let mut invalid_anchor_ids = Vec::new();
    let mut steps = Vec::new();
    for model_step in model_steps {
        let mut anchor_ids = Vec::new();
        for anchor_id in model_step.anchor_ids {
            if anchors_by_id.contains_key(anchor_id.as_str())
                && seen_anchor_ids.insert(anchor_id.clone())
            {
                anchor_ids.push(anchor_id);
            } else if !anchors_by_id.contains_key(anchor_id.as_str()) {
                invalid_anchor_ids.push(anchor_id);
            }
        }
        if anchor_ids.is_empty() {
            continue;
        }
        let index = steps.len() + 1;
        steps.push(ReviewStoryStep {
            step_id: format!("step-{index}"),
            index: index as u32,
            title: model_step.title,
            goal: model_step.goal,
            summary: model_step.summary,
            dependency_rationale: model_step.dependency_rationale,
            anchor_ids,
            review_focus: model_step.review_focus,
            readiness: ReviewStoryStepReadiness::Ready,
            error: None,
        });
    }

    let missing_anchors = anchors
        .iter()
        .filter(|anchor| !seen_anchor_ids.contains(&anchor.anchor_id))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_anchors.is_empty() {
        let index = steps.len() + 1;
        steps.push(ReviewStoryStep {
            step_id: format!("step-{index}"),
            index: index as u32,
            title: "Remaining changes".to_string(),
            goal: "Review changes the model did not place into a story step.".to_string(),
            summary: format!(
                "Covers {} anchor(s) that were omitted from the model-authored story.",
                missing_anchors.len()
            ),
            dependency_rationale:
                "Appended after model-authored steps to preserve full diff coverage.".to_string(),
            anchor_ids: missing_anchors
                .iter()
                .map(|anchor| anchor.anchor_id.clone())
                .collect(),
            review_focus: missing_anchors
                .iter()
                .map(|anchor| format!("Check {} independently.", anchor.file_path))
                .collect(),
            readiness: ReviewStoryStepReadiness::Ready,
            error: None,
        });
    }

    if steps.is_empty() {
        return (
            steps_from_anchors(anchors),
            Some("The model did not return any usable review story steps.".to_string()),
        );
    }

    let mut validation_notes = Vec::new();
    if !missing_anchors.is_empty() {
        validation_notes.push(format!(
            "The model omitted {} anchor(s); Codex appended a coverage step.",
            missing_anchors.len()
        ));
    }
    if !invalid_anchor_ids.is_empty() {
        validation_notes.push(format!(
            "The model referenced unknown anchor id(s): {}.",
            invalid_anchor_ids.join(", ")
        ));
    }
    if validation_notes.is_empty() {
        (steps, None)
    } else {
        (steps, Some(validation_notes.join(" ")))
    }
}

fn summarize_anchor(anchor: &ReviewStoryAnchor) -> String {
    let additions = anchor
        .diff
        .lines()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
        .count();
    let deletions = anchor
        .diff
        .lines()
        .filter(|line| line.starts_with('-') && !line.starts_with("---"))
        .count();
    format!(
        "{} with {additions} added line(s) and {deletions} removed line(s).",
        anchor.summary
    )
}

fn build_story_turn(turn_id: String, display_text: &str) -> Turn {
    Turn {
        id: turn_id.clone(),
        items: vec![ThreadItem::UserMessage {
            id: turn_id,
            content: vec![V2UserInput::Text {
                text: display_text.to_string(),
                text_elements: Vec::new(),
            }],
        }],
        items_view: TurnItemsView::NotLoaded,
        error: None,
        status: TurnStatus::Completed,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    }
}

fn snapshot_record(
    snapshot: &ReviewStorySnapshot,
) -> Result<ReviewStoryRecord, codex_app_server_protocol::JSONRPCErrorError> {
    let target_json = serde_json::to_value(&snapshot.target)
        .map_err(|err| internal_error(format!("failed to encode review story target: {err}")))?;
    let snapshot_json = serde_json::to_value(snapshot)
        .map_err(|err| internal_error(format!("failed to encode review story snapshot: {err}")))?;
    Ok(ReviewStoryRecord {
        story_snapshot_id: snapshot.story_snapshot_id.clone(),
        thread_id: snapshot.thread_id.clone(),
        source_fingerprint: snapshot.source_fingerprint.clone(),
        status: status_as_str(snapshot.status).to_string(),
        title: snapshot.title.clone(),
        step_count: snapshot.steps.len() as i64,
        target_json,
        snapshot_json,
        previous_story_snapshot_id: snapshot.previous_story_snapshot_id.clone(),
        created_at_ms: snapshot.created_at * 1000,
        updated_at_ms: snapshot.updated_at * 1000,
    })
}

fn summary_from_record(
    record: ReviewStorySummaryRecord,
) -> Result<ReviewStorySnapshotSummary, codex_app_server_protocol::JSONRPCErrorError> {
    let target = serde_json::from_value(record.target_json)
        .map_err(|err| internal_error(format!("failed to decode review story target: {err}")))?;
    Ok(ReviewStorySnapshotSummary {
        story_snapshot_id: record.story_snapshot_id,
        thread_id: record.thread_id,
        title: record.title,
        target,
        source_fingerprint: record.source_fingerprint,
        status: status_from_str(record.status.as_str()),
        created_at: record.created_at_ms / 1000,
        updated_at: record.updated_at_ms / 1000,
        previous_story_snapshot_id: record.previous_story_snapshot_id,
        step_count: record.step_count.max(/*other*/ 0) as u32,
    })
}

fn status_as_str(status: ReviewStorySnapshotStatus) -> &'static str {
    match status {
        ReviewStorySnapshotStatus::Building => "building",
        ReviewStorySnapshotStatus::Ready => "ready",
        ReviewStorySnapshotStatus::Partial => "partial",
        ReviewStorySnapshotStatus::Failed => "failed",
    }
}

fn status_from_str(status: &str) -> ReviewStorySnapshotStatus {
    match status {
        "building" => ReviewStorySnapshotStatus::Building,
        "partial" => ReviewStorySnapshotStatus::Partial,
        "failed" => ReviewStorySnapshotStatus::Failed,
        "ready" => ReviewStorySnapshotStatus::Ready,
        _ => ReviewStorySnapshotStatus::Failed,
    }
}

fn parse_thread_id(
    thread_id: &str,
) -> Result<ThreadId, codex_app_server_protocol::JSONRPCErrorError> {
    ThreadId::from_string(thread_id)
        .map_err(|err| invalid_request(format!("invalid thread id: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn anchors_split_diff_by_file() {
        let diff = "\
diff --git a/src/lib.rs b/src/lib.rs
index 111..222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1 +1 @@
-old
+new
diff --git a/src/main.rs b/src/main.rs
new file mode 100644
--- /dev/null
+++ b/src/main.rs
@@ -0,0 +1 @@
+fn main() {}
";

        let anchors = anchors_from_diff(diff);

        assert_eq!(
            anchors,
            vec![
                ReviewStoryAnchor {
                    anchor_id: "anchor-1".to_string(),
                    file_path: "src/lib.rs".to_string(),
                    change_kind: ReviewStoryAnchorKind::Modified,
                    summary: "Modified src/lib.rs".to_string(),
                    diff: "diff --git a/src/lib.rs b/src/lib.rs\nindex 111..222 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
                },
                ReviewStoryAnchor {
                    anchor_id: "anchor-2".to_string(),
                    file_path: "src/main.rs".to_string(),
                    change_kind: ReviewStoryAnchorKind::Added,
                    summary: "Added src/main.rs".to_string(),
                    diff: "diff --git a/src/main.rs b/src/main.rs\nnew file mode 100644\n--- /dev/null\n+++ b/src/main.rs\n@@ -0,0 +1 @@\n+fn main() {}\n".to_string(),
                },
            ]
        );
    }

    #[test]
    fn model_output_groups_anchors_into_cohesive_steps() {
        let anchors = sample_anchors();
        let model_steps = vec![ModelReviewStoryStep {
            title: "Introduce review story state".to_string(),
            goal: "Create the storage and protocol foundation.".to_string(),
            summary: "Adds reusable state and API shapes for review stories.".to_string(),
            dependency_rationale: "The UI depends on the state and protocol contract.".to_string(),
            anchor_ids: vec!["anchor-1".to_string(), "anchor-2".to_string()],
            review_focus: vec!["Confirm the persisted snapshot is reusable.".to_string()],
        }];

        let (steps, validation_error) = steps_from_model_output(model_steps, &anchors);

        assert_eq!(validation_error, None);
        assert_eq!(
            steps,
            vec![ReviewStoryStep {
                step_id: "step-1".to_string(),
                index: 1,
                title: "Introduce review story state".to_string(),
                goal: "Create the storage and protocol foundation.".to_string(),
                summary: "Adds reusable state and API shapes for review stories.".to_string(),
                dependency_rationale: "The UI depends on the state and protocol contract."
                    .to_string(),
                anchor_ids: vec!["anchor-1".to_string(), "anchor-2".to_string()],
                review_focus: vec!["Confirm the persisted snapshot is reusable.".to_string()],
                readiness: ReviewStoryStepReadiness::Ready,
                error: None,
            }]
        );
    }

    #[test]
    fn model_output_appends_missing_anchors_for_full_coverage() {
        let anchors = sample_anchors();
        let model_steps = vec![ModelReviewStoryStep {
            title: "Add protocol".to_string(),
            goal: "Define the API contract.".to_string(),
            summary: "Adds the protocol file.".to_string(),
            dependency_rationale: "Protocol comes first.".to_string(),
            anchor_ids: vec!["anchor-1".to_string(), "missing-anchor".to_string()],
            review_focus: vec!["Check wire shape.".to_string()],
        }];

        let (steps, validation_error) = steps_from_model_output(model_steps, &anchors);

        assert_eq!(
            validation_error,
            Some(
                "The model omitted 1 anchor(s); Codex appended a coverage step. The model referenced unknown anchor id(s): missing-anchor."
                    .to_string()
            )
        );
        assert_eq!(
            steps,
            vec![
                ReviewStoryStep {
                    step_id: "step-1".to_string(),
                    index: 1,
                    title: "Add protocol".to_string(),
                    goal: "Define the API contract.".to_string(),
                    summary: "Adds the protocol file.".to_string(),
                    dependency_rationale: "Protocol comes first.".to_string(),
                    anchor_ids: vec!["anchor-1".to_string()],
                    review_focus: vec!["Check wire shape.".to_string()],
                    readiness: ReviewStoryStepReadiness::Ready,
                    error: None,
                },
                ReviewStoryStep {
                    step_id: "step-2".to_string(),
                    index: 2,
                    title: "Remaining changes".to_string(),
                    goal: "Review changes the model did not place into a story step.".to_string(),
                    summary: "Covers 1 anchor(s) that were omitted from the model-authored story."
                        .to_string(),
                    dependency_rationale:
                        "Appended after model-authored steps to preserve full diff coverage."
                            .to_string(),
                    anchor_ids: vec!["anchor-2".to_string()],
                    review_focus: vec![
                        "Check codex-rs/tui/src/story.rs independently.".to_string()
                    ],
                    readiness: ReviewStoryStepReadiness::Ready,
                    error: None,
                },
            ]
        );
    }

    fn sample_anchors() -> Vec<ReviewStoryAnchor> {
        vec![
            ReviewStoryAnchor {
                anchor_id: "anchor-1".to_string(),
                file_path: "codex-rs/app-server-protocol/src/protocol/v2/review_story.rs"
                    .to_string(),
                change_kind: ReviewStoryAnchorKind::Added,
                summary: "Added protocol".to_string(),
                diff: "+protocol".to_string(),
            },
            ReviewStoryAnchor {
                anchor_id: "anchor-2".to_string(),
                file_path: "codex-rs/tui/src/story.rs".to_string(),
                change_kind: ReviewStoryAnchorKind::Added,
                summary: "Added TUI".to_string(),
                diff: "+tui".to_string(),
            },
        ]
    }
}
