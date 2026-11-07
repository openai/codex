//! Exercises provider management through the public app-server requests.
#![cfg(test)]

use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;

const CLEARED_AUTH_ENV: &[(&str, Option<&str>)] = &[
    ("OPENAI_API_KEY", None),
    ("CODEX_API_KEY", None),
    ("CODEX_ACCESS_TOKEN", None),
    ("OPENAI_FEDERATION_RULE_ID", None),
    ("OPENAI_IDENTITY_TOKEN_FILE", None),
    ("OPENAI_WORKLOAD_IDENTITY_CONTEXT", None),
];

#[tokio::test]
async fn create_and_patch_gateway_providers_without_changing_the_default() -> Result<()> {
    let home = TempDir::new()?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(CLEARED_AUTH_ENV)
        .build_initialized()
        .await?;
    let settings = json!({"type":"customGateway", "name":"Team Gateway", "baseUrl":"https://gateway.example.test/v1", "envKey":"TEAM_KEY"});
    // Pipeline creation requests: allocation must run against the preceding committed write.
    let first = server
        .send_request(
            "modelProvider/config/write",
            Some(json!({"settings":settings})),
        )
        .await?;
    let second = server
        .send_request(
            "modelProvider/config/write",
            Some(json!({"settings":settings})),
        )
        .await?;
    let saved: Value = server.read_response(first).await?;
    let duplicate: Value = server.read_response(second).await?;
    assert_eq!(duplicate["provider"]["id"], "team-gateway-2");
    let mut expected = json!({"id":"team-gateway", "name":"Team Gateway", "canEdit":true,
        "baseUrl":"https://gateway.example.test/v1",
        "settings":{"type":"customGateway", "auth":{"mode":"environmentVariable", "envKey":"TEAM_KEY"}}});
    assert_eq!(saved, json!({"provider":expected}));
    // Disjoint stale-client patches preserve siblings; writing a field again wins.
    for (mut patch, field, value) in [
        (json!({"name":"Renamed"}), "name", json!("Renamed")),
        (
            json!({"baseUrl":"https://new.example.test"}),
            "baseUrl",
            json!("https://new.example.test"),
        ),
        (json!({"name":"Latest"}), "name", json!("Latest")),
        (
            json!({"envKey":null}),
            "settings",
            json!({"type":"customGateway", "auth":{"mode":"none"}}),
        ),
        (json!({"baseUrl":null}), "baseUrl", Value::Null),
    ] {
        patch["type"] = json!("customGateway");
        patch["providerId"] = json!("team-gateway");
        let params = serde_json::from_value(json!({"settings":patch}))?;
        let updated: Value = server
            .request(|request_id| ClientRequest::ModelProviderConfigWrite { request_id, params })
            .await?;
        expected[field] = value;
        assert_eq!(updated, json!({"provider":expected}));
    }
    let path = home.path().join("config.toml");
    let before = std::fs::read_to_string(&path)?;
    let clear = server.send_request("modelProvider/config/write", Some(json!({"settings":{"type":"customGateway","providerId":"team-gateway","name":null, "baseUrl":"https://must-not-save.example.test"}}))).await?;
    let error = server
        .read_stream_until_error_message(RequestId::Integer(clear))
        .await?;
    assert_eq!(error.error.code, -32600);
    assert_eq!(std::fs::read_to_string(&path)?, before);
    let list = server
        .send_request("modelProvider/list", Some(json!({})))
        .await?;
    let after: Value = server.read_response(list).await?;
    assert_eq!(
        after["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "team-gateway"),
        Some(&expected)
    );
    assert_eq!(after["defaultProviderId"], "openai");
    let persisted: Value = toml::from_str(&before)?;
    assert_eq!(
        persisted["model_providers"]["team-gateway"],
        json!({"name":"Latest", "wire_api":"responses"})
    );
    Ok(())
}

#[tokio::test]
async fn custom_provider_edits_preserve_other_fields_and_respect_policy() -> Result<()> {
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("requirements.toml"),
        "model_provider = 'corp.gateway'\n[model_providers.\"corp.gateway\"]\nname = 'Managed gateway'\nbase_url = 'https://managed.example.test'\n",
    )?;
    let config_path = home.path().join("config.toml");
    let mut expected = json!({
        "model_providers": {
            "basic": {"name":"Basic", "base_url":"https://user:password@basic.example.test/v1"},
            "external": {
                "name": "External",
                "base_url": "https://external.example.test",
                "http_headers": {"Authorization": "Bearer test-only"},
                "query_params": {"api-version": "v1"},
            }
        }
    });
    std::fs::write(&config_path, toml::to_string(&expected)?)?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(CLEARED_AUTH_ENV)
        .build_initialized()
        .await?;
    let edit = server
        .send_request(
            "modelProvider/config/write",
            Some(json!({"settings": {"type":"customGateway", "providerId":"external", "name":"Renamed"}})),
        )
        .await?;
    let saved: Value = server.read_response(edit).await?;
    assert_eq!(saved["provider"]["id"], "external");
    expected["model_providers"]["external"]["name"] = json!("Renamed");
    let actual: Value = toml::from_str(&std::fs::read_to_string(&config_path)?)?;
    assert_eq!(actual, expected);

    for (id, mut patch, error_code) in [
        (
            "corp.gateway",
            json!({"name": "Changed"}),
            "configLayerReadonly",
        ),
        (
            "openai",
            json!({"name": "Changed"}),
            "configValidationError",
        ),
        ("external", json!({"envKey": null}), "configValidationError"),
        (
            "basic",
            json!({"envKey": "NEW_KEY"}),
            "configValidationError",
        ),
        ("basic", json!({"envKey": null}), "configValidationError"),
        (
            "ollama-chat",
            json!({"name": "Renamed"}),
            "configValidationError",
        ),
        ("ollama", json!({}), "configValidationError"),
        ("gcp", json!({}), "configValidationError"),
    ] {
        patch["type"] = json!("customGateway");
        patch["providerId"] = json!(id);
        let edit = server
            .send_request(
                "modelProvider/config/write",
                Some(json!({"settings": patch})),
            )
            .await?;
        let error = server
            .read_stream_until_error_message(RequestId::Integer(edit))
            .await?;
        if id == "ollama-chat" {
            insta::assert_snapshot!(
                error.error.message,
                @"A custom provider or gateway cannot use a built-in provider ID."
            );
        }
        assert_eq!(
            error.error.data,
            Some(json!({"config_write_error_code": error_code}))
        );
    }
    let persisted: Value = toml::from_str(&std::fs::read_to_string(&config_path)?)?;
    assert_eq!(persisted, expected);
    Ok(())
}

