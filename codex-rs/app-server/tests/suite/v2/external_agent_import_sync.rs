use std::fs::FileTimes;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportCompletedNotification;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

const TIMEOUT: Duration = Duration::from_secs(60);
const IMPORT_MARKER: &str = "<EXTERNAL SESSION IMPORTED>";
const FIRST_USER: &str = "original external message";
const LATE_ASSISTANT: &str = "late external assistant reply";
const SECOND_USER: &str = "second original external message";
const SECOND_LATE_ASSISTANT: &str = "second late external assistant reply";
const LATER_USER: &str = "later external user message";
const NATIVE_USER: &str = "native Codex message";
const NATIVE_ASSISTANT: &str = "native Codex answer";

fn source_record(cwd: &Path, role: &str, text: &str) -> Value {
    json!({
        "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "type": role,
        "cwd": cwd,
        "message": { "content": text },
    })
}

struct ImportFixture {
    _codex_home: TempDir,
    project_root: PathBuf,
    session_path: PathBuf,
    app_server: TestAppServer,
}

impl ImportFixture {
    async fn new() -> Result<Self> {
        let codex_home = TempDir::new()?;
        let project_root = codex_home.path().join("repo");
        std::fs::create_dir_all(&project_root)?;
        let session_path = codex_home
            .path()
            .join(concat!(".", "cla", "ude"))
            .join("projects/repo/session.jsonl");
        std::fs::create_dir_all(session_path.parent().context("session parent")?)?;
        let initial_record = source_record(&project_root, "user", FIRST_USER);
        std::fs::write(&session_path, format!("{initial_record}\n"))?;

        let app_server = start_app_server(codex_home.path()).await?;
        Ok(Self {
            _codex_home: codex_home,
            project_root,
            session_path,
            app_server,
        })
    }

    fn ledger_path(&self) -> PathBuf {
        self._codex_home
            .path()
            .join("external_agent_session_imports.json")
    }

    fn append(&self, records: &[(&str, &str)]) -> Result<()> {
        self.append_to(&self.session_path, records)
    }

