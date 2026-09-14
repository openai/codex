use super::*;
use anyhow::Result;
use axum::http::HeaderValue;
use codex_app_server_protocol::AppConfig;
use codex_app_server_protocol::AppToolApproval;
use codex_app_server_protocol::AppsConfig;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ConfigLayerSource as ApiConfigLayerSource;
use codex_config::CloudConfigBundleLoader;
use codex_config::LoaderOverrides;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[test]
fn provider_batch_remapping_preserves_ordered_edits() {
    let initial = serde_json::json!({
        "features": { "network_proxy": { "credentials": {
            "a": { "env": ["A_AUTH"], "patterns": ["alpha"], "url_prefixes": ["https://a.example"] },
            "b": { "env": ["B_AUTH"], "patterns": ["bravo"], "url_prefixes": ["https://b.example"], "auth": ["bearer"] },
        } } },
    });
    let retained = serde_json::json!({
        "env": ["A_AUTH"], "patterns": ["bravo"], "url_prefixes": ["https://b.example"],
    });
    let mut changed_auth = retained.clone();
    changed_auth["auth"] = serde_json::json!(["token"]);
    for (path, value, strategy, expected_b) in [
        (
            "b.auth",
            serde_json::json!(["token"]),
            MergeStrategy::Upsert,
            changed_auth,
        ),
        ("b.auth", JsonValue::Null, MergeStrategy::Replace, retained),
        (
            "b",
            JsonValue::Null,
            MergeStrategy::Replace,
            serde_json::json!({ "env": ["A_AUTH"] }),
        ),
        (
            "b",
            serde_json::json!({ "patterns": ["replacement"], "url_prefixes": ["https://new.example"] }),
            MergeStrategy::Replace,
            serde_json::json!({ "env": ["A_AUTH"], "patterns": ["replacement"], "url_prefixes": ["https://new.example"] }),
        ),
        (
            "",
            JsonValue::Null,
            MergeStrategy::Replace,
            serde_json::json!({ "env": ["A_AUTH"] }),
        ),
    ] {
        let mut config: TomlValue = serde_json::from_value(initial.clone()).unwrap();
        let tmp = tempdir().unwrap();
        let file = tmp.path().join(CONFIG_TOML_FILE);
        std::fs::write(&file, toml::to_string(&config).unwrap()).unwrap();
        let mut providers = CredentialProviderEdits::new(&config);
        let mut persistence = Vec::new();
        let path = if path.is_empty() {
            "features.network_proxy.credentials".to_string()
        } else {
            format!("features.network_proxy.credentials.{path}")
        };
        for (key_path, value, strategy) in [
            (
                "features.network_proxy.credentials.a.env",
                serde_json::json!(["B_AUTH"]),
                MergeStrategy::Upsert,
            ),
            (path.as_str(), value, strategy),
            (
                "features.network_proxy.credentials.b.env",
                serde_json::json!(["A_AUTH"]),
                MergeStrategy::Upsert,
            ),
        ] {
            persistence.extend(
                providers
                    .apply(
                        &mut config,
                        &parse_key_path(key_path).unwrap(),
                        parse_value(value).unwrap().as_ref(),
                        strategy,
                    )
                    .unwrap(),
            );
        }
        persistence.extend(providers.remapping_edits(&config).unwrap());
        ConfigEditsBuilder::for_config_path(&file)
            .with_edits(persistence)
            .apply_blocking()
            .unwrap();
        assert_eq!(
            toml::from_str::<TomlValue>(&std::fs::read_to_string(&file).unwrap()).unwrap(),
            config
        );
        let mut expected = serde_json::json!({ "b": expected_b });
        if path != "features.network_proxy.credentials" {
            expected["a"] = initial["features"]["network_proxy"]["credentials"]["a"].clone();
            expected["a"]["env"] = serde_json::json!(["B_AUTH"]);
        }
        assert_eq!(
            config["features"]["network_proxy"]["credentials"],
            serde_json::from_value::<TomlValue>(expected).unwrap(),
            "{path}"
        );
    }
}

#[tokio::test]
async fn provider_edits_preserve_another_writers_sibling_update() -> Result<()> {
    let initial: TomlValue = toml::from_str(
        r#"
[features.network_proxy.credentials.a]
env = ['A_AUTH']
patterns = ['^alpha_[a-z]{8}$']
url_prefixes = ['https://a.example']
auth = ['bearer']
[features.network_proxy.credentials.b]
env = ['B_AUTH']
patterns = ['^bravo_[a-z]{8}$']
url_prefixes = ['https://b.example']
auth = ['bearer']
[features.network_proxy.credentials.c]
env = ['C_AUTH']
patterns = ['^charlie_[a-z]{8}$']
url_prefixes = ['https://c.example']
auth = ['bearer']
"#,
    )?;
    for updates in [
        vec![("a.auth", serde_json::json!(["token"]))],
        vec![("a.env", serde_json::json!(["C_AUTH"]))],
        vec![
            ("a.env", serde_json::json!(["C_AUTH"])),
            ("c.env", serde_json::json!(["A_AUTH"])),
        ],
    ] {
        let tmp = tempdir()?;
        let file = tmp.path().join(CONFIG_TOML_FILE);
        std::fs::write(&file, toml::to_string(&initial)?)?;
        let mut config = initial.clone();
        let mut providers = CredentialProviderEdits::new(&config);
        let mut persistence = Vec::new();
        for (path, value) in updates {
            persistence.extend(providers.apply(
                &mut config,
                &parse_key_path(&format!("features.network_proxy.credentials.{path}")).unwrap(),
                parse_value(value).unwrap().as_ref(),
                MergeStrategy::Upsert,
            )?);
        }
        persistence.extend(providers.remapping_edits(&config)?);

        // W1 has prepared its edits; W2 updates B before W1's persistence reread.
        ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf())
            .write_value(ConfigValueWriteParams {
                file_path: None,
                key_path: "features.network_proxy.credentials.b.auth".to_string(),
                value: serde_json::json!(["token"]),
                merge_strategy: MergeStrategy::Upsert,
                expected_version: None,
            })
            .await?;
        ConfigEditsBuilder::for_config_path(&file)
            .with_edits(persistence)
            .apply()
            .await?;

        config["features"]["network_proxy"]["credentials"]["b"]["auth"] =
            TomlValue::Array(vec![TomlValue::String("token".to_string())]);
        assert_eq!(
            toml::from_str::<TomlValue>(&std::fs::read_to_string(&file)?)?,
            config
        );
    }
    Ok(())
}

#[test]
fn provider_remapping_respects_lower_layer_eviction() -> Result<()> {
    let tmp = tempdir()?;
    let file = AbsolutePathBuf::try_from(tmp.path().join(CONFIG_TOML_FILE))?;
    let complete = |id: &str| -> TomlValue {
        toml::from_str(&format!(
            "[features.network_proxy.credentials.{id}]\nenv = ['VENDOR_PASSWORD']\npatterns = ['^pin_[a-z]{{8}}$']\nurl_prefixes = ['https://old.example']\n"
        )).unwrap()
    };
    // The middle layer must not let an evicted definition's fields reappear later.
    for (lower_id, middle, user, valid, strategy, move_source) in [
        ("a_working", None, "", false, MergeStrategy::Upsert, false),
        ("a_working", None, "", false, MergeStrategy::Replace, false),
        ("z_new", None, "", true, MergeStrategy::Upsert, false),
        ("z_new", None, "", true, MergeStrategy::Replace, false),
        (
            "z_new",
            Some("a_working"),
            "",
            false,
            MergeStrategy::Upsert,
            false,
        ),
        (
            "z_new",
            Some("a_working"),
            "a_working",
            false,
            MergeStrategy::Upsert,
            false,
        ),
        ("a_working", None, "", false, MergeStrategy::Upsert, true),
        (
            "a_working",
            None,
            "a_working",
            false,
            MergeStrategy::Upsert,
            true,
        ),
    ] {
        let mut user_config = if user.is_empty() {
            TomlValue::Table(Default::default())
        } else {
            complete(user)
        };
        let original_user_config = user_config.clone();
        let mut edits = CredentialProviderEdits::new(&user_config);
        if move_source {
            edits
                .apply(
                    &mut user_config,
                    &parse_key_path("features.network_proxy.credentials.a_working.env").unwrap(),
                    Some(&TomlValue::Array(vec![TomlValue::String(
                        "OTHER_PASSWORD".into(),
                    )])),
                    MergeStrategy::Upsert,
                )
                .unwrap();
        }
        edits
            .apply(
                &mut user_config,
                &parse_key_path("features.network_proxy.credentials.z_new.env").unwrap(),
                Some(&TomlValue::Array(vec![TomlValue::String(
                    "VENDOR_PASSWORD".into(),
                )])),
                strategy,
            )
            .unwrap();
        let mut layers = vec![ConfigLayerEntry::new(
            ConfigLayerSource::System { file: file.clone() },
            complete(lower_id),
        )];
        if let Some(id) = middle {
            layers.push(ConfigLayerEntry::new(
                ConfigLayerSource::EnterpriseManaged {
                    id: "test".into(),
                    name: "test".into(),
                },
                complete(id),
            ));
        }
        layers.push(ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: file.clone(),
                profile: None,
            },
            user_config.clone(),
        ));
        let layers = ConfigLayerStack::new(layers, Default::default(), Default::default())?;
        assert_eq!(
            edits
                .validate_remapping(&original_user_config, &user_config, &layers)
                .is_ok(),
            valid,
            "{lower_id} / {middle:?} / {user}",
        );
    }
    for pattern in ["[", "^pin_[a-z]{8}$"] {
        for reverse in [false, true] {
            let original = complete("a_working");
            let mut config = original.clone();
            let mut edits = CredentialProviderEdits::new(&config);
            let mut updates = [
                ("a_working.env", serde_json::json!(["OTHER_PASSWORD"])),
                ("z_new.patterns", serde_json::json!([pattern])),
            ];
            if reverse {
                updates.reverse();
            }
            for (path, value) in updates {
                edits
                    .apply(
                        &mut config,
                        &parse_key_path(&format!("features.network_proxy.credentials.{path}"))
                            .unwrap(),
                        parse_value(value).unwrap().as_ref(),
                        MergeStrategy::Upsert,
                    )
                    .unwrap();
            }
            let layers = ConfigLayerStack::new(
                vec![
                    ConfigLayerEntry::new(
                        ConfigLayerSource::System { file: file.clone() },
                        complete("z_new"),
                    ),
                    ConfigLayerEntry::new(
                        ConfigLayerSource::User {
                            file: file.clone(),
                            profile: None,
                        },
                        config.clone(),
                    ),
                ],
                Default::default(),
                Default::default(),
            )?;
            assert_eq!(
                edits
                    .validate_remapping(&original, &config, &layers)
                    .is_ok(),
                pattern != "[",
                "{pattern}, reverse={reverse}"
            );
        }
    }
    Ok(())
}

