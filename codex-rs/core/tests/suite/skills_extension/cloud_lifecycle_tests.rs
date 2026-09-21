//! Cloud catalog refresh and auth-scoped retention through real Core turns.

use super::*;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_mcp::McpResourceClient;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicUsize;

struct CloudTurnProvider {
    reply: Mutex<Result<Option<(&'static str, u32)>, ()>>,
    lists: AtomicUsize,
    reads: AtomicUsize,
    resource_key: Mutex<Option<codex_mcp::McpResourceServerCacheKey>>,
}

impl SkillProvider for CloudTurnProvider {
    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(async move {
            self.lists.fetch_add(1, Ordering::SeqCst);
            *self.resource_key.lock().expect("resource key lock") = query
                .mcp_resources
                .as_ref()
                .and_then(|client| client.server_cache_key(CODEX_APPS_MCP_SERVER_NAME));
            let reply = *self.reply.lock().expect("cloud reply lock");
            let selected =
                reply.map_err(|()| SkillProviderError::new("cloud skill unavailable"))?;
            let entries = selected
                .into_iter()
                .map(|(plugin, revision)| {
                    let package = format!("skill://{plugin}/demo");
                    SkillCatalogEntry::new(
                        SkillPackageId(package.clone()),
                        SkillAuthority::new(SkillSourceKind::Cloud, CODEX_APPS_MCP_SERVER_NAME),
                        format!("{plugin}:demo"),
                        format!("Cloud {plugin} revision {revision}"),
                        SkillResourceId::new(format!("{package}/SKILL.md")),
                    )
                    .with_display_path(&package)
                    .with_alias_root(format!("skill://{plugin}"))
                })
                .collect();
            Ok(SkillCatalog {
                entries,
                warnings: Vec::new(),
            })
        })
    }

    fn read<'a>(
        &'a self,
        request: SkillReadRequest<'a>,
    ) -> SkillProviderFuture<'a, SkillReadResult> {
        Box::pin(async move {
            self.reads.fetch_add(1, Ordering::SeqCst);
            let selected = *self.reply.lock().expect("cloud reply lock");
            let (plugin, revision) = selected
                .ok()
                .flatten()
                .ok_or_else(|| SkillProviderError::new("cloud skill unavailable"))?;
            assert_eq!(
                request.resource.as_str(),
                format!("skill://{plugin}/demo/SKILL.md")
            );
            Ok(SkillReadResult {
                resource: request.resource,
                contents: format!("{plugin} instructions {revision}"),
            })
        })
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(async { Ok(SkillSearchResult::default()) })
    }
}

#[derive(Default)]
struct SessionResources(Mutex<Option<Arc<McpResourceClient>>>);

