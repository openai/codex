use super::super::ThreadParamsMode;
use super::super::config_request_overrides_from_config;
use crate::legacy_core::config::ConfigBuilder;
use color_eyre::Result;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn legacy_search_overrides_respect_launch_origins() -> Result<()> {
    let home = tempfile::tempdir()?;
    for (settings, flags, expected) in [
        (
            "web_search = 'disabled'",
            "web_search_request = true\nweb_search_cached = true\nweb_search = true",
            None,
        ),
        ("[features]\nweb_search_request = true", "", None),
        (
            "[features]\nweb_search_cached = true\nweb_search_request = true",
            "web_search_cached = false",
            Some("live"),
        ),
    ] {
        std::fs::write(home.path().join("config.toml"), settings)?;
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .cli_overrides(
                toml::from_str::<toml::Table>(&format!("[features]\nmulti_agent = true\n{flags}"))?
                    .into_iter()
                    .collect(),
            )
            .loader_overrides(codex_config::LoaderOverrides::without_managed_config_for_tests())
            .build()
            .await?;
        let overrides = config_request_overrides_from_config(&config, ThreadParamsMode::Remote)
            .expect("config overrides");
        assert_eq!(
            overrides
                .get("web_search")
                .and_then(serde_json::Value::as_str),
            expected,
        );
        assert_eq!(
            overrides.get("features"),
            Some(&serde_json::json!({"multi_agent": true})),
        );
    }
    Ok(())
}

#[tokio::test]
async fn web_search_overrides_preserve_explicit_legacy_choices() -> Result<()> {
    let home = tempfile::tempdir()?;
    for (settings, expected) in [
        ("[features]\nweb_search_request = true", "live"),
        (
            "[features]\nweb_search_cached = true\nweb_search_request = true",
            "cached",
        ),
        ("[features]\nweb_search = true", "live"),
        (
            "[features]\nweb_search = true\nweb_search_request = false",
            "cached",
        ),
        (
            "web_search = 'disabled'\n[features]\nweb_search_request = true",
            "disabled",
        ),
    ] {
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .cli_overrides(
                toml::from_str::<toml::Table>(settings)?
                    .into_iter()
                    .collect(),
            )
            .loader_overrides(codex_config::LoaderOverrides::without_managed_config_for_tests())
            .build()
            .await?;
        assert_eq!(
            config_request_overrides_from_config(&config, ThreadParamsMode::Remote),
            Some(std::collections::HashMap::from([(
                "web_search".to_string(),
                serde_json::json!(expected),
            )])),
            "{settings}",
        );
    }
    Ok(())
}

#[tokio::test]
async fn embedded_legacy_search_honors_shared_feature_requirements() -> Result<()> {
    let home = tempfile::tempdir()?;
    let requirements_path = home.path().join("requirements.toml");
    for (requirements, flags, embedded_mode, remote_mode) in [
        (
            "web_search_request = false",
            "web_search_request = true",
            None,
            "live",
        ),
        (
            "web_search_cached = true",
            "web_search_request = true",
            None,
            "live",
        ),
        (
            "web_search_request = true",
            "web_search_request = false",
            None,
            "cached",
        ),
        (
            "web_search_cached = false",
            "web_search_cached = true\nweb_search_request = true",
            Some("live"),
            "cached",
        ),
    ] {
        std::fs::write(&requirements_path, format!("[features]\n{requirements}"))?;
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .cli_overrides(
                toml::from_str::<toml::Table>(&format!("[features]\n{flags}"))?
                    .into_iter()
                    .collect(),
            )
            .loader_overrides(codex_config::LoaderOverrides {
                system_requirements_path: Some(requirements_path.clone()),
                ..codex_config::LoaderOverrides::without_managed_config_for_tests()
            })
            .build()
            .await?;
        for (mode, expected) in [
            (ThreadParamsMode::Embedded, embedded_mode),
            (ThreadParamsMode::Remote, Some(remote_mode)),
        ] {
            let overrides =
                config_request_overrides_from_config(&config, mode).expect("config overrides");
            assert_eq!(
                overrides
                    .get("web_search")
                    .and_then(serde_json::Value::as_str),
                expected
            );
        }
    }
    Ok(())
}
