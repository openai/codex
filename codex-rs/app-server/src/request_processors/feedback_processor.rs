use super::*;
#[cfg(target_os = "windows")]
use codex_feedback::WINDOWS_SANDBOX_LOG_ATTACHMENT_FILENAME;

#[derive(Clone)]
pub(crate) struct FeedbackRequestProcessor {
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    config: Arc<Config>,
    feedback: CodexFeedback,
    log_db: Option<LogDbLayer>,
    state_db: Option<StateDbHandle>,
}

impl FeedbackRequestProcessor {
    pub(crate) fn new(
        auth_manager: Arc<AuthManager>,
        thread_manager: Arc<ThreadManager>,
        config: Arc<Config>,
        feedback: CodexFeedback,
        log_db: Option<LogDbLayer>,
        state_db: Option<StateDbHandle>,
    ) -> Self {
        Self {
            auth_manager,
            thread_manager,
            config,
            feedback,
            log_db,
            state_db,
        }
    }

    pub(crate) async fn feedback_upload(
        &self,
        params: FeedbackUploadParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.upload_feedback_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    async fn upload_feedback_response(
        &self,
        params: FeedbackUploadParams,
    ) -> Result<FeedbackUploadResponse, JSONRPCErrorError> {
        if !self.config.feedback_enabled {
            return Err(invalid_request(
                "sending feedback is disabled by configuration",
            ));
        }

        let FeedbackUploadParams {
            classification,
            reason,
            thread_id,
            include_logs,
            extra_log_files,
            tags,
        } = params;
        let mut upload_tags = tags.unwrap_or_default();

        let conversation_id = match thread_id.as_deref() {
            Some(thread_id) => match ThreadId::from_string(thread_id) {
                Ok(conversation_id) => Some(conversation_id),
                Err(err) => return Err(invalid_request(format!("invalid thread id: {err}"))),
            },
            None => None,
        };

        if let Some(chatgpt_user_id) = self
            .auth_manager
            .auth_cached()
            .and_then(|auth| auth.get_chatgpt_user_id())
        {
            tracing::info!(target: "feedback_tags", chatgpt_user_id);
        }
        if let Some(account_id) = self
            .auth_manager
            .auth_cached()
            .and_then(|auth| auth.get_account_id())
        {
            tracing::info!(target: "feedback_tags", account_id);
        }
        let snapshot = self.feedback.snapshot(conversation_id);
        let thread_id = snapshot.thread_id.clone();
        let (feedback_thread_ids, sqlite_feedback_logs, state_db_ctx) = if include_logs {
            if let Some(log_db) = self.log_db.as_ref() {
                log_db.flush().await;
            }
            let state_db_ctx = self.state_db.clone();
            let feedback_thread_ids = match conversation_id {
                Some(conversation_id) => match self
                    .thread_manager
                    .list_agent_subtree_thread_ids(conversation_id)
                    .await
                {
                    Ok(thread_ids) => thread_ids,
                    Err(err) => {
                        warn!(
                            "failed to list feedback subtree for thread_id={conversation_id}: {err}"
                        );
                        let mut thread_ids = vec![conversation_id];
                        if let Some(state_db_ctx) = state_db_ctx.as_ref() {
                            for status in [
                                codex_state::DirectionalThreadSpawnEdgeStatus::Open,
                                codex_state::DirectionalThreadSpawnEdgeStatus::Closed,
                            ] {
                                match state_db_ctx
                                    .list_thread_spawn_descendants_with_status(
                                        conversation_id,
                                        status,
                                    )
                                    .await
                                {
                                    Ok(descendant_ids) => thread_ids.extend(descendant_ids),
                                    Err(err) => warn!(
                                        "failed to list persisted feedback subtree for thread_id={conversation_id}: {err}"
                                    ),
                                }
                            }
                        }
                        thread_ids
                    }
                },
                None => Vec::new(),
            };
            let sqlite_feedback_logs = if let Some(state_db_ctx) = state_db_ctx.as_ref()
                && !feedback_thread_ids.is_empty()
            {
                let thread_id_texts = feedback_thread_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                let thread_id_refs = thread_id_texts
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                match state_db_ctx
                    .query_feedback_logs_for_threads(&thread_id_refs)
                    .await
                {
                    Ok(logs) if logs.is_empty() => None,
                    Ok(logs) => Some(logs),
                    Err(err) => {
                        let thread_ids = thread_id_texts.join(", ");
                        warn!(
                            "failed to query feedback logs from sqlite for thread_ids=[{thread_ids}]: {err}"
                        );
                        None
                    }
                }
            } else {
                None
            };
            (feedback_thread_ids, sqlite_feedback_logs, state_db_ctx)
        } else {
            (Vec::new(), None, None)
        };

        let mut extra_attachments = Vec::new();
        let mut attachment_paths = Vec::new();
        let mut seen_attachment_paths = HashSet::new();
        if include_logs {
            for feedback_thread_id in &feedback_thread_ids {
                let Some(rollout_path) = self
                    .resolve_rollout_path(*feedback_thread_id, state_db_ctx.as_ref())
                    .await
                else {
                    continue;
                };
                if seen_attachment_paths.insert(rollout_path.clone()) {
                    match materialized_rollout_feedback_attachment(
                        self.config.codex_home.as_path(),
                        rollout_path.as_path(),
                    )
                    .await
                    {
                        Ok(Some(attachment)) => extra_attachments.push(attachment),
                        Ok(None) => {}
                        Err(err) => warn!(
                            "failed to materialize feedback rollout {}: {err}",
                            rollout_path.display()
                        ),
                    }
                    attachment_paths.push(FeedbackAttachmentPath {
                        path: rollout_path,
                        attachment_filename_override: None,
                    });
                }
            }
            if let Some(conversation_id) = conversation_id
                && let Ok(conversation) = self.thread_manager.get_thread(conversation_id).await
                && let Some(guardian_rollout_path) =
                    conversation.guardian_trunk_rollout_path().await
                && seen_attachment_paths.insert(guardian_rollout_path.clone())
            {
                attachment_paths.push(FeedbackAttachmentPath {
                    path: guardian_rollout_path,
                    attachment_filename_override: Some(auto_review_rollout_filename(
                        conversation_id,
                    )),
                });
            }
            if let Some(sandbox_log_attachment) =
                windows_sandbox_log_attachment(&self.config.codex_home)
                && seen_attachment_paths.insert(sandbox_log_attachment.path.clone())
            {
                attachment_paths.push(sandbox_log_attachment);
            }
        }
        if let Some(extra_log_files) = extra_log_files {
            for extra_log_file in extra_log_files {
                if seen_attachment_paths.insert(extra_log_file.clone()) {
                    attachment_paths.push(FeedbackAttachmentPath {
                        path: extra_log_file,
                        attachment_filename_override: None,
                    });
                }
            }
        }

        if include_logs
            && let Some(doctor_report) =
                super::feedback_doctor_report::doctor_feedback_report(&self.config).await
        {
            extra_attachments.push(doctor_report.attachment);
            for (key, value) in doctor_report.tags {
                upload_tags.entry(key).or_insert(value);
            }
        }

        let session_source = self.thread_manager.session_source();

        let upload_result = tokio::task::spawn_blocking(move || {
            let tags = (!upload_tags.is_empty()).then_some(&upload_tags);
            snapshot.upload_feedback(FeedbackUploadOptions {
                classification: &classification,
                reason: reason.as_deref(),
                tags,
                include_logs,
                extra_attachments: &extra_attachments,
                extra_attachment_paths: &attachment_paths,
                session_source: Some(session_source),
                logs_override: sqlite_feedback_logs,
            })
        })
        .await;

        let upload_result = match upload_result {
            Ok(result) => result,
            Err(join_err) => {
                return Err(internal_error(format!(
                    "failed to upload feedback: {join_err}"
                )));
            }
        };

        upload_result.map_err(|err| internal_error(format!("failed to upload feedback: {err}")))?;
        Ok(FeedbackUploadResponse { thread_id })
    }

    async fn resolve_rollout_path(
        &self,
        conversation_id: ThreadId,
        state_db_ctx: Option<&StateDbHandle>,
    ) -> Option<PathBuf> {
        if let Ok(conversation) = self.thread_manager.get_thread(conversation_id).await
            && let Some(rollout_path) = conversation.rollout_path()
        {
            return Some(rollout_path);
        }

        let state_db_ctx = state_db_ctx?;
        state_db_ctx
            .find_rollout_path_by_id(conversation_id, /*archived_only*/ None)
            .await
            .unwrap_or_else(|err| {
                warn!("failed to resolve rollout path for thread_id={conversation_id}: {err}");
                None
            })
    }
}

async fn materialized_rollout_feedback_attachment(
    codex_home: &Path,
    rollout_path: &Path,
) -> anyhow::Result<Option<FeedbackAttachment>> {
    let (rollout_items, _, _) = RolloutRecorder::load_rollout_items(rollout_path).await?;
    if !rollout_items
        .iter()
        .any(|item| matches!(item, RolloutItem::RolloutReference(_)))
    {
        return Ok(None);
    }
    let rollout_items =
        materialize_rollout_items_for_complete_history(codex_home, &rollout_items).await?;
    let file_stem = rollout_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("rollout");
    Ok(Some(FeedbackAttachment {
        filename: format!("{file_stem}-complete-history.json"),
        content_type: Some("application/json".to_string()),
        buffer: serde_json::to_vec_pretty(&rollout_items)?,
    }))
}

fn auto_review_rollout_filename(thread_id: ThreadId) -> String {
    format!("auto-review-rollout-{thread_id}.jsonl")
}

#[cfg(target_os = "windows")]
fn windows_sandbox_log_attachment(codex_home: &Path) -> Option<FeedbackAttachmentPath> {
    let sandbox_log_path = codex_windows_sandbox::current_log_file_path_for_codex_home(codex_home);
    sandbox_log_path
        .is_file()
        .then_some(FeedbackAttachmentPath {
            path: sandbox_log_path,
            attachment_filename_override: Some(WINDOWS_SANDBOX_LOG_ATTACHMENT_FILENAME.to_string()),
        })
}

#[cfg(not(target_os = "windows"))]
fn windows_sandbox_log_attachment(_codex_home: &Path) -> Option<FeedbackAttachmentPath> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::protocol::RolloutLine;
    use codex_protocol::protocol::RolloutReferenceItem;
    use codex_protocol::protocol::SessionMetaLine;
    use pretty_assertions::assert_eq;

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_sandbox_log_attachment_uses_current_log() {
        let codex_home = tempfile::tempdir().expect("create tempdir");
        let sandbox_dir = codex_windows_sandbox::sandbox_dir(codex_home.path());
        std::fs::create_dir_all(&sandbox_dir).expect("create sandbox dir");
        let sandbox_log_path =
            codex_windows_sandbox::current_log_file_path_for_codex_home(codex_home.path());
        std::fs::write(&sandbox_log_path, "sandbox log").expect("write sandbox log");

        let attachment = windows_sandbox_log_attachment(codex_home.path())
            .map(|attachment| (attachment.path, attachment.attachment_filename_override));

        assert_eq!(
            attachment,
            Some((
                sandbox_log_path,
                Some(WINDOWS_SANDBOX_LOG_ATTACHMENT_FILENAME.to_string())
            ))
        );
    }