#[test]
fn provider_updates_preserve_working_definitions_and_drafts() -> Result<()> {
    let tmp = tempdir()?;
    let file = AbsolutePathBuf::try_from(tmp.path().join(CONFIG_TOML_FILE))?;
    let pattern = "^pin_[a-z]{8}$";
    for (lower_pattern, initial, path, value, valid, managed_pattern) in [
        (
            pattern,
            serde_json::json!({}),
            "a.patterns",
            serde_json::json!(["["]),
            false,
            None,
        ),
        (
            pattern,
            serde_json::json!({}),
            "a.auth",
            serde_json::json!(["token"]),
            true,
            None,
        ),
        (
            "[",
            serde_json::json!({"a": {"patterns": [pattern]}}),
            "a",
            JsonValue::Null,
            true,
            None,
        ),
        (
            pattern,
            serde_json::json!({"a": {"patterns": ["["]}}),
            "a.patterns",
            serde_json::json!(["("]),
            true,
            None,
        ),
        (
            pattern,
            serde_json::json!({"b": {"env": ["NEW_PASSWORD"]}}),
            "b.patterns",
            serde_json::json!(["["]),
            true,
            None,
        ),
        (
            pattern,
            serde_json::json!({"b": {"env": ["NEW_PASSWORD"]}}),
            "b",
            serde_json::json!({"env": ["NEW_PASSWORD"], "patterns": [pattern]}),
            true,
            None,
        ),
        (
            pattern,
            serde_json::json!({"b": {"env": ["NEW_PASSWORD"]}}),
            "",
            serde_json::json!({"b": {"env": ["NEW_PASSWORD"], "patterns": [pattern]}}),
            true,
            None,
        ),
        (
            "[",
            serde_json::json!({"a": {"env": ["VENDOR_PASSWORD"]}}),
            "a.env",
            serde_json::json!([]),
            false,
            Some(pattern),
        ),
    ] {
        let lower: TomlValue = serde_json::from_value(serde_json::json!({
            "features": {"network_proxy": {"credentials": {"a": {
                "env": ["VENDOR_PASSWORD"], "patterns": [lower_pattern],
                "url_prefixes": ["https://api.vendor.example"],
            }}}}
        }))?;
        let original: TomlValue = serde_json::from_value(serde_json::json!({
            "features": {"network_proxy": {"credentials": initial}}
        }))?;
        let mut config = original.clone();
        let mut edits = CredentialProviderEdits::new(&config);
        let key_path = if path.is_empty() {
            "features.network_proxy.credentials".to_string()
        } else {
            format!("features.network_proxy.credentials.{path}")
        };
        edits
            .apply(
                &mut config,
                &parse_key_path(&key_path).unwrap(),
                parse_value(value).unwrap().as_ref(),
                MergeStrategy::Replace,
            )
            .unwrap();
        let mut layers = vec![
            ConfigLayerEntry::new(ConfigLayerSource::System { file: file.clone() }, lower),
            ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: file.clone(),
                    profile: None,
                },
                config.clone(),
            ),
        ];
        if let Some(pattern) = managed_pattern {
            layers.push(ConfigLayerEntry::new(
                ConfigLayerSource::LegacyManagedConfigTomlFromFile { file: file.clone() },
                serde_json::from_value(serde_json::json!({"features": {"network_proxy": {"credentials": {"a": {"patterns": [pattern]}}}}}))?,
            ));
        }
        let layers = ConfigLayerStack::new(layers, Default::default(), Default::default())?;
        assert_eq!(
            edits
                .validate_remapping(&original, &config, &layers)
                .is_ok(),
            valid,
            "{path}"
        );
    }
    Ok(())
}