impl ThreadLifecycleContributor<Config> for SessionResources {
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            *self.0.lock().expect("session resources lock") = input.mcp_resource_client.clone();
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cloud_skills_refresh_per_turn_and_invalidate_on_auth_change() -> Result<()> {
    let server = responses::start_mock_server().await;
    let apps = AppsTestServer::mount(&server).await?;
    let calendar = ("calendar", 1);
    let updated_calendar = ("calendar", 2);
    let gmail = ("gmail", 1);
    let provider = Arc::new(CloudTurnProvider {
        reply: Mutex::new(Ok(Some(calendar))),
        lists: AtomicUsize::new(0),
        reads: AtomicUsize::new(0),
        resource_key: Mutex::new(None),
    });
    let resources = Arc::new(SessionResources::default());
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(resources.clone());
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_cloud_provider(provider.clone()),
        |_config: &Config| SkillsExtensionConfig {
            include_instructions: true,
            max_context_tokens: None,
            bundled_skills_enabled: false,
            cloud_skill_enabled: true,
            shadow_selection_enabled: false,
        },
    );
    let mut builder = apps_enabled_builder(apps.chatgpt_base_url)
        .with_exec_server_url("none")
        .with_extensions(Arc::new(extensions.build()))
        .with_auth(CodexAuth::from_external_chatgpt_tokens(
            "header.e30.initial",
            "account-a",
            /*chatgpt_plan_type*/ None,
        )?)
        .with_config(move |config| {
            config.include_skill_instructions = true;
            config.cloud_skill_enabled = true;
        });
    let test = builder.build_with_auto_env(&server).await?;
    let mut last_available_skill = calendar;
    for (turn, next_reply, expected_skill, new_account, expected_reads) in [
        ("discovered", Ok(Some(calendar)), Some(calendar), None, 1),
        (
            "refreshed",
            Ok(Some(updated_calendar)),
            Some(updated_calendar),
            None,
            1,
        ),
        (
            "same-account-failure",
            Err(()),
            Some(updated_calendar),
            None,
            0,
        ),
        ("removed", Ok(None), None, None, 0),
        (
            "restored",
            Ok(Some(updated_calendar)),
            Some(updated_calendar),
            None,
            1,
        ),
        ("auth-changed-failure", Err(()), None, Some("account-b"), 0),
        (
            "new-account-discovered",
            Ok(Some(gmail)),
            Some(gmail),
            None,
            1,
        ),
        ("connection-replaced-failure", Err(()), Some(gmail), None, 0),
        (
            "replacement-connection-discovered",
            Ok(Some(gmail)),
            Some(gmail),
            None,
            1,
        ),
    ] {
        let previous_lists = provider.lists.load(Ordering::SeqCst);
        let previous_reads = provider.reads.load(Ordering::SeqCst);
        *provider.reply.lock().expect("cloud reply lock") = next_reply;
        let previous_connection_scope = if turn == "connection-replaced-failure" {
            let client = resources
                .0
                .lock()
                .expect("session resources lock")
                .clone()
                .expect("thread MCP resource client");
            let auth = client.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME);
            let connection = client.server_cache_key(CODEX_APPS_MCP_SERVER_NAME);
            // Turn-start preparation must consume the pending reconnect before discovery.
            test.codex.submit(Op::RefreshMcpServers).await?;
            Some((client, auth, connection))
        } else {
            None
        };
        if let Some(account) = new_account {
            let client = resources
                .0
                .lock()
                .expect("session resources lock")
                .clone()
                .expect("thread MCP resource client");
            let previous_auth = client.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME);
            codex_login::auth::login_with_chatgpt_auth_tokens(
                test.codex_home_path(),
                "header.e30.changed",
                account,
                /*chatgpt_plan_type*/ None,
            )?;
            test.thread_manager.auth_manager().reload().await;
            // Wait for the real auth transition, without manually refreshing MCP.
            tokio::time::timeout(Duration::from_secs(10), async {
                while client.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME) == previous_auth
                {
                    tokio::task::yield_now().await;
                }
            })
            .await?;
            assert_eq!(provider.lists.load(Ordering::SeqCst), previous_lists);
        }
        // Empty catalogs must also reject reads of the previously cached skill.
        let (plugin, _) = expected_skill.unwrap_or(last_available_skill);
        let package = format!("skill://{plugin}/demo");
        let list_id = format!("list-{turn}");
        let read_id = format!("read-{turn}");
        let response = responses::mount_sse_sequence(
            &server,
            vec![
                sse(vec![
                    ev_response_created("tools"),
                    responses::ev_function_call_with_namespace(
                        &list_id,
                        "skills",
                        "list",
                        &json!({"authority":{"kind":"cloud"}}).to_string(),
                    ),
                    responses::ev_function_call_with_namespace(
                        &read_id,
                        "skills",
                        "read",
                        &json!({"package": package}).to_string(),
                    ),
                    ev_completed("tools"),
                ]),
                sse(vec![ev_response_created("done"), ev_completed("done")]),
            ],
        )
        .await;
        test.submit_turn("Inspect the cloud skill and read its instructions.")
            .await?;
        let client = resources
            .0
            .lock()
            .expect("session resources lock")
            .clone()
            .expect("thread MCP resource client");
        assert!(
            client.server_cache_key(CODEX_APPS_MCP_SERVER_NAME)
                == *provider.resource_key.lock().expect("resource key lock"),
            "turn-start reprojection must not invalidate resource caches: {turn}"
        );
        if let Some((client, scope, connection)) = previous_connection_scope {
            assert!(client.server_cache_key(CODEX_APPS_MCP_SERVER_NAME) != connection);
            assert!(
                client.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME) == scope,
                "same-auth reconnect must preserve the cloud cache scope"
            );
        }
        let requests = response.requests();
        assert_eq!(requests.len(), 2, "turn {turn}");
        assert_eq!(
            provider.lists.load(Ordering::SeqCst),
            previous_lists + 1,
            "turn {turn}"
        );
        assert_eq!(
            provider.reads.load(Ordering::SeqCst),
            previous_reads + expected_reads,
            "turn {turn}"
        );
        let listed: Value = serde_json::from_str(
            &requests[1]
                .function_call_output_text(&list_id)
                .expect("skills.list result"),
        )?;
        let read = requests[1]
            .function_call_output_text(&read_id)
            .expect("skills.read result");
        if let Some((plugin, revision)) = expected_skill {
            last_available_skill = (plugin, revision);
            let description = format!("Cloud {plugin} revision {revision}");
            assert_eq!(
                listed["skills"].as_array().expect("listed skills").len(),
                1,
                "turn {turn}"
            );
            assert_eq!(listed["skills"][0]["package"], package, "turn {turn}");
            assert_eq!(
                listed["skills"][0]["description"], description,
                "turn {turn}"
            );
            assert!(
                requests[0]
                    .message_input_texts("developer")
                    .join("\n")
                    .contains(&description),
                "turn {turn}"
            );
            let read: Value = serde_json::from_str(&read)?;
            assert_eq!(
                read["contents"],
                format!("{plugin} instructions {revision}"),
                "turn {turn}"
            );
        } else {
            assert_eq!(listed["skills"], json!([]), "turn {turn}");
            assert!(
                read.contains("skill package is not available"),
                "turn {turn}: {read}"
            );
        }
    }
    Ok(())
}