    #[tokio::test]
    async fn feedback_attachment_contains_complete_referenced_history() {
        let codex_home = tempfile::tempdir().expect("create codex home");
        let predecessor_path = codex_home.path().join("predecessor.jsonl");
        let current_path = codex_home.path().join("current.jsonl");
        let timestamp = "2026-06-12T00:00:00Z";
        let predecessor_meta = RolloutItem::SessionMeta(SessionMetaLine {
            meta: SessionMeta {
                id: ThreadId::new(),
                timestamp: timestamp.to_string(),
                ..SessionMeta::default()
            },
            git: None,
        });
        let predecessor = RolloutItem::ResponseItem(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "before rotation".to_string(),
            }],
            phase: None,
        });
        let predecessor_lines = [predecessor_meta.clone(), predecessor.clone()]
            .into_iter()
            .map(|item| {
                serde_json::to_string(&RolloutLine {
                    timestamp: timestamp.to_string(),
                    item,
                })
                .expect("serialize predecessor")
            })
            .collect::<Vec<_>>()
            .join("\n");
        tokio::fs::write(&predecessor_path, format!("{predecessor_lines}\n"))
            .await
            .expect("write predecessor");
        let current = RolloutItem::ResponseItem(ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "after rotation".to_string(),
            }],
            phase: None,
        });
        let current_lines = [
            RolloutItem::RolloutReference(RolloutReferenceItem {
                rollout_path: predecessor_path,
                thread_id: None,
                rollout_timestamp: None,
                segment_id: None,
                max_depth: 1,
                nth_user_message: None,
                compacted_replacement_history_filter_texts: None,
            }),
            current.clone(),
        ]
        .into_iter()
        .map(|item| {
            serde_json::to_string(&RolloutLine {
                timestamp: timestamp.to_string(),
                item,
            })
            .expect("serialize current rollout line")
        })
        .collect::<Vec<_>>()
        .join("\n");
        tokio::fs::write(&current_path, format!("{current_lines}\n"))
            .await
            .expect("write current rollout");

        let attachment =
            materialized_rollout_feedback_attachment(codex_home.path(), current_path.as_path())
                .await
                .expect("materialize attachment")
                .expect("reference-backed rollout should produce an attachment");
        let items: Vec<RolloutItem> =
            serde_json::from_slice(&attachment.buffer).expect("parse attachment");

        assert_eq!(
            serde_json::to_value(items).expect("serialize attachment items"),
            serde_json::to_value(vec![predecessor_meta, predecessor, current])
                .expect("serialize expected items")
        );
        assert_eq!(
            attachment.filename,
            "current-complete-history.json".to_string()
        );
    }
}