#[test]
fn provider_draft_remapping_uses_final_source_ownership() -> Result<()> {
    let tmp = tempdir()?;
    let file = AbsolutePathBuf::try_from(tmp.path().join(CONFIG_TOML_FILE))?;
    let config = |providers| -> TomlValue {
        serde_json::from_value(serde_json::json!({
            "features": {"network_proxy": {"credentials": providers}}
        }))
        .unwrap()
    };
    let complete = serde_json::json!({
        "env": ["VENDOR_PASSWORD"], "patterns": ["^pin_[a-z]{8}$"],
        "url_prefixes": ["https://api.vendor.example"],
    });
    let lower = config(serde_json::json!({"b": complete}));
    let inherited_draft = config(serde_json::json!({
        "a": complete, "b": {"url_prefixes": []},
    }));
    let inherited_only_draft = config(serde_json::json!({
        "b": {"env": ["VENDOR_PASSWORD"]},
    }));
    let mut multi_source = serde_json::json!({"a": complete, "c": complete});
    multi_source["a"]["env"] = serde_json::json!(["VENDOR_PASSWORD", "VENDOR_ALIAS"]);
    multi_source["c"]["env"] = serde_json::json!(["OTHER_PASSWORD"]);
    let mut actual = Vec::new();
    let mut expected = Vec::new();
    for (name, lower, original, updates, strategy, valid) in [
        (
            "automatic eviction reveals an inherited draft",
            inherited_only_draft.clone(),
            config(multi_source.clone()),
            vec![("c.env", serde_json::json!(["VENDOR_ALIAS"]))],
            MergeStrategy::Upsert,
            false,
        ),
        (
            "explicit deletion after remapping remains allowed",
            inherited_only_draft.clone(),
            config(multi_source),
            vec![
                ("c.env", serde_json::json!(["VENDOR_ALIAS"])),
                ("a", JsonValue::Null),
            ],
            MergeStrategy::Upsert,
            true,
        ),
        (
            "inherited-only draft receives the source",
            inherited_only_draft.clone(),
            config(serde_json::json!({"a": complete})),
            vec![("a.env", serde_json::json!(["OTHER_PASSWORD"]))],
            MergeStrategy::Replace,
            false,
        ),
        (
            "deletion may reveal an inherited-only draft",
            inherited_only_draft.clone(),
            config(serde_json::json!({"a": complete})),
            vec![("a", JsonValue::Null)],
            MergeStrategy::Replace,
            true,
        ),
        (
            "deleting both providers may reveal an inherited draft",
            inherited_only_draft,
            config(serde_json::json!({"a": complete, "b": {}})),
            vec![("a", JsonValue::Null), ("b", JsonValue::Null)],
            MergeStrategy::Replace,
            true,
        ),
        (
            "inherited draft receives the source",
            lower.clone(),
            inherited_draft.clone(),
            vec![("a.env", serde_json::json!(["OTHER_PASSWORD"]))],
            MergeStrategy::Replace,
            false,
        ),
        (
            "whole provider deletion remains allowed",
            lower.clone(),
            inherited_draft.clone(),
            vec![("a", JsonValue::Null)],
            MergeStrategy::Replace,
            true,
        ),
        (
            "deletion with an unchanged draft edit",
            lower.clone(),
            inherited_draft.clone(),
            vec![
                ("a", JsonValue::Null),
                ("b.url_prefixes", serde_json::json!([])),
            ],
            MergeStrategy::Replace,
            true,
        ),
        (
            "batch completes the recipient",
            lower,
            inherited_draft,
            vec![
                ("a.env", serde_json::json!(["OTHER_PASSWORD"])),
                ("b.url_prefixes", serde_json::json!(["https://new.example"])),
            ],
            MergeStrategy::Replace,
            true,
        ),
        (
            "batch restores the draft source",
            config(serde_json::json!({})),
            config(serde_json::json!({"b": {"env": ["DRAFT_PASSWORD"]}})),
            vec![
                ("b", serde_json::json!({"patterns": ["^pin_[a-z]{8}$"]})),
                ("b.env", serde_json::json!(["DRAFT_PASSWORD"])),
            ],
            MergeStrategy::Replace,
            true,
        ),
    ] {
        let mut config = original.clone();
        let mut edits = CredentialProviderEdits::new(&config);
        for (path, value) in updates {
            edits
                .apply(
                    &mut config,
                    &parse_key_path(&format!("features.network_proxy.credentials.{path}")).unwrap(),
                    parse_value(value).unwrap().as_ref(),
                    strategy.clone(),
                )
                .unwrap();
        }
        let layers = ConfigLayerStack::new(
            vec![
                ConfigLayerEntry::new(ConfigLayerSource::System { file: file.clone() }, lower),
                ConfigLayerEntry::new(
                    ConfigLayerSource::User {
                        file: file.clone(),
                        profile: None,
                    },
                    config.clone(),
                ),
            ],
            Default::default(),
            Default::default(),
        )?;
        actual.push((
            name,
            edits
                .validate_remapping(&original, &config, &layers)
                .is_ok(),
        ));
        expected.push((name, valid));
    }
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn toml_value_to_item_handles_nested_config_tables() {
    let config = r#"
[mcp_servers.docs]
command = "docs-server"

[mcp_servers.docs.http_headers]
X-Doc = "42"
"#;

    let value: TomlValue = toml::from_str(config).expect("parse config example");
    let item = toml_value_to_item(&value).expect("convert to toml_edit item");

    let root = item.as_table().expect("root table");
    assert!(!root.is_implicit(), "root table should be explicit");

    let mcp_servers = root
        .get("mcp_servers")
        .and_then(TomlItem::as_table)
        .expect("mcp_servers table");
    assert!(
        !mcp_servers.is_implicit(),
        "mcp_servers table should be explicit"
    );

    let docs = mcp_servers
        .get("docs")
        .and_then(TomlItem::as_table)
        .expect("docs table");
    assert_eq!(
        docs.get("command")
            .and_then(TomlItem::as_value)
            .and_then(toml_edit::Value::as_str),
        Some("docs-server")
    );

    let http_headers = docs
        .get("http_headers")
        .and_then(TomlItem::as_table)
        .expect("http_headers table");
    assert_eq!(
        http_headers
            .get("X-Doc")
            .and_then(TomlItem::as_value)
            .and_then(toml_edit::Value::as_str),
        Some("42")
    );
}

#[tokio::test]
async fn write_value_preserves_comments_and_order() -> Result<()> {
    let tmp = tempdir().expect("tempdir");
    let original = r#"# Codex user configuration
model = "gpt-5.2"
approval_policy = "on-request"

[notice]
# Preserve this comment
hide_full_access_warning = true

[features]
unified_exec = true
"#;
    std::fs::write(tmp.path().join(CONFIG_TOML_FILE), original)?;

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    service
        .write_value(ConfigValueWriteParams {
            file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
            key_path: "features.personality".to_string(),
            value: serde_json::json!(true),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect("write succeeds");

    let updated = std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"# Codex user configuration
model = "gpt-5.2"
approval_policy = "on-request"

[notice]
# Preserve this comment
hide_full_access_warning = true

[features]
unified_exec = true
personality = true
"#;
    assert_eq!(updated, expected);
    Ok(())
}

#[tokio::test]
async fn psp_feature_configures_first_party_routing() -> Result<()> {
    let tmp = tempdir()?;
    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        Vec::new(),
        LoaderOverrides::without_managed_config_for_tests(),
        CloudConfigBundleLoader::default(),
    );

    let config = service
        .load_with_overrides(
            Some(
                [(
                    "features".to_string(),
                    serde_json::json!({ "apps": true, "psp": true }),
                )]
                .into_iter()
                .collect(),
            ),
            Default::default(),
        )
        .await?;

    assert!(config.features.enabled(codex_features::Feature::Psp));
    assert_eq!(
        config.http_client_factory(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
            .with_chatgpt_cookies([HeaderValue::from_static("oai-chat-psp=true")])
    );
    assert_eq!(
        config
            .config_layer_stack
            .effective_config()
            .get("features")
            .and_then(|features| features.get("psp")),
        Some(&toml::Value::Boolean(true))
    );
    Ok(())
}

#[tokio::test]
async fn clear_missing_nested_config_is_noop() -> Result<()> {
    let tmp = tempdir().expect("tempdir");
    let path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(&path, "")?;

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    let response = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(path.display().to_string()),
            key_path: "features.personality".to_string(),
            value: serde_json::Value::Null,
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect("clear missing config succeeds");

    assert_eq!(response.status, WriteStatus::Ok);
    assert_eq!(response.overridden_metadata, None);
    assert_eq!(std::fs::read_to_string(&path)?, "");
    Ok(())
}

#[tokio::test]
async fn clearing_user_setting_falls_back_to_packaged_default_without_override() -> Result<()> {
    let tmp = tempdir()?;
    let path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(&path, "hide_agent_reasoning = true\n")?;

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    let response = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(path.display().to_string()),
            key_path: "hide_agent_reasoning".to_string(),
            value: serde_json::Value::Null,
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await?;

    assert_eq!(response.status, WriteStatus::Ok);
    assert_eq!(response.overridden_metadata, None);
    assert_eq!(std::fs::read_to_string(&path)?, "");
    Ok(())
}

#[tokio::test]
async fn write_value_rejects_legacy_profile_selector() -> Result<()> {
    let tmp = tempdir().expect("tempdir");
    let path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(&path, "model = \"gpt-main\"\n")?;

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    let error = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(path.display().to_string()),
            key_path: "profile".to_string(),
            value: serde_json::json!("work"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect_err("legacy profile selector write should fail");

    assert_eq!(
        error.write_error_code(),
        Some(ConfigWriteErrorCode::ConfigValidationError)
    );
    assert!(
        error
            .to_string()
            .contains("`profile` is a legacy config selector"),
        "{error}"
    );
    assert_eq!(std::fs::read_to_string(&path)?, "model = \"gpt-main\"\n");
    Ok(())
}

#[tokio::test]
async fn write_value_rejects_legacy_profile_table() -> Result<()> {
    let tmp = tempdir().expect("tempdir");
    let path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(&path, "")?;

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    let error = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(path.display().to_string()),
            key_path: "profiles.work.model".to_string(),
            value: serde_json::json!("gpt-work"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect_err("legacy profile table write should fail");

    assert_eq!(
        error.write_error_code(),
        Some(ConfigWriteErrorCode::ConfigValidationError)
    );
    assert!(
        error
            .to_string()
            .contains("`profiles` contains legacy config profile tables"),
        "{error}"
    );
    assert_eq!(std::fs::read_to_string(&path)?, "");
    Ok(())
}

#[tokio::test]
async fn batch_write_rejects_legacy_profile_selector() -> Result<()> {
    let tmp = tempdir().expect("tempdir");
    let path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(&path, "model = \"gpt-main\"\n")?;

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    let error = service
        .batch_write(ConfigBatchWriteParams {
            edits: vec![
                codex_app_server_protocol::ConfigEdit {
                    key_path: "model".to_string(),
                    value: serde_json::json!("gpt-work"),
                    merge_strategy: MergeStrategy::Replace,
                },
                codex_app_server_protocol::ConfigEdit {
                    key_path: "profile".to_string(),
                    value: serde_json::json!("work"),
                    merge_strategy: MergeStrategy::Replace,
                },
            ],
            file_path: Some(path.display().to_string()),
            expected_version: None,
            reload_user_config: false,
        })
        .await
        .expect_err("legacy profile selector batch write should fail");

    assert_eq!(
        error.write_error_code(),
        Some(ConfigWriteErrorCode::ConfigValidationError)
    );
    assert!(
        error
            .to_string()
            .contains("`profile` is a legacy config selector"),
        "{error}"
    );
    assert_eq!(std::fs::read_to_string(&path)?, "model = \"gpt-main\"\n");
    Ok(())
}

#[tokio::test]
async fn write_value_supports_nested_app_paths() -> Result<()> {
    let tmp = tempdir().expect("tempdir");
    std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "")?;

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    service
        .write_value(ConfigValueWriteParams {
            file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
            key_path: "apps".to_string(),
            value: serde_json::json!({
                "app1": {
                    "enabled": false,
                },
            }),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect("write apps succeeds");

    service
        .write_value(ConfigValueWriteParams {
            file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
            key_path: "apps.app1.default_tools_approval_mode".to_string(),
            value: serde_json::json!("prompt"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect("write apps.app1.default_tools_approval_mode succeeds");

    let read = service
        .read(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await
        .expect("config read succeeds");

    assert_eq!(
        read.config.apps,
        Some(AppsConfig {
            default: None,
            apps: std::collections::HashMap::from([(
                "app1".to_string(),
                AppConfig {
                    enabled: false,
                    approvals_reviewer: None,
                    destructive_enabled: None,
                    open_world_enabled: None,
                    default_tools_approval_mode: Some(AppToolApproval::Prompt),
                    default_tools_enabled: None,
                    tools: None,
                    links: None,
                },
            )]),
        })
    );

    Ok(())
}

#[tokio::test]
async fn write_value_supports_custom_mcp_server_default_tool_approval_mode() -> Result<()> {
    let tmp = tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join(CONFIG_TOML_FILE),
        "[mcp_servers.docs]\ncommand = \"docs-server\"\n",
    )?;

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    service
        .write_value(ConfigValueWriteParams {
            file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
            key_path: "mcp_servers.docs.default_tools_approval_mode".to_string(),
            value: serde_json::json!("approve"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect("write mcp server default_tools_approval_mode succeeds");

    let contents = std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE))?;
    assert!(contents.contains("default_tools_approval_mode = \"approve\""));

    let read = service
        .read(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await
        .expect("config read succeeds");

    assert_eq!(
        read.config
            .additional
            .get("mcp_servers")
            .and_then(|servers| servers.get("docs"))
            .and_then(|docs| docs.get("default_tools_approval_mode")),
        Some(&serde_json::json!("approve"))
    );

    Ok(())
}

#[tokio::test]
async fn read_includes_origins_and_layers() {
    let tmp = tempdir().expect("tempdir");
    let user_path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(&user_path, "model = \"user\"").unwrap();
    let user_file = AbsolutePathBuf::try_from(user_path.clone()).expect("user file");

    let managed_path = tmp.path().join("managed_config.toml");
    std::fs::write(&managed_path, "approval_policy = \"never\"").unwrap();
    let managed_file = AbsolutePathBuf::try_from(managed_path.clone()).expect("managed file");

    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        vec![],
        LoaderOverrides::with_managed_config_path_for_tests(managed_path.clone()),
        CloudConfigBundleLoader::default(),
    );

    let response = service
        .read(ConfigReadParams {
            include_layers: true,
            cwd: None,
        })
        .await
        .expect("response");

    assert_eq!(response.config.approval_policy, Some(AskForApproval::Never));

    assert_eq!(
        response
            .origins
            .get("approval_policy")
            .expect("origin")
            .name,
        ApiConfigLayerSource::LegacyManagedConfigTomlFromFile {
            file: managed_file.clone()
        },
    );
    let layers = response.layers.expect("layers present");
    // Local macOS machines can surface an MDM-managed config layer at the
    // top of the stack; ignore it so this test stays focused on file/user/system ordering.
    let layers = if matches!(
        layers.first().map(|layer| &layer.name),
        Some(ApiConfigLayerSource::LegacyManagedConfigTomlFromMdm)
    ) {
        &layers[1..]
    } else {
        layers.as_slice()
    };
    assert_eq!(layers.len(), 3, "expected three layers");
    assert_eq!(
        layers.first().unwrap().name,
        ApiConfigLayerSource::LegacyManagedConfigTomlFromFile {
            file: managed_file.clone()
        }
    );
    assert_eq!(
        layers.get(1).unwrap().name,
        ApiConfigLayerSource::User {
            file: user_file.clone(),
            profile: None,
        }
    );
    assert!(matches!(
        layers.get(2).unwrap().name,
        ApiConfigLayerSource::System { .. }
    ));
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn write_value_succeeds_when_managed_preferences_expand_home_directory_paths() -> Result<()> {
    use base64::Engine;

    let tmp = tempdir().expect("tempdir");
    std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "model = \"user\"\n")?;

    let mut loader_overrides =
        LoaderOverrides::with_managed_config_path_for_tests(tmp.path().join("managed_config.toml"));
    loader_overrides.managed_preferences_base64 = Some(
        base64::prelude::BASE64_STANDARD.encode(
            r#"
sandbox_mode = "workspace-write"
[sandbox_workspace_write]
writable_roots = ["~/code"]
"#
            .as_bytes(),
        ),
    );

    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        vec![],
        loader_overrides,
        CloudConfigBundleLoader::default(),
    );

    let response = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
            key_path: "model".to_string(),
            value: serde_json::json!("updated"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect("write succeeds");

    assert_eq!(response.status, WriteStatus::Ok);
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).expect("read config"),
        "model = \"updated\"\n"
    );

    Ok(())
}

#[tokio::test]
async fn write_value_reports_override() {
    let tmp = tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join(CONFIG_TOML_FILE),
        "approval_policy = \"on-request\"",
    )
    .unwrap();

    let managed_path = tmp.path().join("managed_config.toml");
    std::fs::write(&managed_path, "approval_policy = \"never\"").unwrap();
    let managed_file = AbsolutePathBuf::try_from(managed_path.clone()).expect("managed file");

    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        vec![],
        LoaderOverrides::with_managed_config_path_for_tests(managed_path.clone()),
        CloudConfigBundleLoader::default(),
    );

    let result = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
            key_path: "approval_policy".to_string(),
            value: serde_json::json!("never"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect("result");

    let read_after = service
        .read(ConfigReadParams {
            include_layers: true,
            cwd: None,
        })
        .await
        .expect("read");
    assert_eq!(
        read_after.config.approval_policy,
        Some(AskForApproval::Never)
    );
    assert_eq!(
        read_after
            .origins
            .get("approval_policy")
            .expect("origin")
            .name,
        ApiConfigLayerSource::LegacyManagedConfigTomlFromFile {
            file: managed_file.clone()
        }
    );
    assert_eq!(result.status, WriteStatus::Ok);
    assert!(result.overridden_metadata.is_none());
}

#[tokio::test]
async fn version_conflict_rejected() {
    let tmp = tempdir().expect("tempdir");
    let user_path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(&user_path, "model = \"user\"").unwrap();

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    let error = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
            key_path: "model".to_string(),
            value: serde_json::json!("gpt-5.2"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: Some("sha256:bogus".to_string()),
        })
        .await
        .expect_err("should fail");

    assert_eq!(
        error.write_error_code(),
        Some(ConfigWriteErrorCode::ConfigVersionConflict)
    );
}

#[tokio::test]
async fn write_value_defaults_to_user_config_path() {
    let tmp = tempdir().expect("tempdir");
    std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "").unwrap();

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    service
        .write_value(ConfigValueWriteParams {
            file_path: None,
            key_path: "model".to_string(),
            value: serde_json::json!("gpt-new"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect("write succeeds");

    let contents = std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).expect("read config");
    assert!(
        contents.contains("model = \"gpt-new\""),
        "config.toml should be updated even when file_path is omitted"
    );
}

#[tokio::test]
async fn write_value_defaults_to_selected_user_config_path() {
    let tmp = tempdir().expect("tempdir");
    std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "model = \"gpt-main\"").unwrap();
    let selected_path = tmp.path().join("work.config.toml");
    std::fs::write(&selected_path, "").unwrap();

    let mut loader_overrides =
        LoaderOverrides::with_managed_config_path_for_tests(tmp.path().join("managed_config.toml"));
    loader_overrides.user_config_path =
        Some(AbsolutePathBuf::from_absolute_path(&selected_path).expect("selected config path"));
    loader_overrides.user_config_profile = Some("work".parse().expect("profile-v2 name"));
    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        vec![],
        loader_overrides,
        CloudConfigBundleLoader::default(),
    );
    service
        .write_value(ConfigValueWriteParams {
            file_path: None,
            key_path: "model".to_string(),
            value: serde_json::json!("gpt-work"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect("write succeeds");

    assert_eq!(
        std::fs::read_to_string(&selected_path).expect("read selected config"),
        "model = \"gpt-work\"\n"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).expect("read main config"),
        "model = \"gpt-main\""
    );
}

#[tokio::test]
async fn load_default_config_preserves_managed_requirements_and_selected_user_config_path() {
    let tmp = tempdir().expect("tempdir");
    std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "model = \"gpt-main\"").unwrap();
    std::fs::write(
        tmp.path().join("requirements.toml"),
        "allowed_login_methods = [\"api\"]\nallowed_chatgpt_workspaces = [\"managed-workspace\"]\n",
    )
    .unwrap();
    let selected_path = tmp.path().join("work.config.toml");
    std::fs::write(&selected_path, "not valid toml").unwrap();
    let selected_file =
        AbsolutePathBuf::from_absolute_path(&selected_path).expect("selected config path");

    let mut loader_overrides =
        LoaderOverrides::with_managed_config_path_for_tests(tmp.path().join("managed_config.toml"));
    loader_overrides.user_config_path = Some(selected_file.clone());
    loader_overrides.user_config_profile = Some("work".parse().expect("profile-v2 name"));
    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        vec![],
        loader_overrides,
        CloudConfigBundleLoader::default(),
    );

    service
        .load_latest_config(/*fallback_cwd*/ None)
        .await
        .expect_err("selected config should fail to load");
    let config = service
        .load_default_config()
        .await
        .expect("default config loads after selected config error");

    assert_eq!(
        config.config_layer_stack.get_user_config_file(),
        Some(&selected_file)
    );
    assert_eq!(
        config
            .config_layer_stack
            .requirements()
            .managed_auth_policy(),
        codex_config::ManagedAuthPolicy {
            allowed_login_methods: Some(vec![codex_protocol::config_types::ForcedLoginMethod::Api]),
            allowed_chatgpt_workspaces: Some(vec!["managed-workspace".to_string()]),
        }
    );
}

#[tokio::test]
async fn managed_auth_policy_survives_unusable_requirements_file_changes() -> Result<()> {
    let tmp = tempdir()?;
    std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "")?;
    let requirements_path = tmp.path().join("requirements.toml");
    std::fs::write(
        &requirements_path,
        "allowed_login_methods = [\"api\"]\nallowed_chatgpt_workspaces = [\"startup\"]\n",
    )?;
    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        Vec::new(),
        LoaderOverrides::with_managed_config_path_for_tests(tmp.path().join("managed_config.toml")),
        CloudConfigBundleLoader::default(),
    );
    let startup = service.load_latest_config(/*fallback_cwd*/ None).await?;
    let auth_manager = codex_login::AuthManager::shared_from_config(
        &startup, /*enable_codex_api_key_env*/ false,
    )
    .await?;
    std::fs::write(
        &requirements_path,
        "allowed_login_methods = [\"chatgpt\"]\nallowed_chatgpt_workspaces = []\n",
    )?;
    for refreshed in [
        service.load_latest_config(/*fallback_cwd*/ None).await?,
        service.load_latest_config_for_thread(&startup).await?,
    ] {
        assert_eq!(refreshed.forced_login_method, None);
        assert_eq!(refreshed.forced_chatgpt_workspace_id, None);
    }
    assert!(
        auth_manager.is_login_method_allowed(codex_protocol::config_types::ForcedLoginMethod::Api)
    );
    assert!(
        !auth_manager
            .is_login_method_allowed(codex_protocol::config_types::ForcedLoginMethod::Chatgpt)
    );
    assert_eq!(
        auth_manager.effective_chatgpt_workspaces(),
        Some(vec!["startup".to_string()])
    );
    Ok(())
}

#[tokio::test]
async fn provider_remapping_validates_fragments_even_when_source_is_overridden() -> Result<()> {
    for (managed_provider, strategy) in [
        (None, MergeStrategy::Upsert),
        (None, MergeStrategy::Replace),
        (Some("z_new"), MergeStrategy::Upsert),
        (Some("z_new"), MergeStrategy::Replace),
        (Some("z_managed"), MergeStrategy::Upsert),
        (Some("z_managed"), MergeStrategy::Replace),
    ] {
        let tmp = tempdir()?;
        let initial = r#"[features.network_proxy.credentials.a_working]
env = ["VENDOR_PASSWORD"]
patterns = ["^pin_[a-z]{8}$"]
url_prefixes = ["https://old.example"]
"#;
        std::fs::write(tmp.path().join(CONFIG_TOML_FILE), initial)?;
        let managed_path = tmp.path().join("managed.toml");
        if let Some(managed_provider) = managed_provider {
            std::fs::write(
                &managed_path,
                format!(
                    r#"[features.network_proxy.credentials.{managed_provider}]
env = ["VENDOR_PASSWORD"]
patterns = ["^pin_[a-z]{{8}}$"]
url_prefix_from_env = "VENDOR_ENDPOINT"
"#
                ),
            )?;
        }
        let service = ConfigManager::new_for_tests(
            tmp.path().to_path_buf(),
            vec![],
            LoaderOverrides::with_managed_config_path_for_tests(managed_path),
            CloudConfigBundleLoader::default(),
        );
        let result = service
            .write_value(ConfigValueWriteParams {
                file_path: None,
                key_path: match strategy {
                    MergeStrategy::Upsert => "features.network_proxy.credentials.z_new.env",
                    MergeStrategy::Replace => "features.network_proxy.credentials",
                }
                .to_string(),
                value: match strategy {
                    MergeStrategy::Upsert => serde_json::json!(["VENDOR_PASSWORD"]),
                    MergeStrategy::Replace => {
                        serde_json::json!({"z_new": {"env": ["VENDOR_PASSWORD"]}})
                    }
                },
                merge_strategy: strategy,
                expected_version: None,
            })
            .await;
        if managed_provider == Some("z_new") {
            result?;
        } else {
            assert_eq!(
                result.unwrap_err().write_error_code(),
                Some(ConfigWriteErrorCode::ConfigValidationError)
            );
            assert_eq!(
                std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE))?,
                initial
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn invalid_user_value_rejected_even_if_overridden_by_managed() {
    let tmp = tempdir().expect("tempdir");
    std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "model = \"user\"").unwrap();

    let managed_path = tmp.path().join("managed_config.toml");
    std::fs::write(&managed_path, "approval_policy = \"never\"").unwrap();

    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        vec![],
        LoaderOverrides::with_managed_config_path_for_tests(managed_path.clone()),
        CloudConfigBundleLoader::default(),
    );

    let error = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
            key_path: "approval_policy".to_string(),
            value: serde_json::json!("bogus"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect_err("should fail validation");

    assert_eq!(
        error.write_error_code(),
        Some(ConfigWriteErrorCode::ConfigValidationError)
    );

    let contents = std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).expect("read config");
    assert_eq!(contents.trim(), "model = \"user\"");
}

#[tokio::test]
async fn reserved_builtin_provider_override_rejected() {
    let tmp = tempdir().expect("tempdir");
    std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "model = \"user\"\n").unwrap();

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    let error = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
            key_path: "model_providers.openai.name".to_string(),
            value: serde_json::json!("OpenAI Override"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect_err("should reject reserved provider override");

    assert_eq!(
        error.write_error_code(),
        Some(ConfigWriteErrorCode::ConfigValidationError)
    );
    assert!(error.to_string().contains("reserved built-in provider IDs"));
    assert!(error.to_string().contains("`openai`"));

    let contents = std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).expect("read config");
    assert_eq!(contents, "model = \"user\"\n");
}

#[tokio::test]
async fn write_value_rejects_invalid_guardian_review_threshold() -> Result<()> {
    let tmp = tempdir()?;
    let path = tmp.path().join(CONFIG_TOML_FILE);
    let initial = "[features.guardianv2]\nenabled = true\nreview_threshold = 0.8\n";
    std::fs::write(&path, initial)?;
    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());

    let error = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(path.display().to_string()),
            key_path: "features.guardianv2.review_threshold".to_string(),
            value: serde_json::json!(2.0),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect_err("Guardian review thresholds above 1.0 must be rejected");

    assert_eq!(
        error.write_error_code(),
        Some(ConfigWriteErrorCode::ConfigValidationError)
    );
    assert_eq!(std::fs::read_to_string(&path)?, initial);
    Ok(())
}

#[tokio::test]
async fn write_value_rejects_feature_requirement_conflict() {
    let tmp = tempdir().expect("tempdir");
    std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "").unwrap();

    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        vec![],
        LoaderOverrides::without_managed_config_for_tests(),
        CloudConfigBundleFixture::loader_with_enterprise_requirement(
            r#"
[features]
personality = true
"#,
        ),
    );

    let error = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
            key_path: "features.personality".to_string(),
            value: serde_json::json!(false),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect_err("conflicting feature write should fail");

    assert_eq!(
        error.write_error_code(),
        Some(ConfigWriteErrorCode::ConfigValidationError)
    );
    assert!(
        error
            .to_string()
            .contains("invalid value for `features`: `features.personality=false`"),
        "{error}"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).unwrap(),
        ""
    );
}

