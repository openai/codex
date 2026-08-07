use super::*;
use codex_connectors::ConnectorDirectoryCacheContext;
use codex_connectors::ConnectorDirectoryCacheKey;
use codex_connectors::connector_runtime_cache_path;
use codex_feedback::CODEX_APP_DIRECTORY_CACHE_ATTACHMENT_FILENAME;
use codex_feedback::CODEX_APPS_TOOLS_CACHE_ATTACHMENT_FILENAME;
#[cfg(target_os = "windows")]
use codex_feedback::WINDOWS_SANDBOX_LOG_ATTACHMENT_FILENAME;
use codex_rollout::RolloutRecorder;
use sha2::Digest;
use sha2::Sha256;

const MAX_FEEDBACK_TREE_THREADS: usize = 8;

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

        let auth = self.auth_manager.auth_cached();
        let turn_metadata = if let Some(conversation_id) = conversation_id
            && let Some(rollout_path) = self
                .resolve_rollout_path(conversation_id, self.state_db.as_ref())
                .await
        {
            feedback_turn_metadata_from_rollout(
                &rollout_path,
                upload_tags.get("turn_id").map(String::as_str),
            )
            .await
        } else {
            None
        };
        apply_feedback_turn_metadata(&mut upload_tags, turn_metadata);

        if let Some(chatgpt_user_id) = auth
            .as_ref()
            .and_then(codex_login::CodexAuth::get_chatgpt_user_id)
        {
            tracing::info!(target: "feedback_tags", chatgpt_user_id);
        }
        if let Some(account_id) = auth
            .as_ref()
            .and_then(codex_login::CodexAuth::get_account_id)
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
                        vec![conversation_id]
                    }
                },
                None => Vec::new(),
            };
            let mut feedback_thread_ids = feedback_thread_ids;
            let original_len = feedback_thread_ids.len();
            if let Some(conversation_id) = conversation_id {
                let mut descendant_thread_ids = feedback_thread_ids
                    .into_iter()
                    .filter(|thread_id| *thread_id != conversation_id)
                    .collect::<Vec<_>>();
                // Thread ids are UUIDv7, so lexicographic order tracks creation time.
                descendant_thread_ids.sort_unstable_by_key(ToString::to_string);
                if original_len > MAX_FEEDBACK_TREE_THREADS {
                    let keep_descendants = MAX_FEEDBACK_TREE_THREADS.saturating_sub(1);
                    let split_index = descendant_thread_ids.len().saturating_sub(keep_descendants);
                    descendant_thread_ids = descendant_thread_ids.split_off(split_index);
                    warn!(
                        "feedback log upload for thread_id={conversation_id:?} truncated from {original_len} threads to root plus {keep_descendants} most recent descendants"
                    );
                }
                feedback_thread_ids = Vec::with_capacity(descendant_thread_ids.len() + 1);
                feedback_thread_ids.push(conversation_id);
                feedback_thread_ids.extend(descendant_thread_ids);
            }
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
            for cache_attachment in tool_cache_feedback_attachments(
                self.config.codex_home.as_path(),
                &self.config.chatgpt_base_url,
                auth.as_ref(),
            ) {
                if seen_attachment_paths.insert(cache_attachment.path.clone()) {
                    attachment_paths.push(cache_attachment);
                }
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

        let mut extra_attachments = Vec::new();
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

#[derive(Debug, PartialEq)]
struct FeedbackTurnMetadata {
    model: String,
    effort: Option<ReasoningEffort>,
    prompt_hash: Option<String>,
}

fn apply_feedback_turn_metadata(
    upload_tags: &mut BTreeMap<String, String>,
    turn_metadata: Option<FeedbackTurnMetadata>,
) {
    // These are reserved tags derived from the persisted rollout rather than
    // accepted from the feedback request.
    upload_tags.remove("prompt_hash");
    upload_tags.remove("prompt_version");

    if let Some(FeedbackTurnMetadata {
        model,
        effort,
        prompt_hash,
    }) = turn_metadata
    {
        upload_tags.insert("model".to_string(), model);
        upload_tags.insert("effort".to_string(), format!("{effort:?}"));
        if let Some(prompt_hash) = prompt_hash {
            upload_tags.insert("prompt_hash".to_string(), prompt_hash);
        }
    }
}

async fn feedback_turn_metadata_from_rollout(
    rollout_path: &Path,
    turn_id: Option<&str>,
) -> Option<FeedbackTurnMetadata> {
    let (items, _, _) = RolloutRecorder::load_rollout_items(rollout_path)
        .await
        .ok()?;
    let prompt_hash = items.iter().find_map(|item| match item {
        RolloutItem::SessionMeta(meta) => meta
            .meta
            .base_instructions
            .as_ref()
            .map(|prompt| normalized_prompt_hash(&prompt.text)),
        _ => None,
    });

    items.into_iter().rev().find_map(|item| match item {
        RolloutItem::TurnContext(context)
            if turn_id.is_none() || context.turn_id.as_deref() == turn_id =>
        {
            Some(FeedbackTurnMetadata {
                model: context.model,
                effort: context.effort,
                prompt_hash: prompt_hash.clone(),
            })
        }
        _ => None,
    })
}

fn normalized_prompt_hash(prompt: &str) -> String {
    let normalized_prompt = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{:x}", Sha256::digest(normalized_prompt.as_bytes()))
}

fn tool_cache_feedback_attachments(
    codex_home: &Path,
    chatgpt_base_url: &str,
    auth: Option<&CodexAuth>,
) -> Vec<FeedbackAttachmentPath> {
    let mut attachments = Vec::with_capacity(2);
    let tools_cache_path = connector_runtime_cache_path(codex_home, auth);
    if tools_cache_path.is_file() {
        attachments.push(FeedbackAttachmentPath {
            path: tools_cache_path,
            attachment_filename_override: Some(
                CODEX_APPS_TOOLS_CACHE_ATTACHMENT_FILENAME.to_string(),
            ),
        });
    }

    let Some(auth) = auth.filter(|auth| auth.uses_codex_backend()) else {
        return attachments;
    };
    let directory_cache_context = ConnectorDirectoryCacheContext::new(
        codex_home.to_path_buf(),
        ConnectorDirectoryCacheKey::new(
            chatgpt_base_url.to_string(),
            auth.get_account_id(),
            auth.get_chatgpt_user_id(),
            auth.is_workspace_account(),
        ),
    );
    let directory_cache_path = directory_cache_context.cache_path();
    if directory_cache_path.is_file() {
        attachments.push(FeedbackAttachmentPath {
            path: directory_cache_path,
            attachment_filename_override: Some(
                CODEX_APP_DIRECTORY_CACHE_ATTACHMENT_FILENAME.to_string(),
            ),
        });
    }

    attachments
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
    use codex_protocol::protocol::RolloutLine;
    use codex_protocol::protocol::TurnContextItem;
    use pretty_assertions::assert_eq;
    #[test]
    fn feedback_tags_drop_unverified_client_prompt_tags() {
        let mut upload_tags = BTreeMap::from([
            ("custom".to_string(), "preserved".to_string()),
            (
                "prompt_hash".to_string(),
                "unverified-client-hash".to_string(),
            ),
            ("prompt_version".to_string(), "client-prompt-v1".to_string()),
        ]);

        apply_feedback_turn_metadata(&mut upload_tags, /*turn_metadata*/ None);

        assert_eq!(
            upload_tags,
            BTreeMap::from([("custom".to_string(), "preserved".to_string())])
        );
    }

    #[test]
    fn feedback_tags_drop_client_prompt_hash_when_rollout_has_no_hash() {
        let mut upload_tags = BTreeMap::from([(
            "prompt_hash".to_string(),
            "unverified-client-hash".to_string(),
        )]);

        apply_feedback_turn_metadata(
            &mut upload_tags,
            Some(FeedbackTurnMetadata {
                model: "reported-model".to_string(),
                effort: Some(ReasoningEffort::High),
                prompt_hash: None,
            }),
        );

        assert_eq!(
            upload_tags,
            BTreeMap::from([
                ("effort".to_string(), "Some(High)".to_string()),
                ("model".to_string(), "reported-model".to_string()),
            ])
        );
    }

    #[test]
    fn feedback_tags_replace_client_prompt_hash_with_rollout_hash() {
        let mut upload_tags = BTreeMap::from([(
            "prompt_hash".to_string(),
            "unverified-client-hash".to_string(),
        )]);

        apply_feedback_turn_metadata(
            &mut upload_tags,
            Some(FeedbackTurnMetadata {
                model: "reported-model".to_string(),
                effort: Some(ReasoningEffort::High),
                prompt_hash: Some("rollout-prompt-hash".to_string()),
            }),
        );

        assert_eq!(
            upload_tags,
            BTreeMap::from([
                ("effort".to_string(), "Some(High)".to_string()),
                ("model".to_string(), "reported-model".to_string()),
                ("prompt_hash".to_string(), "rollout-prompt-hash".to_string(),),
            ])
        );
    }

    #[tokio::test]
    async fn feedback_tags_do_not_trust_the_prompt_version_from_the_reported_rollout() {
        let (_tempdir, rollout_path) =
            feedback_rollout(&[("turn-1", "synthetic-model", Some(ReasoningEffort::High))]);
        let mut upload_tags = BTreeMap::from([(
            "prompt_version".to_string(),
            "unverified-client-prompt".to_string(),
        )]);

        let turn_metadata =
            feedback_turn_metadata_from_rollout(&rollout_path, Some("turn-1")).await;
        apply_feedback_turn_metadata(&mut upload_tags, turn_metadata);

        assert_eq!(
            upload_tags,
            BTreeMap::from([
                ("effort".to_string(), "Some(High)".to_string()),
                ("model".to_string(), "synthetic-model".to_string()),
                (
                    "prompt_hash".to_string(),
                    normalized_prompt_hash("actual developer prompt"),
                ),
            ])
        );
    }

    #[tokio::test]
    async fn feedback_metadata_uses_the_reported_turn() {
        let (_tempdir, rollout_path) = feedback_rollout(&[
            ("turn-1", "reported-model", Some(ReasoningEffort::High)),
            ("turn-2", "newer-model", Some(ReasoningEffort::Ultra)),
        ]);

        assert_eq!(
            feedback_turn_metadata_from_rollout(&rollout_path, Some("turn-1")).await,
            Some(FeedbackTurnMetadata {
                model: "reported-model".to_string(),
                effort: Some(ReasoningEffort::High),
                prompt_hash: Some(normalized_prompt_hash("actual developer prompt")),
            })
        );
    }

    #[tokio::test]
    async fn feedback_metadata_uses_the_latest_turn_when_no_turn_is_reported() {
        let (_tempdir, rollout_path) = feedback_rollout(&[
            ("turn-1", "older-model", Some(ReasoningEffort::High)),
            ("turn-2", "latest-model", Some(ReasoningEffort::Ultra)),
        ]);

        assert_eq!(
            feedback_turn_metadata_from_rollout(&rollout_path, /*turn_id*/ None).await,
            Some(FeedbackTurnMetadata {
                model: "latest-model".to_string(),
                effort: Some(ReasoningEffort::Ultra),
                prompt_hash: Some(normalized_prompt_hash("actual developer prompt")),
            })
        );
    }

    #[tokio::test]
    async fn feedback_metadata_does_not_substitute_a_different_turn() {
        let (_tempdir, rollout_path) =
            feedback_rollout(&[("turn-1", "different-model", Some(ReasoningEffort::High))]);

        assert_eq!(
            feedback_turn_metadata_from_rollout(&rollout_path, Some("missing-turn")).await,
            None
        );
    }

    #[tokio::test]
    async fn feedback_metadata_preserves_unspecified_effort_and_prompt_hash() {
        let (_tempdir, rollout_path) =
            feedback_rollout(&[("turn-1", "reported-model", /*effort*/ None)]);

        assert_eq!(
            feedback_turn_metadata_from_rollout(&rollout_path, Some("turn-1")).await,
            Some(FeedbackTurnMetadata {
                model: "reported-model".to_string(),
                effort: None,
                prompt_hash: Some(normalized_prompt_hash("actual developer prompt")),
            })
        );
    }

    #[tokio::test]
    async fn feedback_hashes_the_actual_developer_prompt_from_session_metadata() {
        let (_tempdir, rollout_path) = feedback_rollout(&[("turn-1", "reported-model", None)]);

        assert_eq!(
            feedback_turn_metadata_from_rollout(&rollout_path, Some("turn-1"))
                .await
                .and_then(|metadata| metadata.prompt_hash),
            Some(normalized_prompt_hash("actual developer prompt")),
        );
    }

    #[test]
    fn prompt_hash_normalizes_whitespace() {
        assert_eq!(
            normalized_prompt_hash("actual  developer\r\nprompt\t"),
            "9ae77301cc2a30e729c28661b7a0f9490c80a72e7d23277e7e74f0ac81779541"
        );
    }

    fn feedback_rollout(
        turns: &[(&str, &str, Option<ReasoningEffort>)],
    ) -> (tempfile::TempDir, PathBuf) {
        let tempdir = tempfile::tempdir().expect("create feedback rollout directory");
        let rollout_path = tempdir.path().join("feedback-rollout.jsonl");
        let mut lines = vec![RolloutLine {
            timestamp: "2026-07-24T00:00:00Z".to_string(),
            ordinal: None,
            item: RolloutItem::SessionMeta(SessionMetaLine {
                meta: codex_protocol::protocol::SessionMeta {
                    cwd: tempdir.path().to_path_buf(),
                    base_instructions: Some(codex_protocol::models::BaseInstructions {
                        text: "actual developer prompt".to_string(),
                        provenance: None,
                    }),
                    ..Default::default()
                },
                git: None,
            }),
        }];
        lines.extend(turns.iter().map(|(turn_id, model, effort)| {
            RolloutLine {
                timestamp: "2026-07-24T00:00:01Z".to_string(),
                ordinal: None,
                item: RolloutItem::TurnContext(TurnContextItem {
                    turn_id: Some((*turn_id).to_string()),
                    cwd: AbsolutePathBuf::from_absolute_path(tempdir.path())
                        .expect("absolute feedback rollout directory"),
                    workspace_roots: None,
                    current_date: None,
                    timezone: None,
                    approval_policy: codex_protocol::protocol::AskForApproval::Never,
                    approvals_reviewer: None,
                    sandbox_policy: codex_protocol::protocol::SandboxPolicy::new_read_only_policy(),
                    permission_profile: None,
                    network: None,
                    file_system_sandbox_policy: None,
                    model: (*model).to_string(),
                    comp_hash: None,
                    personality: None,
                    collaboration_mode: None,
                    multi_agent_version: None,
                    multi_agent_mode: None,
                    realtime_active: None,
                    effort: effort.clone(),
                    summary: ReasoningSummary::Auto,
                }),
            }
        }));
        let contents = lines
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()
            .expect("serialize feedback rollout")
            .join("\n");
        std::fs::write(&rollout_path, format!("{contents}\n")).expect("write feedback rollout");

        (tempdir, rollout_path)
    }

    #[test]
    fn tool_cache_feedback_attachments_include_existing_active_cache_files() {
        let codex_home = tempfile::tempdir().expect("create tempdir");
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let tools_cache_path = connector_runtime_cache_path(codex_home.path(), Some(&auth));
        std::fs::create_dir_all(tools_cache_path.parent().expect("tools cache parent"))
            .expect("create tools cache directory");
        std::fs::write(&tools_cache_path, b"tools").expect("write tools cache");

        let account_id = auth.get_account_id().expect("dummy auth account id");
        let directory_cache_context = ConnectorDirectoryCacheContext::new(
            codex_home.path().to_path_buf(),
            ConnectorDirectoryCacheKey::new(
                "https://chatgpt.com/backend-api".to_string(),
                Some(account_id),
                auth.get_chatgpt_user_id(),
                auth.is_workspace_account(),
            ),
        );
        let directory_cache_path = directory_cache_context.cache_path();
        std::fs::create_dir_all(
            directory_cache_path
                .parent()
                .expect("directory cache parent"),
        )
        .expect("create directory cache directory");
        std::fs::write(&directory_cache_path, b"directory").expect("write directory cache");

        let attachments = tool_cache_feedback_attachments(
            codex_home.path(),
            "https://chatgpt.com/backend-api",
            Some(&auth),
        )
        .into_iter()
        .map(|attachment| (attachment.path, attachment.attachment_filename_override))
        .collect::<Vec<_>>();

        assert_eq!(
            attachments,
            vec![
                (
                    tools_cache_path,
                    Some(CODEX_APPS_TOOLS_CACHE_ATTACHMENT_FILENAME.to_string()),
                ),
                (
                    directory_cache_path,
                    Some(CODEX_APP_DIRECTORY_CACHE_ATTACHMENT_FILENAME.to_string()),
                ),
            ]
        );
    }

    #[test]
    fn tool_cache_feedback_attachments_include_directory_cache_without_account_id() {
        let codex_home = tempfile::tempdir().expect("create tempdir");
        let auth = CodexAuth::Headers(codex_login::AuthHeaders::new(
            reqwest::header::HeaderMap::new(),
        ));
        let directory_cache_context = ConnectorDirectoryCacheContext::new(
            codex_home.path().to_path_buf(),
            ConnectorDirectoryCacheKey::new(
                "https://chatgpt.com/backend-api".to_string(),
                /*account_id*/ None,
                auth.get_chatgpt_user_id(),
                auth.is_workspace_account(),
            ),
        );
        let directory_cache_path = directory_cache_context.cache_path();
        std::fs::create_dir_all(
            directory_cache_path
                .parent()
                .expect("directory cache parent"),
        )
        .expect("create directory cache directory");
        std::fs::write(&directory_cache_path, b"directory").expect("write directory cache");

        let attachments = tool_cache_feedback_attachments(
            codex_home.path(),
            "https://chatgpt.com/backend-api",
            Some(&auth),
        )
        .into_iter()
        .map(|attachment| (attachment.path, attachment.attachment_filename_override))
        .collect::<Vec<_>>();

        assert_eq!(
            attachments,
            vec![(
                directory_cache_path,
                Some(CODEX_APP_DIRECTORY_CACHE_ATTACHMENT_FILENAME.to_string()),
            )]
        );
    }

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
}
