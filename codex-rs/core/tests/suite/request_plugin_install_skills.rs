//! Connector-free installs invalidate a warmed cloud skill catalog before the next inference.

use super::*;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_skills_extension::SkillProvider;
use codex_skills_extension::SkillProviders;
use codex_skills_extension::SkillsExtensionConfig;
use codex_skills_extension::catalog::SkillAuthority;
use codex_skills_extension::catalog::SkillCatalog;
use codex_skills_extension::catalog::SkillCatalogEntry;
use codex_skills_extension::catalog::SkillPackageId;
use codex_skills_extension::catalog::SkillProviderError;
use codex_skills_extension::catalog::SkillReadResult;
use codex_skills_extension::catalog::SkillResourceId;
use codex_skills_extension::catalog::SkillSearchResult;
use codex_skills_extension::catalog::SkillSourceKind;
use codex_skills_extension::install_with_providers;
use codex_skills_extension::provider::SkillListQuery;
use codex_skills_extension::provider::SkillProviderFuture;
use codex_skills_extension::provider::SkillReadRequest;
use codex_skills_extension::provider::SkillSearchRequest;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicUsize;

const SKILL_DESCRIPTION: &str = "Plan a week using the newly installed Calendar skill.";

#[derive(Default)]
struct InstallableCloudSkillProvider {
    installed: AtomicBool,
    lists: AtomicUsize,
}

impl SkillProvider for InstallableCloudSkillProvider {
    fn list<'a>(&'a self, _query: SkillListQuery<'a>) -> SkillProviderFuture<'a, SkillCatalog> {
        Box::pin(async move {
            self.lists.fetch_add(1, Ordering::SeqCst);
            let entries = if self.installed.load(Ordering::SeqCst) {
                vec![
                    SkillCatalogEntry::new(
                        SkillPackageId("skill://calendar/plan-week".to_string()),
                        SkillAuthority::new(SkillSourceKind::Cloud, CODEX_APPS_MCP_SERVER_NAME),
                        "calendar:plan-week",
                        SKILL_DESCRIPTION,
                        SkillResourceId::new("skill://calendar/plan-week/SKILL.md"),
                    )
                    .with_display_path("skill://calendar/plan-week")
                    .with_alias_root("skill://calendar"),
                ]
            } else {
                Vec::new()
            };
            Ok(SkillCatalog {
                is_partial: false,
                entries,
                warnings: Vec::new(),
            })
        })
    }

    fn read<'a>(
        &'a self,
        _request: SkillReadRequest<'a>,
    ) -> SkillProviderFuture<'a, SkillReadResult> {
        Box::pin(async { Err(SkillProviderError::new("this test only discovers skills")) })
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(async { Ok(SkillSearchResult::default()) })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_skills_only_install_refreshes_warmed_cloud_catalog() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount_without_tools(&server).await?;
    mount_recommendations(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({
            "enabled": true,
            "plugins": [{
                "id": REMOTE_CALENDAR_PLUGIN_ID,
                "name": "calendar",
                "display_name": "Calendar"
            }]
        })),
    )
    .await;
    Mock::given(method("GET"))
        .and(path(format!("/ps/plugins/{REMOTE_CALENDAR_PLUGIN_ID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": REMOTE_CALENDAR_PLUGIN_ID,
            "name": "calendar",
            "scope": "GLOBAL",
            "status": "ENABLED",
            "installation_policy": "AVAILABLE",
            "authentication_policy": "ON_USE",
            "release": {
                "display_name": "Calendar",
                "description": "Calendar planning skills without connector tools.",
                "app_ids": [],
                "interface": {}
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let initially_installed = mount_empty_remote_installed_plugins(&server).await;
    let provider = Arc::new(InstallableCloudSkillProvider::default());
    let mut extensions = ExtensionRegistryBuilder::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_cloud_provider(provider.clone()),
        |_config: &Config| SkillsExtensionConfig {
            include_instructions: true,
            catalog_deltas_enabled: false,
            omit_descriptions: false,
            max_context_tokens: None,
            bundled_skills_enabled: false,
            cloud_skill_enabled: true,
            shadow_selection_enabled: false,
        },
    );
    let mut builder = test_codex()
        .with_exec_server_url("none")
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("test config should allow disabled permissions");
            configure_apps_without_search_tool(config, &apps_server.chatgpt_base_url);
            config.include_skill_instructions = true;
            config.cloud_skill_enabled = true;
        });
    let test = builder.build_with_auto_env(&server).await?;

    let warmup = mount_sse_once(
        &server,
        sse(vec![ev_response_created("warmup"), ev_completed("warmup")]),
    )
    .await;
    test.submit_turn("What skills are available?").await?;
    assert_eq!(provider.lists.load(Ordering::SeqCst), 1);
    assert!(
        !warmup
            .single_request()
            .message_input_texts("developer")
            .join("\n")
            .contains(SKILL_DESCRIPTION)
    );

    let install_call_id = "install-calendar-skills";
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("install"),
                ev_function_call(
                    install_call_id,
                    REQUEST_PLUGIN_INSTALL_TOOL_NAME,
                    &json!({
                        "plugin_id": REMOTE_CALENDAR_PLUGIN_CONFIG_ID,
                        "suggest_reason": "Plan my week"
                    })
                    .to_string(),
                ),
                ev_completed("install"),
            ]),
            sse(vec![
                ev_response_created("installed"),
                ev_completed("installed"),
            ]),
        ],
    )
    .await;
    let elicitation = start_install_turn(&test, "Install Calendar planning skills").await?;
    // A second turn must reuse the warmed empty catalog until installation completes.
    assert_eq!(provider.lists.load(Ordering::SeqCst), 1);
    mount_remote_calendar_installed_plugins(&server).await;
    drop(initially_installed);
    provider.installed.store(true, Ordering::SeqCst);
    resolve_install_elicitation(&test, elicitation, ElicitationAction::Accept).await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        serde_json::from_str::<Value>(
            &requests[1]
                .function_call_output_text(install_call_id)
                .expect("install tool output")
        )?,
        json!({
            "completed": true,
            "user_confirmed": true,
            "tool_type": "plugin",
            "action_type": "install",
            "tool_id": REMOTE_CALENDAR_PLUGIN_CONFIG_ID,
            "tool_name": "Calendar",
            "suggest_reason": "Plan my week"
        })
    );
    assert!(
        !requests[0]
            .message_input_texts("developer")
            .join("\n")
            .contains(SKILL_DESCRIPTION)
    );
    assert!(
        requests[1]
            .message_input_texts("developer")
            .join("\n")
            .contains(SKILL_DESCRIPTION),
        "the inference immediately after a connector-free install must expose its cloud skill"
    );
    assert_eq!(provider.lists.load(Ordering::SeqCst), 2);
    Ok(())
}
