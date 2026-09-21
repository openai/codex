//! Verifies managed curated Git restrictions through plugin and skill loading.

use std::fs;

use crate::PluginsConfigInput;
use crate::PluginsManager;
use crate::test_support::test_skill_root_loader;
use anyhow::Result;
use codex_config::LoaderOverrides;
use codex_config::NoopThreadConfigLoader;
use codex_config::loader::load_config_layers_state;
use codex_exec_server::LOCAL_FS;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::CodexAuth;
use codex_login::test_support::auth_manager_from_optional_auth;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_skills::LoadedSkills;
use codex_skills::SkillMetadata;
use codex_skills::SkillRootLoadRequest;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[tokio::test]
async fn curated_git_requirements_control_plugin_skills() -> Result<()> {
    for (name, allow_curated) in [
        ("openai-curated", true),
        ("openai-curated", false),
        ("openai-api-curated", true),
        ("openai-api-curated", false),
        ("openai-bundled", false),
    ] {
        let home = TempDir::new()?;
        let plugin_id = format!("sample@{name}");
        let root = home
            .path()
            .join(format!("plugins/cache/{name}/sample/local"));
        fs::create_dir_all(root.join(".codex-plugin"))?;
        fs::write(
            root.join(".codex-plugin/plugin.json"),
            r#"{"name":"sample","description":"inspect sample data"}"#,
        )?;
        let skill_dir = root.join("skills/sample-search");
        fs::create_dir_all(&skill_dir)?;
        let skill_path = skill_dir.join("SKILL.md");
        fs::write(
            &skill_path,
            "---\ndescription: inspect sample data\n---\n\n# body\n",
        )?;
        let mut user_config = format!("[plugins.\"{plugin_id}\"]\nenabled = true\n");
        if name == "openai-bundled" {
            let bundled = home.path().join(".tmp/bundled-marketplaces/openai-bundled");
            user_config.push_str(&format!(
                "\n[marketplaces.openai-bundled]\nsource_type = \"local\"\nsource = {bundled:?}\n"
            ));
        }
        fs::write(home.path().join("config.toml"), user_config)?;
        let requirements_path = home.path().join("requirements.toml");
        let rule = if allow_curated {
            "[marketplaces.allowed_sources.curated]\nsource = 'git'\nurl = 'https://github.com/openai/plugins.git'\n"
        } else {
            ""
        };
        fs::write(
            &requirements_path,
            format!("[marketplaces]\nrestrict_to_allowed_sources = true\n{rule}"),
        )?;
        let home_path = AbsolutePathBuf::try_from(home.path())?;
        let config_layer_stack = load_config_layers_state(
            LOCAL_FS.as_ref(),
            home.path(),
            Some(home_path),
            &[],
            LoaderOverrides {
                system_requirements_path: Some(requirements_path),
                ..LoaderOverrides::without_managed_config_for_tests()
            },
            &NoopThreadConfigLoader,
        )
        .await?;
        let config = PluginsConfigInput::new(
            config_layer_stack,
            "openai".to_string(),
            /*plugins_enabled*/ true,
            /*remote_plugin_enabled*/ false,
            "https://chatgpt.com/backend-api/".to_string(),
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            /*product_sku*/ None,
        );
        let auth = if name == "openai-curated" {
            CodexAuth::create_dummy_chatgpt_auth_for_testing()
        } else {
            CodexAuth::from_api_key("test-api-key")
        };
        let skills = test_skill_root_loader();
        let manager = PluginsManager::new(
            home.path().to_path_buf(),
            auth_manager_from_optional_auth(Some(auth)),
            skills.clone(),
        );
        let plugins = manager.plugins_for_config(&config).await;
        let loaded = skills
            .load_roots(SkillRootLoadRequest {
                roots: plugins.effective_plugin_skill_roots(),
                restriction_product: Some(Product::Codex),
                snapshots: manager.plugin_skill_snapshots_for_config(&config),
            })
            .await;
        let expected = if allow_curated || name == "openai-bundled" {
            LoadedSkills {
                skills: vec![SkillMetadata {
                    name: "sample:sample-search".to_string(),
                    description: "inspect sample data".to_string(),
                    short_description: None,
                    interface: None,
                    dependencies: None,
                    policy: None,
                    path_to_skills_md: AbsolutePathBuf::try_from(fs::canonicalize(skill_path)?)?,
                    scope: SkillScope::User,
                    plugin_id: Some(plugin_id),
                    remote_plugin_id: None,
                }],
                errors: Vec::new(),
            }
        } else {
            LoadedSkills::default()
        };
        assert_eq!(
            loaded, expected,
            "marketplace={name}, allow_curated={allow_curated}"
        );
    }
    Ok(())
}