#[tokio::test]
async fn write_value_rejects_exact_managed_requirement() {
    let tmp = tempdir().expect("tempdir");
    let path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(&path, "allow_login_shell = true\n").unwrap();

    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        vec![],
        LoaderOverrides::without_managed_config_for_tests(),
        CloudConfigBundleFixture::loader_with_enterprise_requirement("allow_login_shell = false"),
    );

    let error = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(path.display().to_string()),
            key_path: "allow_login_shell".to_string(),
            value: serde_json::json!(true),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect_err("managed exact field should be read-only");

    assert_eq!(
        error.write_error_code(),
        Some(ConfigWriteErrorCode::ConfigRequirementReadonly)
    );
    assert!(error.to_string().contains("`allow_login_shell`"));
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "allow_login_shell = true\n"
    );
}

fn toml_path(tmp: &Path, name: &str) -> String {
    tmp.join(name).to_string_lossy().replace('\\', "\\\\")
}

#[tokio::test]
async fn read_omits_origins_for_exact_managed_values() {
    for has_user_values in [true, false] {
        let tmp = tempdir().expect("tempdir");
        let user_config = if has_user_values {
            format!(
                r#"model = "user-model"
sqlite_home = "{}"
allow_login_shell = true

[feedback]
enabled = true
"#,
                toml_path(tmp.path(), "user-sqlite"),
            )
        } else {
            "model = \"user-model\"\n".to_string()
        };
        std::fs::write(tmp.path().join(CONFIG_TOML_FILE), user_config).unwrap();

        let requirements = format!(
            r#"sqlite_home = "{}"
allow_login_shell = false

[feedback]
enabled = false
"#,
            toml_path(tmp.path(), "managed-sqlite"),
        );
        let service = ConfigManager::new_for_tests(
            tmp.path().to_path_buf(),
            vec![],
            LoaderOverrides::without_managed_config_for_tests(),
            CloudConfigBundleFixture::loader_with_enterprise_requirement(requirements),
        );

        let response = service
            .read(ConfigReadParams {
                include_layers: false,
                cwd: None,
            })
            .await
            .expect("config read should succeed");

        assert_eq!(
            response.config.additional.get("sqlite_home"),
            Some(&serde_json::json!(tmp.path().join("managed-sqlite")))
        );
        assert_eq!(
            response.config.additional.get("allow_login_shell"),
            Some(&serde_json::json!(false))
        );
        assert_eq!(
            response.config.additional.get("feedback"),
            Some(&serde_json::json!({"enabled": false}))
        );
        for path in ["sqlite_home", "allow_login_shell", "feedback.enabled"] {
            assert!(!response.origins.contains_key(path), "origin for {path}");
        }
        assert!(response.origins.contains_key("model"));
    }
}

