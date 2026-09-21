//! Core cloud-skill behavior exercised without the concrete MCP-backed provider.

use super::*;
use pretty_assertions::assert_eq;
use tokio::sync::Mutex;

struct FakeCloudSkillProvider {
    catalog: SkillCatalog,
    resources: std::collections::HashMap<String, String>,
    reads: Mutex<Vec<String>>,
}

impl SkillProvider for FakeCloudSkillProvider {
    fn list(&self, _query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(async { Ok(self.catalog.clone()) })
    }

    fn read<'a>(
        &'a self,
        request: SkillReadRequest<'a>,
    ) -> SkillProviderFuture<'a, SkillReadResult> {
        Box::pin(async move {
            assert_eq!(
                request.authority,
                SkillAuthority::new(SkillSourceKind::Cloud, CODEX_APPS_MCP_SERVER_NAME)
            );
            assert!(
                self.catalog
                    .entries
                    .iter()
                    .any(|entry| entry.id == request.package)
            );
            self.reads
                .lock()
                .await
                .push(request.resource.as_str().to_string());
            let contents = self
                .resources
                .get(request.resource.as_str())
                .cloned()
                .ok_or_else(|| SkillProviderError::new("unknown fake skill resource"))?;
            Ok(SkillReadResult {
                resource: request.resource,
                contents,
            })
        })
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(async { Ok(SkillSearchResult::default()) })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_only_cloud_skill_is_hidden_but_can_be_invoked() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const SKILL_PACKAGE: &str = "skill://demo/explicit-only";
    const MAIN_RESOURCE: &str = "skill://demo/explicit-only/SKILL.md";
    const REFERENCED_RESOURCE: &str = "skill://demo/explicit-only/references/guide.md";
    const READ_CALL_ID: &str = "read-explicit-only-resource";
    const LIST_CALL_ID: &str = "list-model-visible-skills";
    const CODE_MODE_LIST_CALL_ID: &str = "list-skills-through-code-mode";
    const CONTINUATION_CALL_ID: &str = "read-explicit-only-resource-continuation";
    const MAIN_READ_CALL_ID: &str = "read-explicit-only-main";
    const REPEATED_MAIN_READ_CALL_ID: &str = "read-explicit-only-main-again";
    const INVALID_CURSOR_CALL_ID: &str = "read-explicit-only-invalid-cursor";
    const MISSING_PACKAGE_CALL_ID: &str = "read-missing-package";
    const CODE_MODE_CALL_ID: &str = "code-mode-skill-read";

    // Fill the shared 300-byte page through the emoji; ignoring escaping would fit everything.
    let read_prefix = "a".repeat(184);
    let referenced_contents = format!("{read_prefix}😀\"\\\nabcdefghijklm");
    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    READ_CALL_ID,
                    "skills",
                    "read",
                    &json!({
                        "package": SKILL_PACKAGE,
                        "authority": {
                            "kind": "cloud",
                        },
                        "resource": REFERENCED_RESOURCE,
                    })
                    .to_string(),
                ),
                responses::ev_function_call_with_namespace(
                    LIST_CALL_ID,
                    "skills",
                    "list",
                    &json!({ "authority": { "kind": "cloud" } }).to_string(),
                ),
                responses::ev_custom_tool_call(
                    CODE_MODE_LIST_CALL_ID,
                    "exec",
                    r#"const result = await tools.skills__list({ authority: { kind: "cloud" } });
text({ names: result.skills.map(skill => skill.name), warnings: result.warnings, next_cursor: result.next_cursor });"#,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;

    let mut extensions = ExtensionRegistryBuilder::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_cloud_provider(Arc::new(FakeCloudSkillProvider {
            catalog: SkillCatalog {
                entries: [
                    "alpha-visible",
                    "bravo-visible-skill",
                    "explicit-only",
                    "visible",
                ]
                .into_iter()
                .map(|name| {
                    let package = format!("skill://demo/{name}");
                    let entry = SkillCatalogEntry::new(
                        SkillPackageId(package.clone()),
                        SkillAuthority::new(SkillSourceKind::Cloud, CODEX_APPS_MCP_SERVER_NAME),
                        format!("demo:{name}"),
                        "",
                        SkillResourceId::new(format!("{package}/SKILL.md")),
                    )
                    .with_display_path(&package)
                    .with_alias_root("skill://demo");
                    if name == "explicit-only" {
                        entry.hidden_from_prompt()
                    } else {
                        entry
                    }
                })
                .collect(),
                warnings: Vec::new(),
            },
            reads: Mutex::default(),
            resources: std::collections::HashMap::from([
                (
                    MAIN_RESOURCE.to_string(),
                    format!("# Explicit-only instructions\nRead {REFERENCED_RESOURCE}."),
                ),
                (REFERENCED_RESOURCE.to_string(), referenced_contents.clone()),
            ]),
        })),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: true,
            shadow_selection_enabled: false,
        },
    );
    let chatgpt_base_url = server.uri();
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| config.chatgpt_base_url = chatgpt_base_url)
        // Local executors disable cloud skill discovery.
        .with_exec_server_url("none")
        .with_extensions(Arc::new(extensions.build()))
        .with_model_info_override("gpt-5.5", |model_info| {
            model_info.truncation_policy = TruncationPolicyConfig::bytes(/*limit*/ 250);
        })
        .with_config(|config| {
            config.include_skill_instructions = true;
            config.cloud_skill_enabled = true;
            config
                .features
                .enable(Feature::CodeMode)
                .expect("code mode should be configurable in tests");
            config
                .features
                .enable(Feature::CodeModeHost)
                .expect("code mode host should be configurable in tests");
        })
        .with_code_mode_host_program(codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?);
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("Use $demo:explicit-only.").await?;

    let requests = response.requests();
    assert_eq!(requests.len(), 2);
    let request = &requests[0];
    let developer_messages = request.message_input_texts("developer");
    for name in ["visible", "alpha-visible", "bravo-visible-skill"] {
        let catalog_entry = format!("- demo:{name}:");
        assert!(
            developer_messages
                .iter()
                .any(|message| message.contains(&catalog_entry)),
            "model-visible skills should include `{name}`: {developer_messages:?}"
        );
    }
    assert!(
        developer_messages
            .iter()
            .all(|message| !message.contains("- demo:explicit-only:")),
        "model-visible skills should omit the explicit-only skill: {developer_messages:?}"
    );
    let user_messages = request.message_input_texts("user");
    let skill_instructions = user_messages
        .iter()
        .find(|message| {
            message.contains("<name>demo:explicit-only</name>")
                && message.contains("# Explicit-only instructions")
                && message.contains(REFERENCED_RESOURCE)
        })
        .expect("explicit invocation should inject the hidden skill instructions and reference");
    let resource_access = skill_instructions
        .split_once("<resource_access>")
        .and_then(|(_, remainder)| remainder.split_once("</resource_access>"))
        .map(|(metadata, _)| metadata)
        .expect("hidden cloud skills should include resource-access metadata");
    assert_eq!(
        serde_json::from_str::<Value>(resource_access)?,
        json!({
            "authority": { "kind": "cloud" },
            "package": SKILL_PACKAGE,
            "main_resource": MAIN_RESOURCE,
        })
    );
    let first_output = requests[1]
        .function_call_output_text(READ_CALL_ID)
        .expect("skills.read should return the referenced resource");
    assert!(first_output.len() <= 300);
    let first_page = serde_json::from_str::<Value>(&first_output)?;
    let cursor = first_page["next_cursor"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("skills.read should return a continuation cursor"))?
        .to_string();
    assert_eq!(
        first_page,
        json!({
            "resource": REFERENCED_RESOURCE,
            "contents": format!("{read_prefix}😀"),
            "next_cursor": cursor,
        })
    );
    let mut list_output = requests[1]
        .function_call_output_text(LIST_CALL_ID)
        .expect("skills.list should return the model-visible catalog");
    let code_mode_output = requests[1].custom_tool_call_output(CODE_MODE_LIST_CALL_ID);
    let code_mode_text = code_mode_output["output"]
        .as_array()
        .and_then(|items| items.last())
        .and_then(|item| item["text"].as_str())
        .ok_or_else(|| {
            anyhow::anyhow!("Code Mode should return its skills.list result: {code_mode_output}")
        })?;
    assert_eq!(
        serde_json::from_str::<Value>(code_mode_text)?,
        json!({
            "names": ["demo:alpha-visible", "demo:bravo-visible-skill", "demo:visible"],
            "warnings": [],
            "next_cursor": null,
        })
    );
    let events = wait_for_analytics_events(&server, "skill_invocation", /*expected_count*/ 1).await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["skill_name"], "demo:explicit-only");
    assert_eq!(events[0]["event_params"]["invoke_type"], "explicit");

    let response = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-3"),
                responses::ev_function_call_with_namespace(
                    CONTINUATION_CALL_ID,
                    "skills",
                    "read",
                    &json!({
                        "package": SKILL_PACKAGE,
                        "resource": REFERENCED_RESOURCE,
                        "cursor": cursor,
                    })
                    .to_string(),
                ),
                responses::ev_function_call_with_namespace(
                    MAIN_READ_CALL_ID,
                    "skills",
                    "read",
                    &json!({ "package": SKILL_PACKAGE }).to_string(),
                ),
                responses::ev_function_call_with_namespace(
                    REPEATED_MAIN_READ_CALL_ID,
                    "skills",
                    "read",
                    &json!({ "package": SKILL_PACKAGE, "resource": MAIN_RESOURCE }).to_string(),
                ),
                responses::ev_function_call_with_namespace(
                    INVALID_CURSOR_CALL_ID,
                    "skills",
                    "read",
                    &json!({ "package": SKILL_PACKAGE, "cursor": "invalid" }).to_string(),
                ),
                responses::ev_function_call_with_namespace(
                    MISSING_PACKAGE_CALL_ID,
                    "skills",
                    "read",
                    &json!({ "package": "skill://demo/missing" }).to_string(),
                ),
                ev_completed("resp-3"),
            ]),
            sse(vec![ev_response_created("resp-4"), ev_completed("resp-4")]),
        ],
    )
    .await;

    test.submit_turn("Continue without explicitly selecting a skill.")
        .await?;
    let requests = response.requests();
    assert_eq!(requests.len(), 2);
    let continuation_output = requests[1]
        .function_call_output_text(CONTINUATION_CALL_ID)
        .expect("skills.read should return the next referenced-resource page");
    assert!(continuation_output.len() <= 300);
    let continuation_page = serde_json::from_str::<Value>(&continuation_output)?;
    assert_eq!(
        continuation_page,
        json!({
            "resource": REFERENCED_RESOURCE,
            "contents": "\"\\\nabcdefghijklm",
            "next_cursor": null,
        })
    );
    assert_eq!(
        format!(
            "{}{}",
            first_page["contents"].as_str().unwrap_or_default(),
            continuation_page["contents"].as_str().unwrap_or_default()
        ),
        referenced_contents
    );
    for call_id in [MAIN_READ_CALL_ID, REPEATED_MAIN_READ_CALL_ID] {
        let output = requests[1]
            .function_call_output_text(call_id)
            .expect("skills.read should return the main resource");
        assert_eq!(
            serde_json::from_str::<Value>(&output)?["resource"],
            MAIN_RESOURCE
        );
    }
    for call_id in [INVALID_CURSOR_CALL_ID, MISSING_PACKAGE_CALL_ID] {
        assert!(
            requests[1].function_call_output_text(call_id).is_some(),
            "failed skills.read should return a tool error for {call_id}"
        );
    }

    let events = wait_for_analytics_events(&server, "skill_invocation", /*expected_count*/ 2).await;
    assert_eq!(events.len(), 2, "repeated main reads must be deduplicated");
    assert_eq!(events[1]["skill_name"], "demo:explicit-only");
    assert_eq!(
        events[1]["skill_id"],
        format!("{:x}", sha1::Sha1::digest(MAIN_RESOURCE.as_bytes()))
    );
    assert_eq!(events[1]["event_params"]["invoke_type"], "implicit");

    for (name, has_more) in [
        ("alpha-visible", true),
        ("bravo-visible-skill", true),
        ("visible", false),
    ] {
        assert!(list_output.len() <= 300);
        let list_response = serde_json::from_str::<Value>(&list_output)?;
        let next_cursor = list_response["next_cursor"].as_str();
        assert_eq!(next_cursor.is_some(), has_more);
        assert_eq!(
            list_response,
            json!({
                "skills": [{
                    "authority": {"kind": "cloud"},
                    "package": format!("skill://demo/{name}"),
                    "name": format!("demo:{name}"),
                    "description": "",
                    "main_resource": format!("skill://demo/{name}/SKILL.md"),
                }],
                "warnings": [],
                "next_cursor": next_cursor,
            })
        );
        if let Some(cursor) = next_cursor {
            let call_id = format!("list-after-{name}");
            let page = responses::mount_sse_sequence(
                &server,
                vec![
                    sse(vec![
                        ev_response_created(&call_id),
                        responses::ev_function_call_with_namespace(
                            &call_id,
                            "skills",
                            "list",
                            &json!({
                                "authority": { "kind": "cloud" },
                                "cursor": cursor,
                            })
                            .to_string(),
                        ),
                        ev_completed(&call_id),
                    ]),
                    sse(vec![ev_response_created("listed"), ev_completed("listed")]),
                ],
            )
            .await;
            test.submit_turn("Continue listing skills.").await?;
            list_output = page.requests()[1]
                .function_call_output_text(&call_id)
                .expect("skills.list should return the next page");
        }
    }

    let response = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-5"),
                responses::ev_custom_tool_call(
                    CODE_MODE_CALL_ID,
                    "exec",
                    &format!(
                        "const result = await tools.skills__read({{package: {SKILL_PACKAGE:?}, resource: {REFERENCED_RESOURCE:?}}}); text(JSON.stringify({{contents: result.contents, next_cursor: result.next_cursor}}));"
                    ),
                ),
                ev_completed("resp-5"),
            ]),
            sse(vec![ev_response_created("resp-6"), ev_completed("resp-6")]),
        ],
    )
    .await;
    test.submit_turn("Read the complete skill resource using code mode.")
        .await?;
    let requests = response.requests();
    assert_eq!(requests.len(), 2);
    let output = requests[1].custom_tool_call_output(CODE_MODE_CALL_ID);
    let nested_result = output["output"]
        .as_array()
        .and_then(|items| items.last())
        .and_then(|item| item["text"].as_str())
        .ok_or_else(|| anyhow::anyhow!("code mode should return the nested skill result"))?;
    assert_eq!(
        serde_json::from_str::<Value>(nested_result)?,
        json!({"contents": referenced_contents, "next_cursor": null})
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_aliases_discovered_singleton_cloud_root() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const SKILL_ROOT: &str = "skill://plugin_connector_1p_2330815c823c8191941e5dc465bb899f";
    const SKILL_BODY: &str = "CLOUD_SKILL_REMAINS_AVAILABLE_WITHOUT_HOST_DISCOVERY";

    let server = responses::start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let mut extensions = ExtensionRegistryBuilder::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_cloud_provider(Arc::new(FakeCloudSkillProvider {
            catalog: SkillCatalog {
                entries: vec![
                    SkillCatalogEntry::new(
                        SkillPackageId(format!("{SKILL_ROOT}/search")),
                        SkillAuthority::new(SkillSourceKind::Cloud, CODEX_APPS_MCP_SERVER_NAME),
                        "demo:search",
                        "Search company knowledge.",
                        SkillResourceId::new(format!("{SKILL_ROOT}/search/SKILL.md")),
                    )
                    .with_display_path(format!("{SKILL_ROOT}/search"))
                    .with_alias_root(SKILL_ROOT),
                ],
                warnings: Vec::new(),
            },
            resources: std::collections::HashMap::from([(
                format!("{SKILL_ROOT}/search/SKILL.md"),
                SKILL_BODY.to_string(),
            )]),
            reads: Mutex::default(),
        })),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: true,
            shadow_selection_enabled: false,
        },
    );
    let chatgpt_base_url = server.uri();
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| config.chatgpt_base_url = chatgpt_base_url)
        .with_exec_server_url("none")
        .with_extensions(Arc::new(extensions.build()))
        .with_model_info_override("gpt-5.5", |model_info| {
            model_info.context_window = Some(1_000);
            model_info.max_context_window = None;
        })
        .with_config(|config| {
            config.include_skill_instructions = true;
            config.cloud_skill_enabled = true;
            config
                .features
                .enable(Feature::SkipHostSkillDiscovery)
                .expect("cloud skills must not depend on host discovery");
        });
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("Use $demo:search.").await?;

    let request = response.single_request();
    let developer_text = request.message_input_texts("developer").join("\n");
    assert!(
        developer_text.contains(&format!("- `c0` = `{SKILL_ROOT}`")),
        "model request should include the discovered cloud root: {developer_text}"
    );
    assert!(
        developer_text.lines().any(|line| {
            line.starts_with("- demo:search:") && line.ends_with("(cloud package: c0/search)")
        }),
        "model request should include the aliased cloud skill: {developer_text}"
    );
    assert!(
        developer_text.contains("- Root aliases: Pass short package locators directly"),
        "model request should explain how to read aliased packages: {developer_text}"
    );
    let user_text = request.message_input_texts("user").join("\n");
    assert!(
        user_text.contains("<skill>\n<name>demo:search</name>") && user_text.contains(SKILL_BODY),
        "cloud instruction reads must remain available without host discovery: {user_text}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cloud_skill_can_read_referenced_resource_without_an_executor() -> Result<()> {
    skip_if_no_network!(Ok(()));
    const SKILL_NAME: &str = "demo-plugin:deploy";

    const SKILL_DESCRIPTION: &str = "Deploy through the &lt;hosted&gt; orchestrator.";

    const SKILL_RESOURCE_URI: &str = "skill://plugin_demo/deploy";

    const SKILL_MAIN_PROMPT_URI: &str = "skill://plugin_demo/deploy/SKILL.md";

    const SKILL_REFERENCE_URI: &str = "skill://plugin_demo/deploy/references/deploy.md";

    const SKILL_MARKER: &str = "ORCHESTRATOR_SKILL_BODY_MARKER";

    const SKILL_CONTENTS: &str = concat!(
        "---\n",
        "name: deploy\n",
        "description: Deploy through the orchestrator.\n",
        "---\n\n",
        "# Deploy\n\n",
        "ORCHESTRATOR_SKILL_BODY_MARKER\n\n",
        "Read the [deployment reference](skill://plugin_demo/deploy/references/deploy.md).\n",
    );

    const SKILL_REFERENCE_CONTENTS: &str =
        "# Deploy reference\n\nUse the orchestrator deployment API.\n";

    const SKILLS_LIST_CALL_ID: &str = "skills-list";

    const SKILLS_READ_MAIN_CALL_ID: &str = "skills-read-main";

    const SKILLS_READ_CALL_ID: &str = "skills-read";

    const SKILLS_READ_AGAIN_CALL_ID: &str = "skills-read-again";

    let responses_server = responses::start_mock_server().await;
    let provider = Arc::new(FakeCloudSkillProvider {
        catalog: SkillCatalog {
            entries: vec![
                SkillCatalogEntry::new(
                    SkillPackageId(SKILL_RESOURCE_URI.to_string()),
                    SkillAuthority::new(SkillSourceKind::Cloud, CODEX_APPS_MCP_SERVER_NAME),
                    SKILL_NAME,
                    SKILL_DESCRIPTION,
                    SkillResourceId::new(SKILL_MAIN_PROMPT_URI),
                )
                .with_display_path(SKILL_RESOURCE_URI)
                .with_alias_root("skill://plugin_demo"),
            ],
            warnings: vec!["Cloud discovery returned a partial catalog.".to_string()],
        },
        resources: std::collections::HashMap::from([
            (
                SKILL_MAIN_PROMPT_URI.to_string(),
                SKILL_CONTENTS.to_string(),
            ),
            (
                SKILL_REFERENCE_URI.to_string(),
                SKILL_REFERENCE_CONTENTS.to_string(),
            ),
        ]),
        reads: Mutex::default(),
    });
    let mut extensions = ExtensionRegistryBuilder::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_cloud_provider(provider.clone()),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: true,
            shadow_selection_enabled: false,
        },
    );
    let chatgpt_base_url = responses_server.uri();
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| config.chatgpt_base_url = chatgpt_base_url)
        .with_exec_server_url("none")
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config.include_skill_instructions = true;
            config.cloud_skill_enabled = true;
        });
    let test = builder.build_with_auto_env(&responses_server).await?;
    let response_mock = responses::mount_sse_sequence(
        &responses_server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-skills-read-main"),
                responses::ev_function_call_with_namespace(
                    SKILLS_READ_MAIN_CALL_ID,
                    "skills",
                    "read",
                    &json!({
                        "package": SKILL_RESOURCE_URI,
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-skills-read-main"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-skills-list"),
                responses::ev_function_call_with_namespace(
                    SKILLS_LIST_CALL_ID,
                    "skills",
                    "list",
                    &json!({
                        "authority": {
                            "kind": "cloud",
                        },
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-skills-list"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-skills-read"),
                responses::ev_function_call_with_namespace(
                    SKILLS_READ_CALL_ID,
                    "skills",
                    "read",
                    &json!({
                        "package": SKILL_RESOURCE_URI,
                        "authority": {
                            "kind": "cloud",
                        },
                        "resource": SKILL_REFERENCE_URI,
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-skills-read"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-skills-read-again"),
                responses::ev_function_call_with_namespace(
                    SKILLS_READ_AGAIN_CALL_ID,
                    "skills",
                    "read",
                    &json!({
                        "package": SKILL_RESOURCE_URI,
                        "resource": SKILL_REFERENCE_URI,
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-skills-read-again"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-orchestrator-skill"),
                responses::ev_assistant_message("msg-orchestrator-skill", "Done"),
                responses::ev_completed("resp-orchestrator-skill"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-orchestrator-skill-after-refresh"),
                responses::ev_assistant_message("msg-orchestrator-skill-after-refresh", "Done"),
                responses::ev_completed("resp-orchestrator-skill-after-refresh"),
            ]),
        ],
    )
    .await;
    test.submit_turn("Use the deployment capability.").await?;
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 5);
    let first_request = &requests[0];
    assert!(first_request.tool_by_name("skills", "list").is_some());
    let read_tool = first_request
        .tool_by_name("skills", "read")
        .ok_or_else(|| anyhow::anyhow!("skills.read should be available"))?;
    assert_eq!(read_tool["parameters"]["required"], json!(["package"]));
    assert!(
        read_tool["parameters"]["properties"]
            .get("authority")
            .is_none()
    );
    assert!(first_request.tool_by_name("skills", "search").is_none());

    let developer_messages = first_request.message_input_texts("developer");
    let catalog_line = format!("- {SKILL_NAME}: {SKILL_DESCRIPTION} (cloud package: c0/deploy)");
    assert!(
        developer_messages
            .iter()
            .any(|text| text.contains("- `c0` = `skill://plugin_demo`"))
    );
    assert_eq!(
        1,
        developer_messages
            .iter()
            .filter(|text| text.contains(&catalog_line))
            .count()
    );
    assert!(
        developer_messages
            .iter()
            .all(|text| !text.contains("ignored-plugin:ignored"))
    );
    assert!(
        developer_messages
            .iter()
            .any(|text| text.contains("do not treat `skill://` identifiers as filesystem paths"))
    );
    assert!(
        first_request
            .message_input_texts("user")
            .into_iter()
            .all(|text| !text.starts_with("<skill>"))
    );

    let main_read_output = requests[1]
        .function_call_output_text(SKILLS_READ_MAIN_CALL_ID)
        .ok_or_else(|| anyhow::anyhow!("skills.read output should be sent to the model"))?;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&main_read_output)?,
        json!({
            "resource": SKILL_MAIN_PROMPT_URI,
            "contents": SKILL_CONTENTS,
            "next_cursor": null,
        })
    );

    let list_output = requests[2]
        .function_call_output_text(SKILLS_LIST_CALL_ID)
        .ok_or_else(|| anyhow::anyhow!("skills.list output should be sent to the model"))?;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&list_output)?,
        json!({
            "skills": [{
                "authority": {
                    "kind": "cloud",
                },
                "package": SKILL_RESOURCE_URI,
                "name": SKILL_NAME,
                "description": SKILL_DESCRIPTION,
                "main_resource": SKILL_MAIN_PROMPT_URI,
            }],
            "warnings": ["Cloud discovery returned a partial catalog."],
            "next_cursor": null,
        })
    );

    let read_output = requests[3]
        .function_call_output_text(SKILLS_READ_CALL_ID)
        .ok_or_else(|| anyhow::anyhow!("skills.read output should be sent to the model"))?;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&read_output)?,
        json!({
            "resource": SKILL_REFERENCE_URI,
            "contents": SKILL_REFERENCE_CONTENTS,
            "next_cursor": null,
        })
    );
    let repeated_read_output = requests[4]
        .function_call_output_text(SKILLS_READ_AGAIN_CALL_ID)
        .ok_or_else(|| {
            anyhow::anyhow!("repeated skills.read output should be sent to the model")
        })?;
    assert_eq!(read_output, repeated_read_output);
    assert_eq!(
        *provider.reads.lock().await,
        vec![SKILL_MAIN_PROMPT_URI, SKILL_REFERENCE_URI]
    );

    test.codex.submit(Op::RefreshMcpServers).await?;
    test.submit_turn(&format!("Use ${SKILL_NAME} after refreshing MCP"))
        .await?;
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 6);
    let skill_fragments = requests[5]
        .message_input_texts("user")
        .into_iter()
        .filter(|text| text.starts_with("<skill>"))
        .collect::<Vec<_>>();
    assert_eq!(1, skill_fragments.len());
    assert!(skill_fragments[0].contains(&format!("<name>{SKILL_NAME}</name>")));
    assert!(skill_fragments[0].contains(SKILL_MARKER));
    assert!(skill_fragments[0].contains(SKILL_REFERENCE_URI));
    assert_eq!(
        *provider.reads.lock().await,
        vec![
            SKILL_MAIN_PROMPT_URI,
            SKILL_REFERENCE_URI,
            SKILL_MAIN_PROMPT_URI
        ]
    );
    Ok(())
}