    fn append_to(&self, session_path: &Path, records: &[(&str, &str)]) -> Result<()> {
        let raw = records
            .iter()
            .map(|(role, text)| source_record(&self.project_root, role, text).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        self.append_raw_to(session_path, &format!("{raw}\n"))
    }

    fn append_raw(&self, raw: &str) -> Result<()> {
        self.append_raw_to(&self.session_path, raw)
    }

    fn append_raw_to(&self, session_path: &Path, raw: &str) -> Result<()> {
        let modified_at = std::fs::metadata(session_path)?.modified()?;
        let mut session = OpenOptions::new().append(true).open(session_path)?;
        session.write_all(raw.as_bytes())?;
        session.set_times(FileTimes::new().set_modified(modified_at + Duration::from_secs(1)))?;
        Ok(())
    }

    async fn rpc<T: serde::de::DeserializeOwned>(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<T> {
        let request_id = self
            .app_server
            .send_raw_request(method, Some(params))
            .await?;
        timeout(TIMEOUT, self.app_server.read_response(request_id)).await?
    }

    async fn detect(&mut self) -> Result<ExternalAgentConfigDetectResponse> {
        self.rpc("externalAgentConfig/detect", json!({ "includeHome": true }))
            .await
    }

    async fn import_detected(&mut self) -> Result<Vec<String>> {
        let detected = self.detect().await?;
        assert!(
            !detected.items.is_empty(),
            "changed session must be detected"
        );
        let imported: ExternalAgentConfigImportResponse = self
            .rpc(
                "externalAgentConfig/import",
                json!({ "migrationItems": detected.items }),
            )
            .await?;
        let completed: ExternalAgentConfigImportCompletedNotification = timeout(
            TIMEOUT,
            self.app_server
                .read_notification("externalAgentConfig/import/completed"),
        )
        .await??;
        assert_eq!(completed.import_id, imported.import_id);
        let sessions = completed
            .item_type_results
            .iter()
            .find(|result| result.item_type == ExternalAgentConfigMigrationItemType::Sessions)
            .context("sessions import result")?;
        assert!(
            sessions.failures.is_empty(),
            "session import must not report failures: {:?}",
            sessions.failures
        );
        sessions
            .successes
            .iter()
            .map(|success| success.target.clone().context("imported session target"))
            .collect()
    }

    async fn import_one(&mut self) -> Result<String> {
        let targets = self.import_detected().await?;
        let [target] = targets.as_slice() else {
            anyhow::bail!("session import must produce exactly one task");
        };
        Ok(target.clone())
    }

    async fn read(&mut self, thread_id: &str) -> Result<ThreadReadResponse> {
        self.rpc(
            "thread/read",
            json!({ "threadId": thread_id, "includeTurns": true }),
        )
        .await
    }

    async fn thread_count(&mut self) -> Result<usize> {
        let response: ThreadListResponse = self.rpc("thread/list", json!({})).await?;
        Ok(response.data.len())
    }

    async fn resume(&mut self, thread_id: &str) -> Result<()> {
        let _: Value = self
            .rpc("thread/resume", json!({ "threadId": thread_id }))
            .await?;
        Ok(())
    }

    async fn restart(&mut self) -> Result<()> {
        timeout(TIMEOUT, self.app_server.shutdown_gracefully()).await??;
        self.app_server = start_app_server(self._codex_home.path()).await?;
        Ok(())
    }

    async fn assert_deferred(
        &mut self,
        ledger_before: &[u8],
        threads: &[(&str, &[(&str, &str)])],
    ) -> Result<()> {
        assert!(self.import_detected().await?.is_empty());
        assert_eq!(self.thread_count().await?, threads.len());
        for &(thread_id, expected_history) in threads {
            assert_history(&self.read(thread_id).await?, expected_history);
        }
        assert_eq!(std::fs::read(self.ledger_path())?, ledger_before);
        assert!(!self.detect().await?.items.is_empty());
        Ok(())
    }

    async fn assert_one_deferred(
        &mut self,
        ledger_before: &[u8],
        thread_id: &str,
        expected_history: &[(&str, &str)],
    ) -> Result<()> {
        self.assert_deferred(ledger_before, &[(thread_id, expected_history)])
            .await
    }
}

async fn start_app_server(codex_home: &Path) -> Result<TestAppServer> {
    let home = codex_home.display().to_string();
    TestAppServer::builder()
        .with_codex_home(codex_home)
        .with_env_overrides(&[("HOME", Some(home.as_str()))])
        .build_initialized_with_timeout(TIMEOUT)
        .await
}

fn assert_history(response: &ThreadReadResponse, expected: &[(&str, &str)]) {
    let mut actual = Vec::new();
    let mut markers = 0;
    for item in response.thread.turns.iter().flat_map(|turn| &turn.items) {
        match item {
            ThreadItem::UserMessage { content, .. } => {
                actual.extend(content.iter().filter_map(|input| match input {
                    UserInput::Text { text, .. } => Some(("user", text.as_str())),
                    _ => None,
                }));
            }
            ThreadItem::AgentMessage { text, .. } if text == IMPORT_MARKER => markers += 1,
            ThreadItem::AgentMessage { text, .. } => actual.push(("assistant", text.as_str())),
            _ => {}
        }
    }
    assert_eq!(actual.as_slice(), expected);
    assert_eq!(markers, 1, "the import marker must appear exactly once");
}

#[tokio::test]
async fn cold_exact_prefix_appends_suffix_to_same_task_and_checkpoints() -> Result<()> {
    let mut fixture = ImportFixture::new().await?;
    let original = fixture.import_one().await?;

    fixture.append(&[("assistant", LATE_ASSISTANT)])?;
    assert_eq!(fixture.import_detected().await?, vec![original.clone()]);

    assert_eq!(fixture.thread_count().await?, 1);
    assert_history(
        &fixture.read(&original).await?,
        &[("user", FIRST_USER), ("assistant", LATE_ASSISTANT)],
    );
    assert!(fixture.detect().await?.items.is_empty());
    Ok(())
}

#[tokio::test]
async fn concurrent_changed_sessions_checkpoint_without_lost_update() -> Result<()> {
    let mut fixture = ImportFixture::new().await?;
    let second_session_path = fixture.session_path.with_file_name("second.jsonl");
    let second_initial = source_record(&fixture.project_root, "user", SECOND_USER);
    std::fs::write(&second_session_path, format!("{second_initial}\n"))?;

    let mut original_threads = fixture.import_detected().await?;
    assert_eq!(original_threads.len(), 2);
    original_threads.sort();

    fixture.append(&[("assistant", LATE_ASSISTANT)])?;
    fixture.append_to(
        &second_session_path,
        &[("assistant", SECOND_LATE_ASSISTANT)],
    )?;
    let mut appended_threads = fixture.import_detected().await?;
    appended_threads.sort();

    assert_eq!(appended_threads, original_threads);
    assert_eq!(fixture.thread_count().await?, 2);
    assert!(fixture.detect().await?.items.is_empty());
    Ok(())
}

#[tokio::test]
async fn active_target_is_deferred_without_checkpoint() -> Result<()> {
    let mut fixture = ImportFixture::new().await?;
    let original = fixture.import_one().await?;
    let ledger_before = std::fs::read(fixture.ledger_path())?;
    fixture.resume(&original).await?;
    fixture.append(&[("user", LATER_USER)])?;

    fixture
        .assert_one_deferred(&ledger_before, &original, &[("user", FIRST_USER)])
        .await
}

#[tokio::test]
async fn cold_diverged_target_is_deferred_without_checkpoint() -> Result<()> {
    let model = create_mock_responses_server_repeating_assistant(NATIVE_ASSISTANT).await;
    let mut fixture = ImportFixture::new().await?;
    MockResponsesConfig::new(&model.uri()).write(fixture._codex_home.path())?;
    fixture.restart().await?;
    let original = fixture.import_one().await?;
    fixture.resume(&original).await?;
    let environment = fixture.app_server.auto_env_params()?;
    timeout(
        TIMEOUT,
        fixture
            .app_server
            .start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: original.clone(),
                input: vec![UserInput::Text {
                    text: NATIVE_USER.to_string(),
                    text_elements: Vec::new(),
                }],
                environments: Some(vec![environment]),
                ..Default::default()
            }),
    )
    .await??;