#[tokio::test]
async fn read_materializes_default_allow_login_shell() {
    let tmp = tempdir().expect("tempdir");
    std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "").unwrap();

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    let response = service
        .read(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await
        .expect("config read should succeed");

    assert_eq!(
        response.config.additional.get("allow_login_shell"),
        Some(&serde_json::json!(true))
    );
}

#[tokio::test]
async fn write_value_allows_unmanaged_sibling_of_exact_requirement() {
    let tmp = tempdir().expect("tempdir");
    let path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(&path, "").unwrap();

    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        vec![],
        LoaderOverrides::without_managed_config_for_tests(),
        CloudConfigBundleFixture::loader_with_enterprise_requirement(
            r#"
[windows]
sandbox_private_desktop = false
"#,
        ),
    );

    service
        .write_value(ConfigValueWriteParams {
            file_path: Some(path.display().to_string()),
            key_path: "windows.sandbox".to_string(),
            value: serde_json::json!("elevated"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect("unmanaged sibling should remain writable");

    assert!(
        std::fs::read_to_string(path)
            .unwrap()
            .contains("sandbox = \"elevated\"")
    );
}

#[tokio::test]
async fn read_reports_managed_overrides_user_and_session_flags() {
    let tmp = tempdir().expect("tempdir");
    let user_path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(&user_path, "model = \"user\"").unwrap();
    let user_file = AbsolutePathBuf::try_from(user_path.clone()).expect("user file");

    let managed_path = tmp.path().join("managed_config.toml");
    std::fs::write(&managed_path, "model = \"system\"").unwrap();
    let managed_file = AbsolutePathBuf::try_from(managed_path.clone()).expect("managed file");

    let cli_overrides = vec![(
        "model".to_string(),
        TomlValue::String("session".to_string()),
    )];

    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        cli_overrides,
        LoaderOverrides::with_managed_config_path_for_tests(managed_path.clone()),
        CloudConfigBundleLoader::default(),
    );

    let response = service
        .read(ConfigReadParams {
            include_layers: true,
            cwd: None,
        })
        .await
        .expect("response");

    assert_eq!(response.config.model.as_deref(), Some("system"));
    assert_eq!(
        response.origins.get("model").expect("origin").name,
        ApiConfigLayerSource::LegacyManagedConfigTomlFromFile {
            file: managed_file.clone()
        },
    );
    let layers = response.layers.expect("layers");
    // Local macOS machines can surface an MDM-managed config layer at the
    // top of the stack; ignore it so this test stays focused on file/session/user ordering.
    let layers = if matches!(
        layers.first().map(|layer| &layer.name),
        Some(ApiConfigLayerSource::LegacyManagedConfigTomlFromMdm)
    ) {
        &layers[1..]
    } else {
        layers.as_slice()
    };
    assert_eq!(
        layers.first().unwrap().name,
        ApiConfigLayerSource::LegacyManagedConfigTomlFromFile { file: managed_file }
    );
    assert_eq!(
        layers.get(1).unwrap().name,
        ApiConfigLayerSource::SessionFlags
    );
    assert_eq!(
        layers.get(2).unwrap().name,
        ApiConfigLayerSource::User {
            file: user_file,
            profile: None
        }
    );
}

#[tokio::test]
async fn write_value_reports_managed_override() {
    let tmp = tempdir().expect("tempdir");
    std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "").unwrap();

    let managed_path = tmp.path().join("managed_config.toml");
    std::fs::write(&managed_path, "approval_policy = \"never\"").unwrap();
    let managed_file = AbsolutePathBuf::try_from(managed_path.clone()).expect("managed file");

    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        vec![],
        LoaderOverrides::with_managed_config_path_for_tests(managed_path.clone()),
        CloudConfigBundleLoader::default(),
    );

    let result = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(tmp.path().join(CONFIG_TOML_FILE).display().to_string()),
            key_path: "approval_policy".to_string(),
            value: serde_json::json!("on-request"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect("result");

    assert_eq!(result.status, WriteStatus::OkOverridden);
    let overridden = result.overridden_metadata.expect("overridden metadata");
    assert_eq!(
        overridden.overriding_layer.name,
        ApiConfigLayerSource::LegacyManagedConfigTomlFromFile { file: managed_file }
    );
    assert_eq!(overridden.effective_value, serde_json::json!("never"));
}

