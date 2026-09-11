use anyhow::Result;
use anyhow::anyhow;
use codex_core::ForkSnapshot;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_extension_api::Instructions;
use codex_extension_api::LoadInstructionsFuture;
use codex_extension_api::LoadedUserInstructions;
use codex_extension_api::ThreadInstructionsProvider;
use codex_extension_api::UserInstructionsProvider;
use codex_features::Feature;
use codex_history::InitialHistory;
use codex_history::ResumedHistory;
use codex_history::RolloutItem;
use codex_home::CodexHomeUserInstructionsProvider;
use codex_protocol::ThreadId;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use codex_thread_store::ForkBoundary;
use codex_thread_store::LoadThreadHistoryParams;
use codex_thread_store::PrepareForkParams;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use codex_utils_string::approx_bytes_for_tokens;
use core_test_support::PathBufExt;
use core_test_support::create_directory_symlink;
use core_test_support::load_default_config_for_test;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_no_remote_env;
use core_test_support::skip_if_sandbox;
use core_test_support::skip_if_target_windows;
use core_test_support::test_codex::RecordingUserInstructionsProvider;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::executor_path_uri;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tempfile::TempDir;

#[path = "agents_md_refresh.rs"]
mod refresh;

const GLOBAL_AGENTS_FILENAME: &str = "AGENTS.md";
const GLOBAL_AGENTS_OVERRIDE_FILENAME: &str = "AGENTS.override.md";
const GLOBAL_INSTRUCTIONS: &str = "global instructions";
const NEW_GLOBAL_INSTRUCTIONS: &str = "new global instructions";
const NEW_PROJECT_INSTRUCTIONS: &str = "new project instructions";
const OLD_GLOBAL_INSTRUCTIONS: &str = "old global instructions";
const PROJECT_INSTRUCTIONS: &str = "project instructions";
const PROJECT_SEPARATOR: &str = "--- project-doc ---";
const TASK_USER_INSTRUCTIONS: &str = "task user instructions";
const UPDATED_TASK_USER_INSTRUCTIONS: &str = "updated task user instructions";
const SPAWN_CALL_ID: &str = "spawn-global-instructions-child";
const SPAWN_CHILD_PROMPT: &str = "inspect inherited global instructions";
const SPAWN_FRESH_PARENT_PROMPT: &str = "spawn a child with fresh context";
const SPAWN_PARENT_PROMPT: &str = "spawn a child with the parent context";
const SPAWN_SEED_PROMPT: &str = "seed parent history";
const PROVIDER_WARNING: &str = "global instruction source unavailable; using fallback";

struct WarningInstructionsProvider {
    inner: CodexHomeUserInstructionsProvider,
    warning_active: AtomicBool,
}

impl UserInstructionsProvider for WarningInstructionsProvider {
    fn load_user_instructions(&self) -> LoadInstructionsFuture<'_> {
        Box::pin(async move {
            let mut loaded = self.inner.load_user_instructions().await;
            if self.warning_active.load(Ordering::SeqCst) {
                loaded.warnings = vec![PROVIDER_WARNING.to_string()];
            }
            loaded
        })
    }
}

struct RecordingThreadInstructionsProvider {
    loaded: Mutex<LoadedUserInstructions>,
    load_count: AtomicUsize,
}

impl RecordingThreadInstructionsProvider {
    fn new(instructions: Option<Instructions>) -> Self {
        Self {
            loaded: Mutex::new(LoadedUserInstructions {
                instructions,
                warnings: Vec::new(),
            }),
            load_count: AtomicUsize::new(0),
        }
    }

    fn with_text(text: impl Into<String>) -> Self {
        Self::new(Some(Instructions {
            text: text.into(),
            source: None,
        }))
    }

    fn load_count(&self) -> usize {
        self.load_count.load(Ordering::SeqCst)
    }

    fn set_instructions(&self, instructions: Option<Instructions>) {
        self.loaded
            .lock()
            .expect("instruction snapshot lock")
            .instructions = instructions;
    }

    fn set_warnings(&self, warnings: Vec<String>) {
        self.loaded
            .lock()
            .expect("instruction snapshot lock")
            .warnings = warnings;
    }
}

impl ThreadInstructionsProvider for RecordingThreadInstructionsProvider {
    fn load_thread_instructions(&self) -> LoadInstructionsFuture<'_> {
        self.load_count.fetch_add(1, Ordering::SeqCst);
        let loaded = self
            .loaded
            .lock()
            .expect("instruction snapshot lock")
            .clone();
        Box::pin(async move { loaded })
    }
}

async fn agents_instructions(mut builder: TestCodexBuilder) -> Result<String> {
    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;

    let test = builder.build_with_auto_env(&server).await?;
    test.submit_turn("hello").await?;

    let request = resp_mock.single_request();
    request
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.starts_with("# AGENTS.md instructions"))
        .ok_or_else(|| anyhow::anyhow!("instructions message not found"))
}

fn write_global_file(
    home: &TempDir,
    filename: &str,
    contents: impl AsRef<[u8]>,
) -> Result<AbsolutePathBuf> {
    let path = home.path().join(filename);
    std::fs::write(&path, contents)?;
    Ok(path.abs())
}

fn remove_agents_md_world_state_section(rollout_path: &Path) -> Result<()> {
    let rollout = std::fs::read_to_string(rollout_path)?;
    let mut removed_section = false;
    let retained = rollout
        .lines()
        .map(codex_rollout::parse_rollout_line)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .map(|mut line| {
            if let RolloutItem::WorldState(world_state) = &mut line.item
                && world_state.state.remove("agents_md").is_some()
            {
                removed_section = true;
            }
            serde_json::to_string(&line)
        })
        .collect::<std::result::Result<Vec<_>, _>>()?
        .join("\n");
    anyhow::ensure!(
        removed_section,
        "rollout did not contain a persisted AGENTS.md WorldState section"
    );
    std::fs::write(rollout_path, format!("{retained}\n"))?;
    Ok(())
}

fn instruction_fragments(request: &responses::ResponsesRequest) -> Vec<String> {
    request
        .message_input_texts("user")
        .into_iter()
        .filter(|text| text.starts_with("# AGENTS.md instructions"))
        .collect()
}

fn expected_instruction_fragment(cwd: &PathUri, contents: &str) -> String {
    let cwd = cwd.inferred_native_path_string();
    format!("# AGENTS.md instructions for {cwd}\n\n<INSTRUCTIONS>\n{contents}\n</INSTRUCTIONS>")
}

fn expected_provider_only_instruction_fragment(contents: &str) -> String {
    format!("# AGENTS.md instructions\n\n<INSTRUCTIONS>\n{contents}\n</INSTRUCTIONS>")
}

fn assert_single_instruction_fragment(request: &responses::ResponsesRequest, expected: &str) {
    assert_eq!(instruction_fragments(request), vec![expected.to_string()]);
}

async fn submit_thread_turn(thread: &Arc<codex_core::CodexThread>, prompt: &str) -> Result<()> {
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: prompt.to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    Ok(())
}

async fn persisted_resume_history(
    thread: &Arc<codex_core::CodexThread>,
) -> Result<(ThreadId, InitialHistory)> {
    thread.ensure_rollout_materialized().await;
    thread.flush_rollout().await?;
    let stored = thread
        .read_thread(
            /*include_archived*/ true, /*include_history*/ true,
        )
        .await?;
    let thread_id = stored.thread_id;
    Ok((
        thread_id,
        InitialHistory::Resumed(ResumedHistory {
            conversation_id: thread_id,
            history: Arc::new(
                stored
                    .history
                    .ok_or_else(|| anyhow!("thread history should be loaded"))?
                    .items,
            ),
            rollout_path: stored.rollout_path,
        }),
    ))
}