    fixture.restart().await?;
    let ledger_before = std::fs::read(fixture.ledger_path())?;
    fixture.append(&[("user", LATER_USER)])?;
    fixture
        .assert_one_deferred(
            &ledger_before,
            &original,
            &[
                ("user", FIRST_USER),
                ("user", NATIVE_USER),
                ("assistant", NATIVE_ASSISTANT),
            ],
        )
        .await
}

#[tokio::test]
async fn changed_hash_with_equal_transcript_is_deferred_without_checkpoint() -> Result<()> {
    let mut fixture = ImportFixture::new().await?;
    let original = fixture.import_one().await?;
    let ledger_before = std::fs::read(fixture.ledger_path())?;
    fixture.append_raw("\n")?;

    fixture
        .assert_one_deferred(&ledger_before, &original, &[("user", FIRST_USER)])
        .await
}

#[tokio::test]
async fn ambiguous_legacy_targets_are_deferred_without_checkpoint() -> Result<()> {
    let mut fixture = ImportFixture::new().await?;
    let original = fixture.import_one().await?;
    let mut ambiguous_ledger: Value =
        serde_json::from_slice(&std::fs::read(fixture.ledger_path())?)?;
    let mut second_target = ambiguous_ledger["records"][0].clone();
    second_target["imported_thread_id"] = json!("01800000-0001-7000-8000-000000000001");
    ambiguous_ledger["records"]
        .as_array_mut()
        .context("ledger records")?
        .push(second_target);
    let ambiguous_ledger = serde_json::to_vec(&ambiguous_ledger)?;
    std::fs::write(fixture.ledger_path(), ambiguous_ledger)?;

    fixture.append(&[("user", LATER_USER)])?;
    let ledger_before = std::fs::read(fixture.ledger_path())?;
    fixture
        .assert_one_deferred(&ledger_before, &original, &[("user", FIRST_USER)])
        .await
}