/// Legacy managed feature toggles own their normalized enabled origin and override metadata.
#[tokio::test]
async fn multi_agent_v2_boolean_layer_owns_enabled_origin_and_overrides() {
    let tmp = tempdir().expect("tempdir");
    let user_path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(
        &user_path,
        "[features.multi_agent_v2]\nenabled = true\nsubagent_usage_hint_text = \"keep\"\n\n[features.network_proxy]\nenabled = true\ncredential_broker = true\n",
    )
    .expect("user config");

    let managed_path = tmp.path().join("managed_config.toml");
    std::fs::write(
        &managed_path,
        "[features]\nmulti_agent_v2 = false\nnetwork_proxy = false\n",
    )
    .expect("managed config");
    let managed_file = AbsolutePathBuf::try_from(managed_path.clone()).expect("managed file");
    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        vec![],
        LoaderOverrides::with_managed_config_path_for_tests(managed_path),
        CloudConfigBundleLoader::default(),
    );

    let read = service
        .read(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await
        .expect("read config");
    for path in [
        "features.multi_agent_v2.enabled",
        "features.network_proxy",
        "features.network_proxy.enabled",
    ] {
        assert_eq!(
            read.origins.get(path).expect("managed feature origin").name,
            ApiConfigLayerSource::LegacyManagedConfigTomlFromFile {
                file: managed_file.clone(),
            },
        );
    }

    let result = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(user_path.display().to_string()),
            key_path: "features.multi_agent_v2.enabled".to_string(),
            value: serde_json::json!(true),
            merge_strategy: MergeStrategy::Upsert,
            expected_version: None,
        })
        .await
        .expect("write config");
    assert_eq!(result.status, WriteStatus::OkOverridden);
    let overridden = result.overridden_metadata.expect("overridden metadata");
    assert_eq!(
        overridden.overriding_layer.name,
        ApiConfigLayerSource::LegacyManagedConfigTomlFromFile { file: managed_file }
    );
    assert_eq!(overridden.effective_value, serde_json::json!(false));
}