fn request_body_contains(request: &wiremock::Request, text: &str) -> bool {
    let is_zstd = request
        .headers
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|entry| entry.trim().eq_ignore_ascii_case("zstd"))
        });
    let body = if is_zstd {
        zstd::stream::decode_all(std::io::Cursor::new(&request.body)).ok()
    } else {
        Some(request.body.clone())
    };
    body.and_then(|body| String::from_utf8(body).ok())
        .is_some_and(|body| body.contains(text))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agents_override_is_preferred_over_agents_md() -> Result<()> {
    let instructions =
        agents_instructions(test_codex().with_workspace_setup(|cwd, fs| async move {
            let agents_md = cwd.join("AGENTS.md");
            let override_md = cwd.join("AGENTS.override.md");
            let agents_md_uri = executor_path_uri(&agents_md)?;
            let override_md_uri = executor_path_uri(&override_md)?;
            fs.write_file(
                &agents_md_uri,
                b"base doc".to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            fs.write_file(
                &override_md_uri,
                b"override doc".to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok::<(), anyhow::Error>(())
        }))
        .await?;

    assert!(
        instructions.contains("override doc"),
        "expected AGENTS.override.md contents: {instructions}"
    );
    assert!(
        !instructions.contains("base doc"),
        "expected AGENTS.md to be ignored when override exists: {instructions}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_fallback_is_used_when_agents_candidate_is_directory() -> Result<()> {
    let instructions = agents_instructions(
        test_codex()
            .with_config(|config| {
                config.project_doc_fallback_filenames = vec!["WORKFLOW.md".to_string()];
            })
            .with_workspace_setup(|cwd, fs| async move {
                let agents_dir = cwd.join("AGENTS.md");
                let fallback = cwd.join("WORKFLOW.md");
                let agents_dir_uri = executor_path_uri(&agents_dir)?;
                let fallback_uri = executor_path_uri(&fallback)?;
                fs.create_directory(
                    &agents_dir_uri,
                    CreateDirectoryOptions {
                        recursive: true,
                        follow_symlinks: true,
                    },
                    /*sandbox*/ None,
                )
                .await?;
                fs.write_file(
                    &fallback_uri,
                    b"fallback doc".to_vec(),
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await?;
                Ok::<(), anyhow::Error>(())
            }),
    )
    .await?;

    assert!(
        instructions.contains("fallback doc"),
        "expected fallback doc contents: {instructions}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agents_docs_are_concatenated_from_project_root_to_cwd() -> Result<()> {
    let instructions = agents_instructions(
        test_codex()
            .with_config(|config| {
                config.cwd = config.cwd.join("nested/workspace");
            })
            .with_workspace_setup(|cwd, fs| async move {
                let nested = cwd.clone();
                let root = nested
                    .parent()
                    .and_then(|parent| parent.parent())
                    .expect("nested workspace should have a project root ancestor");
                let root_agents = root.join("AGENTS.md");
                let git_marker = root.join(".git");
                let nested_agents = nested.join("AGENTS.md");
                let nested_uri = executor_path_uri(&nested)?;
                let root_agents_uri = executor_path_uri(&root_agents)?;
                let git_marker_uri = executor_path_uri(&git_marker)?;
                let nested_agents_uri = executor_path_uri(&nested_agents)?;

                fs.create_directory(
                    &nested_uri,
                    CreateDirectoryOptions {
                        recursive: true,
                        follow_symlinks: true,
                    },
                    /*sandbox*/ None,
                )
                .await?;
                fs.write_file(
                    &root_agents_uri,
                    b"root doc".to_vec(),
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await?;
                fs.write_file(
                    &git_marker_uri,
                    b"gitdir: /tmp/mock-git-dir\n".to_vec(),
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await?;
                fs.write_file(
                    &nested_agents_uri,
                    b"child doc".to_vec(),
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await?;
                Ok::<(), anyhow::Error>(())
            }),
    )
    .await?;

    let root_pos = instructions
        .find("root doc")
        .expect("expected root doc in AGENTS instructions");
    let child_pos = instructions
        .find("child doc")
        .expect("expected child doc in AGENTS instructions");
    assert!(
        root_pos < child_pos,
        "expected root doc before child doc: {instructions}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn symlinked_cwd_uses_logical_parent_for_agents_discovery() -> Result<()> {
    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;

    let mut builder = test_codex()
        .with_config(|config| {
            config.cwd = config.cwd.join("logical-repo/workspace");
        })
        .with_workspace_setup(|cwd, _fs| async move {
            // Construct two sibling repositories with the configured cwd as a
            // directory symlink from the logical repository into the physical
            // repository:
            //
            // test-root/
            // |-- logical-repo/
            // |   |-- .git
            // |   |-- AGENTS.md              ("logical parent doc")
            // |   `-- workspace ------------> physical-repo/workspace/
            // `-- physical-repo/
            //     |-- .git
            //     |-- AGENTS.md              ("physical parent doc")
            //     `-- workspace/
            //         `-- AGENTS.md          ("workspace doc")
            //
            // Discovery should walk the lexical path through logical-repo,
            // while opening logical-repo/workspace/AGENTS.md still follows the
            // symlink into physical-repo/workspace.
            let logical_root = cwd.parent().expect("symlink should have a parent");
            let test_root = logical_root
                .parent()
                .expect("logical repository should have a parent");
            let physical_root = test_root.join("physical-repo");
            let physical_workspace = physical_root.join("workspace");

            std::fs::create_dir_all(logical_root.as_path())?;
            std::fs::write(logical_root.join(".git"), "")?;
            std::fs::write(logical_root.join("AGENTS.md"), "logical parent doc")?;

            std::fs::create_dir_all(physical_workspace.as_path())?;
            std::fs::write(physical_root.join(".git"), "")?;
            std::fs::write(physical_root.join("AGENTS.md"), "physical parent doc")?;
            std::fs::write(physical_workspace.join("AGENTS.md"), "workspace doc")?;

            create_directory_symlink(physical_workspace.as_path(), cwd.as_path());
            Ok(())
        });
    let test = builder.build(&server).await?;
    let logical_root = test
        .config
        .cwd
        .parent()
        .expect("symlink should have a parent");

    assert_eq!(
        test.codex.instruction_sources().await,
        vec![
            PathUri::from_abs_path(&logical_root.join("AGENTS.md")),
            PathUri::from_abs_path(&test.config.cwd.join("AGENTS.md"))
        ]
    );

    test.submit_turn("hello").await?;
    let instructions = resp_mock
        .single_request()
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.starts_with("# AGENTS.md instructions"))
        .expect("instructions message");
    assert!(instructions.contains("logical parent doc"));
    assert!(instructions.contains("workspace doc"));
    assert!(!instructions.contains("physical parent doc"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_environment_sources_match_model_visible_instructions() -> Result<()> {
    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let global_agents = home.path().join("AGENTS.md");
    std::fs::write(&global_agents, "global doc")?;

    let mut builder = test_codex()
        .with_home(home)
        .with_workspace_setup(|cwd, fs| async move {
            let agents_md_uri = executor_path_uri(cwd.join("AGENTS.md"))?;
            fs.write_file(
                &agents_md_uri,
                b"project doc".to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok::<(), anyhow::Error>(())
        });
    let test = builder.build_with_auto_env(&server).await?;
    let global_agents = global_agents.abs();

    assert_eq!(
        test.codex.instruction_sources().await,
        vec![
            PathUri::from_abs_path(&global_agents),
            test.workspace_path_uri("AGENTS.md")?,
        ]
    );

    test.submit_turn("hello").await?;
    let instructions = resp_mock
        .single_request()
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.starts_with("# AGENTS.md instructions"))
        .expect("instructions message");
    assert!(instructions.contains("global doc\n\n--- project-doc ---\n\nproject doc"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn untrusted_project_excludes_project_instructions() -> Result<()> {
    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let global_agents =
        write_global_file(home.as_ref(), GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;

    let mut builder = test_codex()
        .with_home(home)
        .with_config(|config| {
            config.active_project.trust_level = Some(TrustLevel::Untrusted);
        })
        .with_workspace_setup(|cwd, fs| async move {
            fs.write_file(
                &executor_path_uri(cwd.join(GLOBAL_AGENTS_FILENAME))?,
                PROJECT_INSTRUCTIONS.as_bytes().to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok::<(), anyhow::Error>(())
        });
    let test = builder.build_with_auto_env(&server).await?;

    assert_eq!(
        test.codex.instruction_sources().await,
        vec![PathUri::from_abs_path(&global_agents)]
    );

    test.submit_turn("hello").await?;
    let instructions = resp_mock
        .single_request()
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.starts_with("# AGENTS.md instructions"))
        .expect("global instructions message");
    assert!(instructions.contains(GLOBAL_INSTRUCTIONS));
    assert!(!instructions.contains(PROJECT_INSTRUCTIONS));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn runtime_trust_reload_refreshes_project_instructions() -> Result<()> {
    let server = start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
            sse(vec![ev_response_created("resp2"), ev_completed("resp2")]),
            sse(vec![ev_response_created("resp3"), ev_completed("resp3")]),
        ],
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let global_agents =
        write_global_file(home.as_ref(), GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;
    let mut builder = test_codex()
        .with_home(home)
        .with_config(|config| {
            config.active_project.trust_level = Some(TrustLevel::Trusted);
        })
        .with_workspace_setup(|cwd, fs| async move {
            fs.write_file(
                &executor_path_uri(cwd.join(GLOBAL_AGENTS_FILENAME))?,
                PROJECT_INSTRUCTIONS.as_bytes().to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok::<(), anyhow::Error>(())
        });
    let test = builder.build_with_auto_env(&server).await?;
    let project_agents = test.workspace_path_uri(GLOBAL_AGENTS_FILENAME)?;
    let global_agents = PathUri::from_abs_path(&global_agents);

    test.submit_turn("trusted project").await?;
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![global_agents.clone(), project_agents.clone()]
    );

    let mut untrusted_config = (*test.codex.config().await).clone();
    untrusted_config.active_project.trust_level = Some(TrustLevel::Untrusted);
    test.codex.refresh_runtime_config(untrusted_config).await;
    test.submit_turn("untrusted project").await?;
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![global_agents.clone()]
    );

    let mut trusted_config = (*test.codex.config().await).clone();
    trusted_config.active_project.trust_level = Some(TrustLevel::Trusted);
    test.codex.refresh_runtime_config(trusted_config).await;
    test.submit_turn("trusted again").await?;
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![global_agents, project_agents]
    );

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);
    let latest_instruction_fragments = requests
        .iter()
        .map(instruction_fragments)
        .map(|fragments| fragments.last().cloned().expect("instructions message"))
        .collect::<Vec<_>>();
    assert!(latest_instruction_fragments[0].contains(PROJECT_INSTRUCTIONS));
    assert!(!latest_instruction_fragments[1].contains(PROJECT_INSTRUCTIONS));
    assert!(latest_instruction_fragments[2].contains(PROJECT_INSTRUCTIONS));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restricted_project_without_instructions_starts_successfully() -> Result<()> {
    skip_if_target_windows!(
        Ok(()),
        "Windows restricted-token sandbox cannot enforce deny-read policies"
    );
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;
    let mut builder = test_codex().with_config(|config| {
        let mut file_system_policy = FileSystemSandboxPolicy::read_only();
        file_system_policy.entries.push(FileSystemSandboxEntry::new(
            config.cwd.join("private.txt").into(),
            FileSystemAccessMode::Deny,
        ));
        config
            .permissions
            .set_permission_profile(PermissionProfile::from_runtime_permissions(
                &file_system_policy,
                NetworkSandboxPolicy::Restricted,
            ))
            .expect("test config should allow a restricted read policy");
    });
    let test = builder.build_with_auto_env(&server).await?;

    assert_eq!(
        test.codex.instruction_sources().await,
        Vec::<PathUri>::new()
    );
    test.submit_text_turn("continue without project instructions")
        .await?;
    response_mock.single_request();

    Ok(())
}

/// Thread creation fails when sandboxing prevents project instructions from loading.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn denied_project_instructions_fail_thread_creation() -> Result<()> {
    skip_if_target_windows!(
        Ok(()),
        "Windows restricted-token sandbox cannot enforce deny-read policies"
    );
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    write_global_file(home.as_ref(), GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;

    let mut builder = test_codex()
        .with_home(home)
        .with_config(|config| {
            let mut file_system_policy = FileSystemSandboxPolicy::read_only();
            file_system_policy.entries.push(FileSystemSandboxEntry::new(
                config.cwd.join(GLOBAL_AGENTS_FILENAME).into(),
                FileSystemAccessMode::Deny,
            ));
            config
                .permissions
                .set_permission_profile(PermissionProfile::from_runtime_permissions(
                    &file_system_policy,
                    NetworkSandboxPolicy::Restricted,
                ))
                .expect("test config should allow a restricted read policy");
        })
        .with_workspace_setup(|cwd, fs| async move {
            fs.write_file(
                &executor_path_uri(cwd.join(GLOBAL_AGENTS_FILENAME))?,
                PROJECT_INSTRUCTIONS.as_bytes().to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
        });
    let error = match builder.build_with_auto_env(&server).await {
        Ok(_) => {
            anyhow::bail!("thread creation must fail when project instructions are unreadable")
        }
        Err(error) => error,
    };
    let error = format!("{error:#}");
    assert!(
        error.contains("AGENTS.md"),
        "thread creation should report the unreadable project instructions: {error}"
    );

    Ok(())
}

#[cfg(target_os = "macos")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn symlinked_writable_root_reports_sandbox_failure_instead_of_session_corruption()
-> Result<()> {
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let home_path = home.path().display().to_string();
    let canonical_home_path = home.path().canonicalize()?.display().to_string();
    let visualization_target = home.path().join("visualization-target");
    std::fs::create_dir(&visualization_target)?;
    let visualization_root = home.path().join("visualizations");
    create_directory_symlink(&visualization_target, &visualization_root);

    let mut builder = test_codex().with_home(home).with_config(move |config| {
        config.project_doc_max_bytes = 1;
        let mut file_system_policy = FileSystemSandboxPolicy::read_only();
        file_system_policy.entries.push(FileSystemSandboxEntry::new(
            config.cwd.join("private.txt").into(),
            FileSystemAccessMode::Deny,
        ));
        file_system_policy.entries.push(FileSystemSandboxEntry::new(
            visualization_root.abs().into(),
            FileSystemAccessMode::Write,
        ));
        config
            .permissions
            .set_permission_profile(PermissionProfile::from_runtime_permissions(
                &file_system_policy,
                NetworkSandboxPolicy::Restricted,
            ))
            .expect("test config should allow the restricted filesystem policy");
    });

    let error = match builder.build(&server).await {
        Ok(_) => anyhow::bail!("thread creation must reject the symlinked writable root"),
        Err(error) => format!("{error:#}"),
    };

    assert!(
        error.contains("failed to prepare fs sandbox"),
        "thread creation should report the sandbox preparation failure: {error}"
    );
    assert!(
        error.contains("symlinked writable roots are not supported"),
        "thread creation should preserve the rejected writable root: {error}"
    );
    assert!(
        !error.contains("Session data under"),
        "sandbox preparation failure should not be diagnosed as session corruption: {error}"
    );
    let error = error
        .replace(&canonical_home_path, "$CODEX_HOME")
        .replace(&home_path, "$CODEX_HOME");
    insta::assert_snapshot!(error, @"
    failed to load AGENTS.md instructions for environment `local`: failed to prepare fs sandbox: failed to prepare Seatbelt sandbox: writable root $CODEX_HOME/visualizations contains symlink component $CODEX_HOME/visualizations; symlinked writable roots are not supported.
    If this writable root is at or beneath CODEX_HOME and you trust its symlink targets, set `allow_symlinked_codex_home = true` at the top level of `$CODEX_HOME/config.toml` (normally `~/.codex/config.toml`) on the execution host, then restart Codex or its executor. This opt-out trusts targets outside CODEX_HOME and targets changed between commands. It does not apply to other writable roots.
    ");

    Ok(())
}

/// Tightening permissions fails the turn before stale project instructions reach the model.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tightening_environment_read_permissions_invalidates_cached_project_instructions()
-> Result<()> {
    skip_if_target_windows!(
        Ok(()),
        "Windows restricted-token sandbox cannot enforce deny-read policies"
    );
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let global_source = write_global_file(&home, GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;
    let provider = Arc::new(WarningInstructionsProvider {
        inner: CodexHomeUserInstructionsProvider::new(home.path().to_path_buf().abs()),
        warning_active: AtomicBool::new(/*v*/ false),
    });
    let mut builder = test_codex()
        .with_home(home)
        .with_user_instructions_provider(provider.clone())
        .with_workspace_setup(|cwd, fs| async move {
            fs.write_file(
                &executor_path_uri(cwd.join(GLOBAL_AGENTS_FILENAME))?,
                PROJECT_INSTRUCTIONS.as_bytes().to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
        });
    let test = builder.build_with_auto_env(&server).await?;

    assert_eq!(
        test.codex.instruction_sources().await,
        vec![
            PathUri::from_abs_path(&global_source),
            test.workspace_path_uri(GLOBAL_AGENTS_FILENAME)?
        ]
    );

    provider
        .warning_active
        .store(/*val*/ true, Ordering::SeqCst);
    let mut file_system_policy = FileSystemSandboxPolicy::read_only();
    file_system_policy.entries.push(FileSystemSandboxEntry::new(
        test.config.cwd.join(GLOBAL_AGENTS_FILENAME).into(),
        FileSystemAccessMode::Deny,
    ));
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_policy,
        NetworkSandboxPolicy::Restricted,
    );
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(permission_profile, test.config.cwd.as_path());
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "inspect instructions after tightening permissions".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                ..Default::default()
            }),
        )
        .await?;

    let mut warnings_before_error = Vec::new();
    let EventMsg::Error(error) = wait_for_event(&test.codex, |event| {
        if let EventMsg::Warning(warning) = event {
            warnings_before_error.push(warning.message.clone());
        }
        matches!(event, EventMsg::Error(_))
    })
    .await
    else {
        unreachable!();
    };
    assert_eq!(warnings_before_error, vec![PROVIDER_WARNING.to_string()]);
    assert!(
        error.message.contains("AGENTS.md"),
        "turn should report the unreadable project instructions: {}",
        error.message
    );

    assert_eq!(
        test.codex.instruction_sources().await,
        Vec::<PathUri>::new()
    );
    assert!(
        response_mock.requests().is_empty(),
        "the denied turn must fail before sending a model request"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn loads_user_instructions_without_a_primary_environment() -> Result<()> {
    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("no-primary-environment-response"),
            ev_completed("no-primary-environment-response"),
        ]),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let global_source =
        write_global_file(home.as_ref(), GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;
    let provider = Arc::new(RecordingUserInstructionsProvider::new(Arc::new(
        CodexHomeUserInstructionsProvider::new(AbsolutePathBuf::try_from(
            home.path().to_path_buf(),
        )?),
    )));

    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_user_instructions_provider(provider.clone())
        .with_workspace_setup(|cwd, fs| async move {
            let project_agents_uri = executor_path_uri(cwd.join(GLOBAL_AGENTS_FILENAME))?;
            fs.write_file(
                &project_agents_uri,
                PROJECT_INSTRUCTIONS.as_bytes().to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
        });
    let test = builder.build_with_auto_env(&server).await?;
    assert_eq!(provider.load_count(), 1);

    let no_environment_thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(Vec::new()),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    assert_eq!(provider.load_count(), 2);
    assert_eq!(
        no_environment_thread.thread.instruction_sources().await,
        vec![PathUri::from_abs_path(&global_source)]
    );

    no_environment_thread
        .thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "inspect global instructions without an environment".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&no_environment_thread.thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let instruction_fragments = instruction_fragments(&response_mock.single_request());
    assert_eq!(instruction_fragments.len(), 1);
    assert!(instruction_fragments[0].contains(GLOBAL_INSTRUCTIONS));
    assert!(!instruction_fragments[0].contains(PROJECT_INSTRUCTIONS));

    Ok(())
}

struct ThreadInstructionsFixture {
    test: TestCodex,
    thread: Arc<codex_core::CodexThread>,
    provider: Arc<RecordingThreadInstructionsProvider>,
}

impl ThreadInstructionsFixture {
    async fn new(server: &wiremock::MockServer) -> Result<Self> {
        let home = Arc::new(TempDir::new()?);
        write_global_file(home.as_ref(), GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;
        let global_provider = Arc::new(RecordingUserInstructionsProvider::new(Arc::new(
            CodexHomeUserInstructionsProvider::new(AbsolutePathBuf::try_from(
                home.path().to_path_buf(),
            )?),
        )));
        let mut builder = test_codex()
            .with_home(home)
            .with_user_instructions_provider(global_provider.clone())
            .with_config(|config| {
                config.project_doc_max_bytes = PROJECT_INSTRUCTIONS.len();
                config
                    .features
                    .enable(Feature::DefaultModeRequestUserInput)
                    .expect("test config should allow request-user-input feature");
            })
            .with_workspace_setup(|cwd, fs| async move {
                let project_agents_uri = executor_path_uri(cwd.join(GLOBAL_AGENTS_FILENAME))?;
                fs.write_file(
                    &project_agents_uri,
                    PROJECT_INSTRUCTIONS.as_bytes().to_vec(),
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await?;
                Ok(())
            });
        let test = builder.build_with_auto_env(server).await?;
        assert_eq!(global_provider.load_count(), 1);
        let provider = Arc::new(RecordingThreadInstructionsProvider::with_text(
            TASK_USER_INSTRUCTIONS,
        ));
        let thread = test
            .thread_manager
            .start_thread(StartThreadOptions {
                environments: Some(vec![test.executor_environment().selection().clone()]),
                thread_instructions_provider: Some(provider.clone()),
                ..StartThreadOptions::new(test.config.clone())
            })
            .await?
            .thread;
        Ok(Self {
            test,
            thread,
            provider,
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn thread_provider_composes_and_clears_only_its_instructions() -> Result<()> {
    let server = start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        ["initial", "unchanged", "cleared", "unchanged-empty"]
            .map(|id| sse(vec![ev_response_created(id), ev_completed(id)]))
            .to_vec(),
    )
    .await;
    let fixture = ThreadInstructionsFixture::new(&server).await?;
    let sources = vec![
        PathUri::from_abs_path(&fixture.test.config.codex_home.join(GLOBAL_AGENTS_FILENAME)),
        fixture.test.workspace_path_uri(GLOBAL_AGENTS_FILENAME)?,
    ];
    assert_eq!(fixture.thread.instruction_sources().await, sources);
    for prompt in [
        "inspect task instructions",
        "inspect unchanged task instructions",
    ] {
        submit_thread_turn(&fixture.thread, prompt).await?;
    }
    for instructions in [
        Some(Instructions {
            text: String::new(),
            source: None,
        }),
        None,
    ] {
        fixture.provider.set_instructions(instructions);
        submit_thread_turn(&fixture.thread, "inspect cleared task instructions").await?;
    }
    assert!(fixture.provider.load_count() > 1);
    assert_eq!(fixture.thread.instruction_sources().await, sources);

    let cwd = &fixture.test.executor_environment().selection().cwd;
    let initial = expected_instruction_fragment(
        cwd,
        &format!(
            "{GLOBAL_INSTRUCTIONS}\n\n{TASK_USER_INSTRUCTIONS}\n\n{PROJECT_SEPARATOR}\n\n{PROJECT_INSTRUCTIONS}"
        ),
    );
    let cleared = expected_instruction_fragment(
        cwd,
        &format!(
            "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\n{GLOBAL_INSTRUCTIONS}\n\n{PROJECT_SEPARATOR}\n\n{PROJECT_INSTRUCTIONS}"
        ),
    );
    assert_eq!(
        response_mock
            .requests()
            .iter()
            .map(instruction_fragments)
            .collect::<Vec<_>>(),
        vec![
            vec![initial.clone()],
            vec![initial.clone()],
            vec![initial.clone(), cleared.clone()],
            vec![initial, cleared],
        ],
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn thread_provider_refreshes_at_the_next_step_of_an_active_turn() -> Result<()> {
    let server = start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("active-turn"),
                responses::ev_function_call(
                    "pause-for-thread-instructions",
                    "request_user_input",
                    &json!({
                        "questions": [{
                            "id": "continue",
                            "header": "Continue",
                            "question": "Continue after updating thread instructions?",
                            "options": [{
                                "label": "Yes (Recommended)",
                                "description": "Continue the current turn."
                            }, {
                                "label": "No",
                                "description": "Stop the current turn."
                            }]
                        }]
                    })
                    .to_string(),
                ),
                ev_completed("active-turn"),
            ]),
            sse(vec![
                ev_response_created("updated"),
                ev_completed("updated"),
            ]),
        ],
    )
    .await;
    let fixture = ThreadInstructionsFixture::new(&server).await?;
    fixture
        .thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "inspect instructions updated during the active turn".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let EventMsg::RequestUserInput(request) = wait_for_event(&fixture.thread, |event| {
        matches!(event, EventMsg::RequestUserInput(_))
    })
    .await
    else {
        unreachable!("wait_for_event should return the request-user-input event")
    };
    // Only the host's provider changes; no explicit instruction update or resume call.
    fixture.provider.set_instructions(Some(Instructions {
        text: UPDATED_TASK_USER_INSTRUCTIONS.to_string(),
        source: None,
    }));
    fixture
        .thread
        .submit(Op::UserInputAnswer {
            id: request.turn_id,
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "continue".to_string(),
                    RequestUserInputAnswer {
                        answers: vec!["Yes (Recommended)".to_string()],
                    },
                )]),
            },
        })
        .await?;
    wait_for_event(&fixture.thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let cwd = &fixture.test.executor_environment().selection().cwd;
    let initial = expected_instruction_fragment(
        cwd,
        &format!(
            "{GLOBAL_INSTRUCTIONS}\n\n{TASK_USER_INSTRUCTIONS}\n\n{PROJECT_SEPARATOR}\n\n{PROJECT_INSTRUCTIONS}"
        ),
    );
    let updated = expected_instruction_fragment(
        cwd,
        &format!(
            "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\n{GLOBAL_INSTRUCTIONS}\n\n{UPDATED_TASK_USER_INSTRUCTIONS}\n\n{PROJECT_SEPARATOR}\n\n{PROJECT_INSTRUCTIONS}"
        ),
    );
    assert_eq!(
        response_mock
            .requests()
            .iter()
            .map(instruction_fragments)
            .collect::<Vec<_>>(),
        vec![vec![initial.clone()], vec![initial, updated]],
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn thread_provider_enforces_its_own_limit_before_startup_and_sampling() -> Result<()> {
    let server = start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        ["initial", "full-thread-budget"]
            .map(|id| sse(vec![ev_response_created(id), ev_completed(id)]))
            .to_vec(),
    )
    .await;
    let fixture = ThreadInstructionsFixture::new(&server).await?;
    let oversized = Instructions {
        text: "x".repeat(approx_bytes_for_tokens(/*tokens*/ 10_001)),
        source: None,
    };
    let error = fixture
        .test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![
                fixture.test.executor_environment().selection().clone(),
            ]),
            thread_instructions_provider: Some(Arc::new(RecordingThreadInstructionsProvider::new(
                Some(oversized.clone()),
            ))),
            ..StartThreadOptions::new(fixture.test.config.clone())
        })
        .await
        .err()
        .ok_or_else(|| anyhow!("oversized instructions must fail before thread creation"))?;
    assert!(matches!(
        error.details(),
        CodexErrorDetails::InvalidRequest(_)
    ));
    assert!(error.to_string().contains("10000 estimated tokens"));

    submit_thread_turn(&fixture.thread, "inspect valid instructions").await?;
    fixture.provider.set_instructions(Some(oversized));
    fixture
        .provider
        .set_warnings(vec!["provider refresh warning".to_string()]);
    let request_count = response_mock.requests().len();
    fixture
        .thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "reject oversized instructions before sampling".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let EventMsg::Warning(warning) = wait_for_event(&fixture.thread, |event| {
        matches!(event, EventMsg::Warning(_) | EventMsg::Error(_))
    })
    .await
    else {
        panic!("provider warnings must be emitted before validation errors");
    };
    assert_eq!(warning.message, "provider refresh warning");
    let EventMsg::Error(error) =
        wait_for_event(&fixture.thread, |event| matches!(event, EventMsg::Error(_))).await
    else {
        unreachable!();
    };
    assert!(error.message.contains("10000 estimated tokens"));
    wait_for_event(&fixture.thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(response_mock.requests().len(), request_count);
    fixture.provider.set_warnings(Vec::new());

    // The full thread budget is independent of global instructions and wrapping.
    // Changing environments must also remove the previously selected repository docs.
    let host_text = "x".repeat(approx_bytes_for_tokens(/*tokens*/ 10_000));
    let expected = expected_provider_only_instruction_fragment(&format!(
        "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\n{GLOBAL_INSTRUCTIONS}\n\n{host_text}"
    ));
    fixture.provider.set_instructions(Some(Instructions {
        text: host_text,
        source: None,
    }));
    fixture
        .thread
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "inspect instructions without the previous environment".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    fixture.test.config.cwd.clone(),
                    Vec::new(),
                )),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_event(&fixture.thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(instruction_fragments(&requests[1]).last(), Some(&expected));
    Ok(())
}

#[derive(Clone, Copy)]
enum InstructionForkSource {
    LiveRollout,
    OfflineHistory,
    OfflinePrepared,
}

#[test_case::test_case(InstructionForkSource::LiveRollout; "live snapshot")]
#[test_case::test_case(InstructionForkSource::OfflineHistory; "offline history provider")]
#[test_case::test_case(InstructionForkSource::OfflinePrepared; "offline prepared provider")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fork_preserves_thread_instructions(source: InstructionForkSource) -> Result<()> {
    let server = start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        ["parent", "fork", "updated-fork"]
            .map(|id| sse(vec![ev_response_created(id), ev_completed(id)]))
            .to_vec(),
    )
    .await;
    let history_mode = match source {
        InstructionForkSource::LiveRollout | InstructionForkSource::OfflineHistory => {
            ThreadHistoryMode::Legacy
        }
        InstructionForkSource::OfflinePrepared => ThreadHistoryMode::Paginated,
    };
    let mut builder = test_codex()
        .with_config(|config| {
            config
                .features
                .enable(Feature::Sqlite)
                .expect("enable local persistence");
        })
        .with_history_mode(history_mode);
    let test = builder.build_with_auto_env(&server).await?;
    let parent_provider = Arc::new(RecordingThreadInstructionsProvider::with_text(
        TASK_USER_INSTRUCTIONS,
    ));
    let parent = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(Vec::new()),
            thread_instructions_provider: Some(parent_provider.clone()),
            history_mode: Some(history_mode),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    submit_thread_turn(&parent.thread, "persist parent instructions").await?;
    parent.thread.ensure_rollout_materialized().await;
    parent.thread.flush_rollout().await?;
    let parent_id = parent.thread_id;
    let rollout_path = parent
        .thread
        .rollout_path()
        .expect("persisted parent rollout");
    let parent_loads = parent_provider.load_count();
    let offline = !matches!(source, InstructionForkSource::LiveRollout);
    if offline {
        parent.thread.shutdown_and_wait().await?;
        test.thread_manager
            .remove_thread_if_matches(&parent_id, &parent.thread)
            .await;
    }
    let fork_provider = Arc::new(RecordingThreadInstructionsProvider::with_text(
        TASK_USER_INSTRUCTIONS,
    ));
    let options = StartThreadOptions {
        environments: Some(Vec::new()),
        thread_instructions_provider: offline
            .then(|| fork_provider.clone() as Arc<dyn ThreadInstructionsProvider>),
        ..StartThreadOptions::new(test.config.clone())
    };
    let fork = match source {
        InstructionForkSource::LiveRollout => {
            test.thread_manager
                .fork_thread(ForkSnapshot::Interrupted, options, rollout_path)
                .await?
        }
        InstructionForkSource::OfflineHistory => {
            let stored = test
                .thread_store
                .load_latest_model_context(LoadThreadHistoryParams {
                    thread_id: parent_id,
                    include_archived: true,
                })
                .await?;
            let history = InitialHistory::Resumed(ResumedHistory {
                conversation_id: parent_id,
                history: Arc::new(stored.items),
                rollout_path: None,
            });
            test.thread_manager
                .fork_thread_from_history(ForkSnapshot::Interrupted, options, history)
                .await?
        }
        InstructionForkSource::OfflinePrepared => {
            let prepared = test
                .thread_store
                .prepare_fork(PrepareForkParams {
                    thread_id: parent_id,
                    boundary: ForkBoundary::Latest,
                })
                .await?;
            test.thread_manager
                .fork_prepared_thread(options, prepared)
                .await?
        }
    };
    submit_thread_turn(&fork.thread, "continue with inherited instructions").await?;
    // Live forks inherit only a snapshot; offline forks retain their own provider.
    // Neither path may call the original task's provider.
    for provider in [&parent_provider, &fork_provider] {
        provider.set_instructions(Some(Instructions {
            text: UPDATED_TASK_USER_INSTRUCTIONS.to_string(),
            source: None,
        }));
    }
    submit_thread_turn(&fork.thread, "continue after source update").await?;
    assert_eq!(parent_provider.load_count(), parent_loads);
    let initial = expected_provider_only_instruction_fragment(TASK_USER_INSTRUCTIONS);
    let final_fragments = if offline {
        vec![
            initial.clone(),
            expected_provider_only_instruction_fragment(&format!(
                "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\n{UPDATED_TASK_USER_INSTRUCTIONS}"
            )),
        ]
    } else {
        vec![initial.clone()]
    };
    assert_eq!(
        response_mock
            .requests()
            .iter()
            .map(instruction_fragments)
            .collect::<Vec<_>>(),
        vec![vec![initial.clone()], vec![initial], final_fragments]
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn thread_provider_lives_with_its_session_across_resume() -> Result<()> {
    let server = start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        ["updated-live-session", "cold-session"]
            .map(|id| sse(vec![ev_response_created(id), ev_completed(id)]))
            .to_vec(),
    )
    .await;
    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;
    let provider = Arc::new(RecordingThreadInstructionsProvider::new(
        /*instructions*/ None,
    ));
    let started = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(Vec::new()),
            thread_instructions_provider: Some(provider.clone()),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    assert_eq!(provider.load_count(), 1);
    let (_, history) = persisted_resume_history(&started.thread).await?;
    let cold_provider = Arc::new(RecordingThreadInstructionsProvider::with_text(
        "cold session instructions",
    ));
    provider.set_instructions(Some(Instructions {
        text: UPDATED_TASK_USER_INSTRUCTIONS.to_string(),
        source: None,
    }));
    let same_provider = Arc::new(RecordingThreadInstructionsProvider::with_text(
        UPDATED_TASK_USER_INSTRUCTIONS,
    ));
    for supplied in [
        None,
        Some(same_provider.clone() as Arc<dyn ThreadInstructionsProvider>),
        Some(cold_provider.clone() as Arc<dyn ThreadInstructionsProvider>),
    ] {
        let resumed = test
            .thread_manager
            .start_thread(StartThreadOptions {
                initial_history: history.clone(),
                thread_instructions_provider: supplied,
                ..StartThreadOptions::new(test.config.clone())
            })
            .await?;
        assert!(Arc::ptr_eq(&resumed.thread, &started.thread));
        assert_eq!(provider.load_count(), 1);
        assert_eq!(same_provider.load_count(), 0);
        assert_eq!(cold_provider.load_count(), 0);
    }

    // Reattachment does no loading; the existing Session pulls at the next model step.
    submit_thread_turn(&started.thread, "load instructions added after creation").await?;
    assert_eq!(provider.load_count(), 2);
    assert_eq!(same_provider.load_count(), 0);
    let (_, history) = persisted_resume_history(&started.thread).await?;
    started.thread.shutdown_and_wait().await?;
    let resumed = test
        .thread_manager
        .start_thread(StartThreadOptions {
            initial_history: history,
            environments: Some(Vec::new()),
            thread_instructions_provider: Some(cold_provider.clone()),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    assert!(!Arc::ptr_eq(&resumed.thread, &started.thread));
    assert_eq!(cold_provider.load_count(), 1);
    submit_thread_turn(&resumed.thread, "load cold session instructions").await?;
    assert_eq!(provider.load_count(), 2);
    let requests = response_mock.requests();
    assert_single_instruction_fragment(
        &requests[0],
        &expected_provider_only_instruction_fragment(UPDATED_TASK_USER_INSTRUCTIONS),
    );
    assert_eq!(
        instruction_fragments(&requests[1]).last(),
        Some(&expected_provider_only_instruction_fragment(
            "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\ncold session instructions",
        )),
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_thread_composes_global_before_project_and_reports_sources() -> Result<()> {
    // Set up one global source, one project source, and two ordinary model turns.
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("response-1"),
                responses::ev_completed("response-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("response-2"),
                responses::ev_completed("response-2"),
            ]),
        ],
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let global_source =
        write_global_file(home.as_ref(), GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;

    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_workspace_setup(|cwd, fs| async move {
            let agents_md_uri = executor_path_uri(cwd.join("AGENTS.md"))?;
            fs.write_file(
                &agents_md_uri,
                PROJECT_INSTRUCTIONS.as_bytes().to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
        });
    let test = builder.build_with_auto_env(&server).await?;
    let creation_sources = vec![
        PathUri::from_abs_path(&global_source),
        test.workspace_path_uri(GLOBAL_AGENTS_FILENAME)?,
    ];

    // Confirm the thread records both creation-time sources in composition order.
    assert_eq!(test.codex.instruction_sources().await, creation_sources);

    // Materialize the initial snapshot, then rewrite both selected files in place before another
    // ordinary turn.
    test.submit_turn("first turn").await?;
    let rewritten_global_source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_FILENAME,
        NEW_GLOBAL_INSTRUCTIONS,
    )?;
    test.fs()
        .write_file(
            &test.workspace_path_uri(GLOBAL_AGENTS_FILENAME)?,
            NEW_PROJECT_INSTRUCTIONS.as_bytes().to_vec(),
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    assert_eq!(
        rewritten_global_source, global_source,
        "same-path mutation should retain the selected global source path"
    );
    test.submit_turn("second turn").await?;

    // The global provider refreshes, while repository discovery keeps its cached snapshot.
    // Append the changed instructions without rewriting the earlier model input.
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let expected_contents =
        format!("{GLOBAL_INSTRUCTIONS}\n\n{PROJECT_SEPARATOR}\n\n{PROJECT_INSTRUCTIONS}");
    let expected_fragment = expected_instruction_fragment(
        &test.executor_environment().selection().cwd,
        &expected_contents,
    );
    let fragments = instruction_fragments(&requests[0]);
    assert_eq!(fragments, vec![expected_fragment.clone()]);
    let updated_fragment = expected_instruction_fragment(
        &test.executor_environment().selection().cwd,
        &format!(
            "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\n{NEW_GLOBAL_INSTRUCTIONS}\n\n{PROJECT_SEPARATOR}\n\n{PROJECT_INSTRUCTIONS}"
        ),
    );
    assert_eq!(
        instruction_fragments(&requests[1]),
        vec![expected_fragment, updated_fragment]
    );
    let rendered = fragments
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("expected one rendered instruction fragment"))?;
    let global_position = rendered.find(GLOBAL_INSTRUCTIONS).ok_or_else(|| {
        anyhow!(
            "expected rendered instructions to contain {GLOBAL_INSTRUCTIONS:?}; observed: {rendered}"
        )
    })?;
    let project_position = rendered.find(PROJECT_INSTRUCTIONS).ok_or_else(|| {
        anyhow!(
            "expected rendered instructions to contain {PROJECT_INSTRUCTIONS:?}; observed: {rendered}"
        )
    })?;
    assert!(
        global_position < project_position,
        "global instructions should precede project instructions: {rendered}"
    );
    assert!(
        rendered.contains(PROJECT_SEPARATOR),
        "expected rendered instructions to contain {PROJECT_SEPARATOR:?}; observed: {rendered}"
    );
    assert_eq!(
        test.codex.instruction_sources().await,
        creation_sources,
        "same-path global refresh preserves source paths and composition order"
    );
    let first_input = requests[0].input();
    let second_input = requests[1].input();
    assert_eq!(
        second_input.get(..first_input.len()),
        Some(first_input.as_slice()),
        "the ordinary second turn should retain the cached prefix"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_environment_project_instructions_share_one_byte_budget() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_no_remote_env!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("multi-env-budget-response"),
            responses::ev_completed("multi-env-budget-response"),
        ]),
    )
    .await;
    let local_root = TempDir::new()?;
    std::fs::write(local_root.path().join(GLOBAL_AGENTS_FILENAME), "VWXYZ")?;
    let mut builder = test_codex()
        .with_config(|config| config.project_doc_max_bytes = 7)
        .with_workspace_setup(|cwd, fs| async move {
            fs.write_file(
                &executor_path_uri(cwd.join(GLOBAL_AGENTS_FILENAME))?,
                b"ABCDE".to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
        });
    let test = builder.build_with_remote_and_local_env(&server).await?;
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![
                TurnEnvironmentSelection {
                    environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                    cwd: test.executor_environment().selection().cwd.clone(),
                    workspace_roots: vec![test.executor_environment().selection().cwd.clone()],
                    config: EnvironmentConfigState::FromThread,
                },
                TurnEnvironmentSelection {
                    environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                    cwd: PathUri::from_host_native_path(local_root.path())?,
                    workspace_roots: vec![PathUri::from_host_native_path(local_root.path())?],
                    config: EnvironmentConfigState::FromThread,
                },
            ]),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;

    submit_thread_turn(&thread.thread, "inspect the shared AGENTS.md budget").await?;

    let contents = format!(
        "for `{REMOTE_ENVIRONMENT_ID}` with root {}\n\nABCDE\n\nfor `{LOCAL_ENVIRONMENT_ID}` with root {}\n\nVW",
        test.executor_environment()
            .selection()
            .cwd
            .inferred_native_path_string(),
        local_root.path().display(),
    );
    let expected =
        format!("# AGENTS.md instructions\n\n<INSTRUCTIONS>\n{contents}\n</INSTRUCTIONS>");
    assert_single_instruction_fragment(&response_mock.single_request(), &expected);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_environment_thread_refreshes_global_and_keeps_repository_snapshot() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_no_remote_env!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("multi-env-response-1"),
                responses::ev_completed("multi-env-response-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("multi-env-response-2"),
                responses::ev_completed("multi-env-response-2"),
            ]),
        ],
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let global_source =
        write_global_file(home.as_ref(), GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;
    let provider = Arc::new(RecordingUserInstructionsProvider::new(Arc::new(
        CodexHomeUserInstructionsProvider::new(AbsolutePathBuf::try_from(
            home.path().to_path_buf(),
        )?),
    )));
    let local_root = TempDir::new()?;
    let local_source = local_root.path().join(GLOBAL_AGENTS_FILENAME);
    std::fs::write(&local_source, "local project instructions")?;
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_user_instructions_provider(provider.clone())
        .with_workspace_setup(|cwd, fs| async move {
            fs.write_file(
                &executor_path_uri(cwd.join(GLOBAL_AGENTS_FILENAME))?,
                b"remote project instructions".to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
        });
    let test = builder.build_with_remote_and_local_env(&server).await?;
    let remote_source = test.config.cwd.join(GLOBAL_AGENTS_FILENAME);
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![
                TurnEnvironmentSelection {
                    environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                    cwd: test.executor_environment().selection().cwd.clone(),
                    workspace_roots: vec![test.executor_environment().selection().cwd.clone()],
                    config: EnvironmentConfigState::FromThread,
                },
                TurnEnvironmentSelection {
                    environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                    cwd: PathUri::from_host_native_path(local_root.path())?,
                    workspace_roots: vec![PathUri::from_host_native_path(local_root.path())?],
                    config: EnvironmentConfigState::FromThread,
                },
            ]),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    assert_eq!(provider.load_count(), 2);
    assert_eq!(
        thread.thread.instruction_sources().await,
        vec![
            PathUri::from_abs_path(&global_source),
            executor_path_uri(&remote_source)?,
            PathUri::from_host_native_path(&local_source)?,
        ]
    );

    submit_thread_turn(&thread.thread, "first multi-environment turn").await?;

    let new_global_source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_OVERRIDE_FILENAME,
        NEW_GLOBAL_INSTRUCTIONS,
    )?;
    test.fs()
        .write_file(
            &executor_path_uri(test.config.cwd.join(GLOBAL_AGENTS_OVERRIDE_FILENAME))?,
            b"new remote project instructions".to_vec(),
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    std::fs::write(
        local_root.path().join(GLOBAL_AGENTS_OVERRIDE_FILENAME),
        "new local project instructions",
    )?;
    submit_thread_turn(&thread.thread, "second multi-environment turn").await?;

    let contents = format!(
        "{GLOBAL_INSTRUCTIONS}\n\nfor `{REMOTE_ENVIRONMENT_ID}` with root {}\n\nremote project instructions\n\nfor `{LOCAL_ENVIRONMENT_ID}` with root {}\n\nlocal project instructions",
        test.executor_environment()
            .selection()
            .cwd
            .inferred_native_path_string(),
        local_root.path().display(),
    );
    let expected =
        format!("# AGENTS.md instructions\n\n<INSTRUCTIONS>\n{contents}\n</INSTRUCTIONS>");
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    assert_single_instruction_fragment(&requests[0], &expected);
    let replacement = expected_provider_only_instruction_fragment(&format!(
        "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\n{}",
        contents.replace(GLOBAL_INSTRUCTIONS, NEW_GLOBAL_INSTRUCTIONS),
    ));
    assert_eq!(
        instruction_fragments(&requests[1]),
        vec![expected, replacement]
    );
    assert_eq!(provider.load_count(), 4);
    assert_eq!(
        thread.thread.instruction_sources().await,
        vec![
            PathUri::from_abs_path(&new_global_source),
            executor_path_uri(&remote_source)?,
            PathUri::from_host_native_path(&local_source)?,
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn global_instruction_warnings_reappear_only_after_recovery() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        (0..4)
            .map(|index| {
                responses::sse(vec![
                    responses::ev_response_created(&format!("response-{index}")),
                    responses::ev_completed(&format!("response-{index}")),
                ])
            })
            .collect(),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    write_global_file(home.as_ref(), GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;
    let override_path = home.path().join(GLOBAL_AGENTS_OVERRIDE_FILENAME);
    let create_unreadable_override = || {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(GLOBAL_AGENTS_OVERRIDE_FILENAME, &override_path)
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(GLOBAL_AGENTS_OVERRIDE_FILENAME, &override_path)
        }
    };
    create_unreadable_override()?;
    let read_error = std::fs::read(&override_path).expect_err("symlink loop must be unreadable");
    let expected_warning = format!(
        "Failed to read global AGENTS.md instructions from `{}`: {read_error}",
        override_path.display()
    );
    let mut builder = test_codex().with_home(Arc::clone(&home));
    let test = builder.build_with_auto_env(&server).await?;
    wait_for_event(
        &test.codex,
        |event| matches!(event, EventMsg::Warning(warning) if warning.message == expected_warning),
    )
    .await;

    for (warning_active, expected_warnings) in [
        (true, Vec::new()),
        (false, Vec::new()),
        (true, vec![expected_warning]),
        (true, Vec::new()),
    ] {
        std::fs::remove_file(&override_path)?;
        if warning_active {
            create_unreadable_override()?;
        } else {
            std::fs::write(&override_path, "")?;
        }
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "check the instructions".to_string(),
                text_elements: Vec::new(),
            }]))
            .await?;
        let mut warnings = Vec::new();
        wait_for_event(&test.codex, |event| {
            if let EventMsg::Warning(warning) = event {
                warnings.push(warning.message.clone());
            }
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        assert_eq!(warnings, expected_warnings);
    }
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 4);
    for request in &requests {
        assert_single_instruction_fragment(
            request,
            &expected_provider_only_instruction_fragment(GLOBAL_INSTRUCTIONS),
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_utf8_global_instructions_are_lossy() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("warning-response"),
            responses::ev_completed("warning-response"),
        ]),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_FILENAME,
        b"global\xFFinstructions",
    )?;

    let mut builder = test_codex().with_home(home);
    let test = builder.build(&server).await?;
    test.submit_turn("inspect lossy global instructions")
        .await?;

    assert_eq!(
        test.codex.instruction_sources().await,
        vec![PathUri::from_abs_path(&source)]
    );
    let expected_fragment =
        expected_provider_only_instruction_fragment("global\u{FFFD}instructions");
    assert_single_instruction_fragment(&response_mock.single_request(), &expected_fragment);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cold_resume_invalidates_deleted_legacy_agents_md_once() -> Result<()> {
    // Set up an initial turn and a later cold-resumed turn against the same rollout.
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("initial-response"),
                responses::ev_completed("initial-response"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resumed-response"),
                responses::ev_completed("resumed-response"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("second-resumed-response"),
                responses::ev_completed("second-resumed-response"),
            ]),
            responses::sse_completed("refreshed-resumed-response"),
        ],
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let old_source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_FILENAME,
        OLD_GLOBAL_INSTRUCTIONS,
    )?;

    // Create the initial thread and persist its creation-time instruction snapshot.
    let mut initial_builder = test_codex().with_home(Arc::clone(&home));
    let initial = initial_builder.build(&server).await?;

    // Assert the pre-resume thread reports the source used to create its snapshot.
    assert_eq!(
        initial.codex.instruction_sources().await,
        vec![PathUri::from_abs_path(&old_source)],
        "initial thread reports the creation-time global source"
    );
    initial.submit_turn("persist instructions").await?;
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");
    initial.codex.submit(Op::Shutdown).await?;
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::ShutdownComplete)
    })
    .await;

    // Simulate a rollout written before AGENTS.md had a persisted WorldState section.
    remove_agents_md_world_state_section(&rollout_path)?;

    std::fs::remove_file(old_source.as_path())?;
    let mut resume_builder = test_codex().with_home(Arc::clone(&home));
    let resumed = resume_builder
        .resume(&server, Arc::clone(&home), rollout_path)
        .await?;

    // Model history still contains the old fragment, but the source no longer exists.
    assert_eq!(
        resumed.codex.instruction_sources().await,
        Vec::<PathUri>::new(),
        "resume reports no deleted instruction source"
    );

    resumed.submit_turn("continue resumed thread").await?;
    resumed.submit_turn("continue again").await?;
    write_global_file(&home, GLOBAL_AGENTS_FILENAME, NEW_GLOBAL_INSTRUCTIONS)?;
    resumed.submit_turn("refresh after resume").await?;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 4);
    let initial_input = requests[0].input();
    let resumed_input = requests[1].input();
    assert_eq!(
        resumed_input.get(..initial_input.len()),
        Some(initial_input.as_slice()),
        "cold resume should replay the original structured input prefix"
    );
    let initial = expected_provider_only_instruction_fragment(OLD_GLOBAL_INSTRUCTIONS);
    let removal = expected_provider_only_instruction_fragment(
        "The previously provided AGENTS.md instructions no longer apply.",
    );
    assert_eq!(instruction_fragments(&requests[0]), vec![initial.clone()]);
    assert_eq!(
        instruction_fragments(&requests[1]),
        vec![initial.clone(), removal.clone()]
    );
    assert_eq!(
        instruction_fragments(&requests[2]),
        vec![initial.clone(), removal.clone()]
    );
    let replacement = expected_provider_only_instruction_fragment(NEW_GLOBAL_INSTRUCTIONS);
    assert_eq!(
        instruction_fragments(&requests[3]),
        vec![initial, removal, replacement]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fork_injects_changed_agents_md_once() -> Result<()> {
    // Set up a parent turn and a later fork turn against the parent's rollout.
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("parent-response"),
                responses::ev_completed("parent-response"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("fork-response"),
                responses::ev_completed("fork-response"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("second-fork-response"),
                responses::ev_completed("second-fork-response"),
            ]),
            responses::sse_completed("refreshed-fork-response"),
        ],
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_FILENAME,
        OLD_GLOBAL_INSTRUCTIONS,
    )?;

    // Create the parent and persist its creation-time instruction snapshot.
    let mut builder = test_codex().with_home(Arc::clone(&home));
    let parent = builder.build(&server).await?;

    // Assert the parent reports the source used to create its snapshot.
    assert_eq!(
        parent.codex.instruction_sources().await,
        vec![PathUri::from_abs_path(&source)],
        "parent reports the creation-time global source"
    );
    parent.submit_turn("persist instructions").await?;
    parent.codex.ensure_rollout_materialized().await;
    parent.codex.flush_rollout().await?;
    let rollout_path = parent.codex.rollout_path().expect("rollout path");

    // Add a preferred override source, then fork with freshly loaded configuration.
    let new_source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_OVERRIDE_FILENAME,
        NEW_GLOBAL_INSTRUCTIONS,
    )?;
    assert_ne!(source, new_source);
    let mut fork_config = load_default_config_for_test(home.as_ref()).await;
    fork_config.cwd = parent.config.cwd.clone();
    fork_config.model = parent.config.model.clone();
    fork_config.model_provider = parent.config.model_provider.clone();
    fork_config.model_catalog = parent.config.model_catalog.clone();
    fork_config.codex_self_exe = parent.config.codex_self_exe.clone();
    fork_config
        .features
        .enable(Feature::ContentItemKinds)
        .expect("test config should allow ContentItemKinds override");
    let forked = parent
        .thread_manager
        .fork_thread(
            ForkSnapshot::Interrupted,
            codex_core::StartThreadOptions::new(fork_config),
            rollout_path,
        )
        .await?;

    // Assert the fork reports the new source before issuing its first turn.
    assert_eq!(
        forked.thread.instruction_sources().await,
        vec![PathUri::from_abs_path(&new_source)],
        "fork config should reflect the newly loaded global source"
    );

    submit_thread_turn(&forked.thread, "continue fork").await?;
    submit_thread_turn(&forked.thread, "continue fork again").await?;
    write_global_file(
        &home,
        GLOBAL_AGENTS_OVERRIDE_FILENAME,
        "instructions changed after fork",
    )?;
    submit_thread_turn(&forked.thread, "refresh after fork").await?;

    // Assert the forked model request replays the parent's exact structured history.
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 4);
    let parent_input = requests[0].input();
    let fork_input = requests[1].input();
    assert_eq!(
        fork_input.get(..parent_input.len()),
        Some(parent_input.as_slice()),
        "fork should replay the parent's original structured input prefix"
    );
    let initial = expected_provider_only_instruction_fragment(OLD_GLOBAL_INSTRUCTIONS);
    let replacement = expected_provider_only_instruction_fragment(&format!(
        "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\n{NEW_GLOBAL_INSTRUCTIONS}"
    ));
    assert_eq!(instruction_fragments(&requests[0]), vec![initial.clone()]);
    assert_eq!(
        instruction_fragments(&requests[1]),
        vec![initial.clone(), replacement.clone()]
    );
    assert_eq!(
        instruction_fragments(&requests[2]),
        vec![initial.clone(), replacement.clone()]
    );
    let refreshed = expected_provider_only_instruction_fragment(
        "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\ninstructions changed after fork",
    );
    assert_eq!(
        instruction_fragments(&requests[3]),
        vec![initial, replacement, refreshed]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forked_subagent_replays_parent_applied_global_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));
    run_subagent_global_instruction_case(/*fork_context*/ true).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_subagent_uses_parent_applied_instructions_without_parent_history() -> Result<()> {
    skip_if_no_network!(Ok(()));
    run_subagent_global_instruction_case(/*fork_context*/ false).await
}

async fn run_subagent_global_instruction_case(fork_context: bool) -> Result<()> {
    // Set up matched responses for the parent seed, spawn call, child turn, and parent follow-up.
    let server = responses::start_mock_server().await;
    let parent_prompt = if fork_context {
        SPAWN_PARENT_PROMPT
    } else {
        SPAWN_FRESH_PARENT_PROMPT
    };
    let seed_mock = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| request_body_contains(request, SPAWN_SEED_PROMPT),
        responses::sse(vec![
            responses::ev_response_created("seed-response"),
            responses::ev_assistant_message("seed-message", "seeded"),
            responses::ev_completed("seed-response"),
        ]),
    )
    .await;
    let spawn_args = serde_json::to_string(&json!({
        "message": SPAWN_CHILD_PROMPT,
        "fork_context": fork_context,
    }))?;
    let spawn_mock = responses::mount_sse_once_match(
        &server,
        move |request: &wiremock::Request| request_body_contains(request, parent_prompt),
        responses::sse(vec![
            responses::ev_response_created("spawn-response"),
            responses::ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                "multi_agent_v1",
                "spawn_agent",
                &spawn_args,
            ),
            responses::ev_completed("spawn-response"),
        ]),
    )
    .await;
    let child_mock = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            request_body_contains(request, SPAWN_CHILD_PROMPT)
                && !request_body_contains(request, SPAWN_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("child-response"),
            responses::ev_assistant_message("child-message", "done"),
            responses::ev_completed("child-response"),
        ]),
    )
    .await;
    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| request_body_contains(request, SPAWN_CALL_ID),
        responses::sse(vec![
            responses::ev_response_created("spawn-follow-up-response"),
            responses::ev_assistant_message("spawn-follow-up-message", "child started"),
            responses::ev_completed("spawn-follow-up-response"),
        ]),
    )
    .await;

    // Create the parent thread, record its source, and seed the history inherited by the child.
    let home = Arc::new(TempDir::new()?);
    let source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_FILENAME,
        OLD_GLOBAL_INSTRUCTIONS,
    )?;
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(|config| {
            let _ = config.features.enable(Feature::Collab);
            let _ = config.features.disable(Feature::EnableRequestCompression);
        });
    let test = builder.build(&server).await?;

    // Assert the parent reports the creation-time source before spawning.
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![PathUri::from_abs_path(&source)],
        "parent reports the creation-time global source before spawning"
    );
    test.submit_turn(SPAWN_SEED_PROMPT).await?;
    let seed_request = seed_mock.single_request();

    // Add a preferred override, then spawn a full-history child while observing its thread ID.
    let new_source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_OVERRIDE_FILENAME,
        NEW_GLOBAL_INSTRUCTIONS,
    )?;
    assert_ne!(source, new_source);
    let mut created_threads = test.thread_manager.subscribe_thread_created();
    test.submit_turn(parent_prompt).await?;
    let child_thread_id = tokio::time::timeout(Duration::from_secs(10), created_threads.recv())
        .await
        .map_err(|_| anyhow!("timed out waiting for the subagent thread"))??;
    let child_thread = test.thread_manager.get_thread(child_thread_id).await?;
    let spawn_request = spawn_mock.single_request();
    let child_request = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(request) = child_mock.requests().into_iter().find(|request| {
                request
                    .message_input_texts("user")
                    .iter()
                    .any(|text| text == SPAWN_CHILD_PROMPT)
            }) {
                break request;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| anyhow!("timed out waiting for the subagent request"))?;

    // The parent refreshes global instructions before spawning. The child inherits
    // that applied snapshot without independently loading the global provider.
    let expected_fragment = expected_provider_only_instruction_fragment(OLD_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&seed_request, &expected_fragment);
    let replacement = expected_provider_only_instruction_fragment(&format!(
        "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\n{NEW_GLOBAL_INSTRUCTIONS}"
    ));
    let inherited_fragments = vec![expected_fragment, replacement];
    assert_eq!(instruction_fragments(&spawn_request), inherited_fragments);
    if fork_context {
        assert_eq!(instruction_fragments(&child_request), inherited_fragments);
    } else {
        assert_single_instruction_fragment(
            &child_request,
            &expected_provider_only_instruction_fragment(NEW_GLOBAL_INSTRUCTIONS),
        );
    }
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![PathUri::from_abs_path(&new_source)],
        "parent reports the refreshed global source"
    );
    assert_eq!(
        child_thread.instruction_sources().await,
        vec![PathUri::from_abs_path(&new_source)],
        "subagent reports the parent's applied global source"
    );
    if fork_context {
        let seed_input = seed_request.input();
        let child_input = child_request.input();
        assert_eq!(
            child_input.get(..seed_input.len()),
            Some(seed_input.as_slice()),
            "forked subagent should replay the parent's original structured input prefix"
        );
    } else {
        let child_user_texts = child_request.message_input_texts("user");
        assert_eq!(
            child_user_texts
                .iter()
                .filter(|text| text.as_str() == SPAWN_SEED_PROMPT)
                .count(),
            0,
            "fresh-context subagent should omit parent user history; observed: {child_user_texts:?}"
        );
        assert_eq!(
            child_user_texts
                .iter()
                .filter(|text| text.as_str() == SPAWN_CHILD_PROMPT)
                .count(),
            1,
            "fresh-context subagent should contain its own prompt exactly once; observed: {child_user_texts:?}"
        );
    }

    wait_for_event(&child_thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    write_global_file(
        &home,
        GLOBAL_AGENTS_OVERRIDE_FILENAME,
        "instructions changed after child creation",
    )?;
    let child_follow_up =
        responses::mount_sse_once(&server, responses::sse_completed("child-follow-up")).await;
    submit_thread_turn(&child_thread, "continue with inherited instructions").await?;
    assert_eq!(
        instruction_fragments(&child_follow_up.single_request()),
        instruction_fragments(&child_request),
        "children keep the parent's snapshot even after the global source changes",
    );
    let parent_follow_up =
        responses::mount_sse_once(&server, responses::sse_completed("parent-refresh")).await;
    test.submit_turn("refresh the parent independently").await?;
    let refreshed = expected_provider_only_instruction_fragment(
        "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\ninstructions changed after child creation",
    );
    let mut parent_fragments = instruction_fragments(&spawn_request);
    parent_fragments.push(refreshed);
    assert_eq!(
        instruction_fragments(&parent_follow_up.single_request()),
        parent_fragments
    );

    Ok(())
}