#[tokio::test]
async fn custom_provider_writes_reject_builtin_auth_names_and_preserve_existing_ids() -> Result<()>
{
    let home = TempDir::new()?;
    let config_path = home.path().join("config.toml");
    let ids = [r"team\gpu", "corp.gateway", r#"team"gpu"#];
    let mut providers = json!({});
    for id in ids {
        providers[id] = json!({"name":"Amazon Bedrock", "base_url":"https://old.example.test"});
    }
    std::fs::write(
        &config_path,
        toml::to_string(&json!({"model_providers":providers}))?,
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(CLEARED_AUTH_ENV)
        .build_initialized()
        .await?;

    let new_url = "https://new.example.test";
    for (provider_id, mut settings) in [
        (None, json!({"name": "OpenAI", "baseUrl": new_url})),
        (
            None,
            json!({"name": " Amazon Bedrock ", "baseUrl": new_url}),
        ),
        (Some(ids[0]), json!({"name": "Amazon Bedrock Runtime"})),
        (Some(ids[0]), json!({"name": "Google Cloud Vertex AI"})),
        (Some(ids[0]), json!({"baseUrl": new_url})),
        (Some(ids[0]), json!({"name":"Renamed", "envKey":null})),
    ] {
        settings["type"] = json!("customGateway");
        settings["providerId"] = json!(provider_id);
        let request = server
            .send_request(
                "modelProvider/config/write",
                Some(json!({"settings":settings})),
            )
            .await?;
        let error = server
            .read_stream_until_error_message(RequestId::Integer(request))
            .await?;
        assert_eq!(
            error.error.data,
            Some(json!({"config_write_error_code":"configValidationError"}))
        );
    }

    for id in ids {
        let request = server
            .send_request(
                "modelProvider/config/write",
                Some(json!({"settings":{"type":"customGateway","providerId":id,"name":"Amazon Bedrock Mantle","baseUrl":new_url}})),
            )
            .await?;
        let saved: Value = server.read_response(request).await?;
        assert_eq!(saved["provider"]["id"], id);
        providers[id] = json!({"name": "Amazon Bedrock Mantle", "base_url": new_url});
    }
    let saved: Value = toml::from_str(&std::fs::read_to_string(config_path)?)?;
    assert_eq!(saved["model_providers"], providers);
    Ok(())
}

#[tokio::test]
async fn builtin_provider_patches_preserve_auth_and_do_not_select_or_authenticate() -> Result<()> {
    let home = TempDir::new()?;
    let config_path = home.path().join("config.toml");
    std::fs::write(
        home.path().join("requirements.toml"),
        "model_provider = 'openai'\n",
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(CLEARED_AUTH_ENV)
        .build_initialized()
        .await?;
    let credential_file = home.path().join("missing-credentials.json");
    for mut settings in [
        json!({"type":"bedrock", "providerId":"amazon-bedrock", "profile":"work", "region":"us-east-1"}),
        json!({"type":"bedrock", "providerId":"amazon-bedrock-runtime", "profile":null, "region":"us-west-2"}),
        json!({"type":"gcp", "providerId":"gcp", "projectId":"test-project", "location":"global", "credentialFile":credential_file}),
    ] {
        let params = serde_json::from_value(json!({"settings":settings}))?;
        let saved: Value = server
            .request(|request_id| ClientRequest::ModelProviderConfigWrite { request_id, params })
            .await?;
        let id = settings
            .as_object_mut()
            .unwrap()
            .remove("providerId")
            .unwrap();
        assert_eq!(saved["provider"]["id"], id);
        assert_eq!(saved["provider"]["settings"], settings);
    }
    let refresh = json!({"command":"aws", "args":["sso", "login"]});
    let export = json!({"command":"unused-credential-command", "args":["test-only"]});
    let mut configured: Value = toml::from_str(&std::fs::read_to_string(&config_path)?)?;
    configured["model_providers"]["amazon-bedrock"]["aws"]["auth_refresh"] = refresh.clone();
    configured["model_providers"]["amazon-bedrock-runtime"]["aws"]["credential_export"] =
        export.clone();
    std::fs::write(&config_path, toml::to_string(&configured)?)?;
    for settings in [
        json!({"type":"bedrock", "providerId":"amazon-bedrock", "region":null}),
        json!({"type":"bedrock", "providerId":"amazon-bedrock-runtime", "region":"eu-west-1"}),
        json!({"type":"gcp", "providerId":"gcp", "location":"us-central1", "credentialFile":null}),
        json!({"type":"gcp", "providerId":"gcp"}),
    ] {
        let params = serde_json::from_value(json!({"settings":settings}))?;
        let saved: Value = server
            .request(|request_id| ClientRequest::ModelProviderConfigWrite { request_id, params })
            .await?;
        assert_eq!(saved["provider"]["id"], settings["providerId"]);
    }
    let list: Value = server
        .request(|request_id| ClientRequest::ModelProviderList {
            request_id,
            params: Default::default(),
        })
        .await?;
    assert_eq!(list["defaultProviderId"], "openai");
    assert_eq!(list["canChangeDefault"], false);
    let before_rejections = std::fs::read_to_string(&config_path)?;
    let actual: Value = toml::from_str(&before_rejections)?;
    assert_eq!(
        actual,
        json!({"model_providers": {
            "amazon-bedrock": {"aws":{"profile":"work", "auth_refresh":refresh}},
            "amazon-bedrock-runtime": {"aws":{"region":"eu-west-1", "credential_export":export}},
            "gcp":{"project_id":"test-project", "location":"us-central1"}
        }})
    );
    for settings in [
        json!({"type":"bedrock", "providerId":"amazon-bedrock-runtime", "profile":"conflicts-with-command", "region":"must-not-save"}),
        json!({"type":"gcp", "providerId":"gcp", "credentialFile":"relative/path", "location":"must-not-save"}),
    ] {
        let write = server
            .send_request(
                "modelProvider/config/write",
                Some(json!({"settings":settings})),
            )
            .await?;
        let error = server
            .read_stream_until_error_message(RequestId::Integer(write))
            .await?;
        assert_eq!(
            error.error.data,
            Some(json!({"config_write_error_code":"configValidationError"}))
        );
        assert_eq!(std::fs::read_to_string(&config_path)?, before_rejections);
    }
    for settings in [
        json!({"type":"bedrock", "providerId":"gcp", "region":"must-not-save"}),
        json!({"type":"gcp", "providerId":"amazon-bedrock", "location":"must-not-save"}),
        json!({"type":"bedrock", "providerId":"amazon-bedrock", "envKey":"UNSUPPORTED"}),
        json!({"type":"gcp", "providerId":"gcp", "profile":"wrong-provider-field"}),
        json!({"type":"gcp", "location":"missing-id"}),
    ] {
        let write = server
            .send_request(
                "modelProvider/config/write",
                Some(json!({"settings":settings})),
            )
            .await?;
        let error = server
            .read_stream_until_error_message(RequestId::Integer(write))
            .await?;
        assert_eq!(error.error.code, -32600);
    }
    assert_eq!(std::fs::read_to_string(&config_path)?, before_rejections);
    Ok(())
}