#[tokio::test]
async fn structured_feature_toggle_ignores_unrelated_managed_settings() -> Result<()> {
    let tmp = tempdir()?;
    let user_path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(
        &user_path,
        "[features.network_proxy]\nenabled = false\ncredential_broker = true\n",
    )?;

    let managed_path = tmp.path().join("managed_config.toml");
    std::fs::write(
        &managed_path,
        "[features.network_proxy]\nproxy_url = 'http://127.0.0.1:4321'\n",
    )?;
    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        vec![],
        LoaderOverrides::with_managed_config_path_for_tests(managed_path.clone()),
        CloudConfigBundleLoader::default(),
    );

    service
        .write_value(ConfigValueWriteParams {
            file_path: Some(user_path.display().to_string()),
            key_path: "features.network_proxy".to_string(),
            value: serde_json::Value::Null,
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await?;
    assert_eq!(
        toml::from_str::<TomlValue>(&std::fs::read_to_string(&user_path)?)?,
        toml::from_str::<TomlValue>(
            "[features.network_proxy]\nenabled = false\ncredential_broker = true\n"
        )?
    );

    let result = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(user_path.display().to_string()),
            key_path: "features.network_proxy".to_string(),
            value: serde_json::json!(true),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await?;

    assert_eq!(result.status, WriteStatus::Ok);
    assert_eq!(result.overridden_metadata, None);

    let selected_path = tmp.path().join("work.config.toml");
    std::fs::write(&selected_path, "")?;
    let mut loader_overrides = LoaderOverrides::with_managed_config_path_for_tests(managed_path);
    loader_overrides.user_config_path = Some(AbsolutePathBuf::from_absolute_path(&selected_path)?);
    loader_overrides.user_config_profile = Some("work".parse()?);
    let profile_service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        vec![],
        loader_overrides,
        CloudConfigBundleLoader::default(),
    );
    profile_service
        .write_value(ConfigValueWriteParams {
            file_path: Some(selected_path.display().to_string()),
            key_path: "features.network_proxy".to_string(),
            value: serde_json::Value::Null,
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await?;
    assert_eq!(
        toml::from_str::<TomlValue>(&std::fs::read_to_string(&selected_path)?)?,
        toml::from_str::<TomlValue>("[features]\nnetwork_proxy = false\n")?
    );

    service
        .batch_write(ConfigBatchWriteParams {
            edits: vec![
                codex_app_server_protocol::ConfigEdit {
                    key_path: "features.network_proxy.credential_broker".to_string(),
                    value: serde_json::Value::Null,
                    merge_strategy: MergeStrategy::Replace,
                },
                codex_app_server_protocol::ConfigEdit {
                    key_path: "features.network_proxy".to_string(),
                    value: serde_json::Value::Null,
                    merge_strategy: MergeStrategy::Replace,
                },
            ],
            file_path: Some(user_path.display().to_string()),
            expected_version: None,
            reload_user_config: false,
        })
        .await?;
    assert!(
        toml::from_str::<TomlValue>(&std::fs::read_to_string(&user_path)?)?
            .get("features")
            .and_then(|features| features.get("network_proxy"))
            .is_none()
    );

    let configured_provider = "[features.network_proxy]\nenabled = true\n\n\
         [features.network_proxy.credentials.vendor]\n\
         env = ['VENDOR_TOKEN']\n\
         patterns = ['token_[a-z]{24}']\n\
         url_prefixes = ['https://api.vendor.example']\n";
    std::fs::write(&user_path, configured_provider)?;
    service
        .write_value(ConfigValueWriteParams {
            file_path: Some(user_path.display().to_string()),
            key_path: "features.network_proxy".to_string(),
            value: serde_json::Value::Null,
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await?;
    let updated: TomlValue = toml::from_str(&std::fs::read_to_string(&user_path)?)?;
    let feature = &updated["features"]["network_proxy"];
    assert_eq!(
        feature.get("enabled").and_then(TomlValue::as_bool),
        Some(false)
    );
    assert!(feature.get("credentials").is_some());
    Ok(())
}

#[tokio::test]
async fn upsert_merges_tables_replace_overwrites() -> Result<()> {
    let tmp = tempdir().expect("tempdir");
    let path = tmp.path().join(CONFIG_TOML_FILE);
    let base = r#"[mcp_servers.linear]
bearer_token_env_var = "TOKEN"
name = "linear"
url = "https://linear.example"

[mcp_servers.linear.env_http_headers]
existing = "keep"

[mcp_servers.linear.http_headers]
alpha = "a"
"#;

    let overlay = serde_json::json!({
        "bearer_token_env_var": "NEW_TOKEN",
        "http_headers": {
            "alpha": "updated",
            "beta": "b"
        },
        "name": "linear",
        "url": "https://linear.example"
    });

    std::fs::write(&path, base)?;

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    service
        .write_value(ConfigValueWriteParams {
            file_path: Some(path.display().to_string()),
            key_path: "mcp_servers.linear".to_string(),
            value: overlay.clone(),
            merge_strategy: MergeStrategy::Upsert,
            expected_version: None,
        })
        .await
        .expect("upsert succeeds");

    let upserted: TomlValue = toml::from_str(&std::fs::read_to_string(&path)?)?;
    let expected_upsert: TomlValue = toml::from_str(
        r#"[mcp_servers.linear]
bearer_token_env_var = "NEW_TOKEN"
name = "linear"
url = "https://linear.example"

[mcp_servers.linear.env_http_headers]
existing = "keep"

[mcp_servers.linear.http_headers]
alpha = "updated"
beta = "b"
"#,
    )?;
    assert_eq!(upserted, expected_upsert);

    std::fs::write(&path, base)?;

    service
        .write_value(ConfigValueWriteParams {
            file_path: Some(path.display().to_string()),
            key_path: "mcp_servers.linear".to_string(),
            value: overlay,
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await
        .expect("replace succeeds");

    let replaced: TomlValue = toml::from_str(&std::fs::read_to_string(&path)?)?;
    let expected_replace: TomlValue = toml::from_str(
        r#"[mcp_servers.linear]
bearer_token_env_var = "NEW_TOKEN"
name = "linear"
url = "https://linear.example"

[mcp_servers.linear.http_headers]
alpha = "updated"
beta = "b"
"#,
    )?;
    assert_eq!(replaced, expected_replace);

    Ok(())
}

#[tokio::test]
async fn config_writes_apply_path_sensitive_merge_rules() -> Result<()> {
    let cases = [
        (
            r#"[shell_environment_policy]
exclude = ["AWS_*"]
"#,
            "shell_environment_policy",
            serde_json::json!({"filters": {"AWS_*": "include"}}),
            r#"[shell_environment_policy.filters]
"AWS_*" = "include"
"#,
        ),
        (
            r#"[shell_environment_policy]
inherit = "core"
exclude = ["AWS_*"]
"#,
            "shell_environment_policy.filters",
            serde_json::json!({"AWS_*": "include"}),
            r#"[shell_environment_policy]
inherit = "core"

[shell_environment_policy.filters]
"AWS_*" = "include"
"#,
        ),
        (
            r#"[shell_environment_policy.filters]
"AWS_*" = "include"
"#,
            "shell_environment_policy.exclude",
            serde_json::json!(["AWS_*"]),
            r#"[shell_environment_policy]
exclude = ["AWS_*"]
"#,
        ),
        (
            r#"[shell_environment_policy]
exclude = ["AWS_*"]
include_only = ["PATH"]
"#,
            "shell_environment_policy.filters",
            serde_json::json!({}),
            r#"[shell_environment_policy.filters]
"#,
        ),
        (
            r#"[shell_environment_policy.filters]
"AWS_*" = "include"
"#,
            "shell_environment_policy.exclude",
            serde_json::json!([]),
            r#"[shell_environment_policy]
exclude = []
"#,
        ),
        (
            r#"[shell_environment_policy.filters]
"aws_*" = "exclude"
"#,
            "shell_environment_policy.filters",
            serde_json::json!({"AWS_*": "include"}),
            r#"[shell_environment_policy.filters]
"aws_*" = "include"
"#,
        ),
        (
            r#"[shell_environment_policy.filters]
"aws_*" = "exclude"
"#,
            "shell_environment_policy.filters.AWS_*",
            serde_json::json!("include"),
            r#"[shell_environment_policy.filters]
"aws_*" = "include"
"#,
        ),
        (
            r#"[shell_environment_policy.filters]
"секрет_*" = "exclude"
"#,
            "shell_environment_policy.filters.СЕКРЕТ_*",
            serde_json::json!("include"),
            r#"[shell_environment_policy.filters]
"секрет_*" = "include"
"#,
        ),
        (
            r#"[permissions.dev.network.domains]
"example.com" = "deny"
"#,
            "permissions.dev.network.domains",
            serde_json::json!({"EXAMPLE.COM": "allow"}),
            r#"[permissions.dev.network.domains]
"example.com" = "allow"
"#,
        ),
        (
            r#"[memories]
no_memories_if_mcp_or_web_search = false
"#,
            "memories",
            serde_json::json!({"disable_on_external_context": true}),
            r#"[memories]
disable_on_external_context = true
"#,
        ),
        (
            r#"[features]
multi_agent_v2 = true
"#,
            "features.multi_agent_v2.subagent_usage_hint_text",
            serde_json::json!("Delegate carefully."),
            r#"[features.multi_agent_v2]
enabled = true
subagent_usage_hint_text = "Delegate carefully."
"#,
        ),
        (
            r#"[features]
multi_agent_v2 = true
"#,
            "features.multi_agent_v2",
            serde_json::json!({"subagent_usage_hint_text": "Delegate carefully."}),
            r#"[features.multi_agent_v2]
enabled = true
subagent_usage_hint_text = "Delegate carefully."
"#,
        ),
        (
            r#"[features.multi_agent_v2]
enabled = true
subagent_usage_hint_text = "Delegate carefully."
"#,
            "features.multi_agent_v2",
            serde_json::json!(false),
            r#"[features.multi_agent_v2]
enabled = false
subagent_usage_hint_text = "Delegate carefully."
"#,
        ),
        (
            r#"[features.multi_agent_v2]
enabled = true
subagent_usage_hint_text = "Delegate carefully."
"#,
            "features.multi_agent_v2",
            serde_json::Value::Null,
            "",
        ),
        (
            r#"[features]
network_proxy = true
"#,
            "features.network_proxy.credential_broker",
            serde_json::json!(true),
            r#"[features.network_proxy]
enabled = true
credential_broker = true
"#,
        ),
        (
            r#"[features.network_proxy]
enabled = true
credential_broker = true
"#,
            "features.network_proxy",
            serde_json::json!(false),
            r#"[features.network_proxy]
enabled = false
credential_broker = true
"#,
        ),
        (
            r#"[desktop.features.multi_agent_v2]
custom = true
"#,
            "desktop.features.multi_agent_v2",
            serde_json::json!(false),
            r#"[desktop.features]
multi_agent_v2 = false
"#,
        ),
        (
            r#"[desktop.features]
multi_agent_v2 = true
"#,
            "desktop.features.multi_agent_v2",
            serde_json::json!({"custom": true}),
            r#"[desktop.features.multi_agent_v2]
custom = true
"#,
        ),
    ];

    for (base, key_path, value, expected) in cases {
        let tmp = tempdir()?;
        let path = tmp.path().join(CONFIG_TOML_FILE);
        std::fs::write(&path, base)?;

        let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
        service
            .write_value(ConfigValueWriteParams {
                file_path: Some(path.display().to_string()),
                key_path: key_path.to_string(),
                value,
                merge_strategy: MergeStrategy::Upsert,
                expected_version: None,
            })
            .await?;

        let updated: TomlValue = toml::from_str(&std::fs::read_to_string(&path)?)?;
        let expected: TomlValue = toml::from_str(expected)?;
        assert_eq!(updated, expected);

        service
            .read(ConfigReadParams {
                include_layers: false,
                cwd: None,
            })
            .await?;
    }

    Ok(())
}

#[tokio::test]
async fn clear_shell_environment_filter_ignores_ascii_case() -> Result<()> {
    let tmp = tempdir()?;
    let path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(
        &path,
        r#"[shell_environment_policy.filters]
"aws_*" = "exclude"
"keep_*" = "include"
"#,
    )?;

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    let response = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(path.display().to_string()),
            key_path: "shell_environment_policy.filters.AWS_*".to_string(),
            value: serde_json::Value::Null,
            merge_strategy: MergeStrategy::Upsert,
            expected_version: None,
        })
        .await?;

    assert_eq!(response.status, WriteStatus::Ok);
    assert_eq!(response.overridden_metadata, None);
    assert_eq!(
        std::fs::read_to_string(&path)?,
        r#"[shell_environment_policy.filters]
"keep_*" = "include"
"#
    );
    service
        .read(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await?;

    Ok(())
}

#[tokio::test]
async fn upsert_shell_environment_scalar_preserves_unrelated_formatting() -> Result<()> {
    let tmp = tempdir()?;
    let path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(
        &path,
        r#"[shell_environment_policy]
inherit = "all"
exclude = [
    "AWS_*", # keep this comment
]
set = { KEEP = "1", OTHER = "2", GH_HOST = "github.stale.example" } # keep this inline table
"#,
    )?;

    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());
    service
        .write_value(ConfigValueWriteParams {
            file_path: Some(path.display().to_string()),
            key_path: "shell_environment_policy.inherit".to_string(),
            value: serde_json::json!("core"),
            merge_strategy: MergeStrategy::Upsert,
            expected_version: None,
        })
        .await?;

    assert_eq!(
        std::fs::read_to_string(&path)?,
        r#"[shell_environment_policy]
inherit = "core"
exclude = [
    "AWS_*", # keep this comment
]
set = { KEEP = "1", OTHER = "2", GH_HOST = "github.stale.example" } # keep this inline table
"#
    );
    for (value, expected_set) in [
        (
            serde_json::json!("github.trusted.example"),
            "KEEP='1'\nOTHER='2'\nGH_HOST='github.stale.example'\ngh_host='github.trusted.example'",
        ),
        (
            serde_json::Value::Null,
            "KEEP='1'\nOTHER='2'\nGH_HOST='github.stale.example'",
        ),
    ] {
        let response = service
            .write_value(ConfigValueWriteParams {
                file_path: Some(path.display().to_string()),
                key_path: "shell_environment_policy.set.gh_host".to_string(),
                value,
                merge_strategy: MergeStrategy::Upsert,
                expected_version: None,
            })
            .await?;
        assert_eq!(response.status, WriteStatus::Ok);
        assert_eq!(response.overridden_metadata, None);
        let config: TomlValue = toml::from_str(&std::fs::read_to_string(&path)?)?;
        assert_eq!(
            config["shell_environment_policy"]["set"],
            toml::from_str::<TomlValue>(expected_set)?
        );
    }
    service
        .read(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await?;

    Ok(())
}

#[tokio::test]
async fn upsert_shell_environment_filter_scalar_preserves_formatting_and_version() -> Result<()> {
    let tmp = tempdir()?;
    let path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(
        &path,
        r#"[shell_environment_policy]
set = { KEEP = "1", OTHER = "2" } # keep this inline table

[shell_environment_policy.filters]
"AWS_*" = "exclude" # keep this edited comment
"KEEP_*" = "include" # keep this untouched comment
"#,
    )?;
    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());

    let response = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(path.display().to_string()),
            key_path: "shell_environment_policy.filters.aws_*".to_string(),
            value: serde_json::json!("include"),
            merge_strategy: MergeStrategy::Upsert,
            expected_version: None,
        })
        .await?;

    assert_eq!(
        std::fs::read_to_string(&path)?,
        r#"[shell_environment_policy]
set = { KEEP = "1", OTHER = "2" } # keep this inline table

[shell_environment_policy.filters]
"AWS_*" = "include" # keep this edited comment
"KEEP_*" = "include" # keep this untouched comment
"#
    );
    service
        .write_value(ConfigValueWriteParams {
            file_path: Some(path.display().to_string()),
            key_path: "shell_environment_policy.filters.AWS_*".to_string(),
            value: serde_json::json!("exclude"),
            merge_strategy: MergeStrategy::Upsert,
            expected_version: Some(response.version),
        })
        .await?;
    service
        .read(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await?;

    Ok(())
}

#[tokio::test]
async fn shell_environment_upsert_rejects_case_variant_filters_in_one_edit() -> Result<()> {
    let tmp = tempdir()?;
    let path = tmp.path().join(CONFIG_TOML_FILE);
    let initial = r#"[shell_environment_policy.filters]
"KEEP_*" = "include"
"#;
    std::fs::write(&path, initial)?;
    let service = ConfigManager::without_managed_config_for_tests(tmp.path().to_path_buf());

    let error = service
        .write_value(ConfigValueWriteParams {
            file_path: Some(path.display().to_string()),
            key_path: "shell_environment_policy.filters".to_string(),
            value: serde_json::json!({"AWS_*": "include", "aws_*": "exclude"}),
            merge_strategy: MergeStrategy::Upsert,
            expected_version: None,
        })
        .await
        .expect_err("one filter-map edit must not contain case-variant keys");

    assert_eq!(
        error.write_error_code(),
        Some(ConfigWriteErrorCode::ConfigValidationError)
    );
    assert!(
        error
            .to_string()
            .contains("duplicate shell environment filter")
    );
    assert_eq!(std::fs::read_to_string(&path)?, initial);
    Ok(())
}

#[tokio::test]
async fn shell_environment_representation_switch_reports_managed_override() -> Result<()> {
    let cases = [
        (
            r#"[shell_environment_policy]
exclude = ["AWS_*"]
"#,
            "shell_environment_policy.filters.AWS_*",
            serde_json::json!("include"),
            Some(serde_json::Value::Null),
        ),
        (
            r#"[shell_environment_policy.filters]
"AWS_*" = "include"
"#,
            "shell_environment_policy.exclude",
            serde_json::json!(["AWS_*"]),
            Some(serde_json::Value::Null),
        ),
        (
            "[shell_environment_policy.set]\nGH_HOST = 'github.managed.example'\n",
            "shell_environment_policy.set.GH_HOST",
            serde_json::json!("github.user.example"),
            Some(serde_json::json!("github.managed.example")),
        ),
        (
            "[shell_environment_policy.set]\nGH_HOST = 'github.managed.example'\n",
            "shell_environment_policy.set.gh_host",
            serde_json::json!("github.user.example"),
            None,
        ),
        (
            "[shell_environment_policy.set]\ngh_host = 'github.managed.example'\n",
            "shell_environment_policy.set.gh_host",
            serde_json::json!("github.user.example"),
            Some(serde_json::json!("github.managed.example")),
        ),
        (
            "[shell_environment_policy.set]\ngh_host = 'github.managed.example'\n",
            "shell_environment_policy.set.GH_HOST",
            serde_json::json!("github.user.example"),
            None,
        ),
    ];

    for (managed, key_path, value, expected_effective_value) in cases {
        let tmp = tempdir()?;
        let path = tmp.path().join(CONFIG_TOML_FILE);
        std::fs::write(
            &path,
            "[shell_environment_policy.set]\n\
             GH_HOST = 'github.original.example'\ngh_host = 'github.original-lower.example'\n",
        )?;
        let managed_path = tmp.path().join("managed_config.toml");
        std::fs::write(&managed_path, managed)?;
        let managed_file = AbsolutePathBuf::try_from(managed_path.clone())?;
        let service = ConfigManager::new_for_tests(
            tmp.path().to_path_buf(),
            vec![],
            LoaderOverrides::with_managed_config_path_for_tests(managed_path),
            CloudConfigBundleLoader::default(),
        );

        let response = service
            .write_value(ConfigValueWriteParams {
                file_path: Some(path.display().to_string()),
                key_path: key_path.to_string(),
                value,
                merge_strategy: MergeStrategy::Upsert,
                expected_version: None,
            })
            .await?;

        if let Some(expected_effective_value) = expected_effective_value {
            assert_eq!(response.status, WriteStatus::OkOverridden);
            let overridden = response
                .overridden_metadata
                .expect("managed setting should override the user edit");
            assert_eq!(
                overridden.overriding_layer.name,
                ApiConfigLayerSource::LegacyManagedConfigTomlFromFile { file: managed_file }
            );
            assert_eq!(overridden.effective_value, expected_effective_value);
        } else {
            assert_eq!(response.status, WriteStatus::Ok);
            assert_eq!(response.overridden_metadata, None);
        }
        service
            .read(ConfigReadParams {
                include_layers: false,
                cwd: None,
            })
            .await?;
    }

    Ok(())
}

#[tokio::test]
async fn allowed_login_methods_follow_current_forced_workspaces() -> Result<()> {
    use codex_protocol::config_types::ForcedLoginMethod;

    let tmp = tempdir()?;
    std::fs::write(tmp.path().join(CONFIG_TOML_FILE), "")?;
    std::fs::write(
        tmp.path().join("requirements.toml"),
        "allowed_chatgpt_workspaces = ['managed']",
    )?;
    let service = ConfigManager::new_for_tests(
        tmp.path().to_path_buf(),
        Vec::new(),
        LoaderOverrides::with_managed_config_path_for_tests(tmp.path().join("managed_config.toml")),
        CloudConfigBundleLoader::default(),
    );
    let config = service.load_latest_config(/*fallback_cwd*/ None).await?;
    let auth = codex_login::AuthManager::shared_from_config(
        &config, /*enable_codex_api_key_env*/ false,
    )
    .await?;
    for (workspaces, expected) in [
        (
            Some(vec!["managed".to_string()]),
            vec![ForcedLoginMethod::Api, ForcedLoginMethod::Chatgpt],
        ),
        (
            Some(vec!["other".to_string()]),
            vec![ForcedLoginMethod::Api],
        ),
        (Some(Vec::new()), vec![ForcedLoginMethod::Api]),
        (
            None,
            vec![ForcedLoginMethod::Api, ForcedLoginMethod::Chatgpt],
        ),
    ] {
        auth.set_forced_chatgpt_workspace_id(workspaces);
        assert_eq!(auth.allowed_login_methods(), expected);
    }
    Ok(())
}
