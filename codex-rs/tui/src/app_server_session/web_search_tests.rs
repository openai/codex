use super::super::ThreadParamsMode;
use super::super::config_request_overrides_from_config;
use crate::legacy_core::config::ConfigBuilder;
use codex_protocol::config_types::WebSearchMode;
use codex_utils_absolute_path::AbsolutePathBuf;
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

#[tokio::test]
async fn legacy_search_disables_keep_indexed_default_implicit() -> Result<()> {
    let client_home = tempfile::tempdir()?;
    let server_home = tempfile::tempdir()?;
    let profile = AbsolutePathBuf::from_absolute_path(client_home.path().join("work.config.toml"))?;
    for (selected_profile, enabled) in [(false, false), (false, true), (true, false), (true, true)]
    {
        for flags in [
            "",
            "web_search = false",
            "web_search_cached = false",
            "web_search_request = false",
        ] {
            let settings = format!("[features]\nweb_search_indexed_default = {enabled}\n{flags}");
            std::fs::write(&profile, &settings)?;
            let config = ConfigBuilder::default()
                .codex_home(client_home.path().to_path_buf())
                .cli_overrides(if selected_profile {
                    Vec::new()
                } else {
                    toml::from_str::<toml::Table>(&settings)?
                        .into_iter()
                        .collect()
                })
                .loader_overrides(codex_config::LoaderOverrides {
                    user_config_path: selected_profile.then(|| profile.clone()),
                    user_config_profile: selected_profile.then(|| "work".parse()).transpose()?,
                    ..codex_config::LoaderOverrides::without_managed_config_for_tests()
                })
                .build()
                .await?;
            for mode in [ThreadParamsMode::Embedded, ThreadParamsMode::Remote] {
                let overrides =
                    config_request_overrides_from_config(&config, mode).expect("config overrides");
                let forwarded = ConfigBuilder::default()
                    .codex_home(server_home.path().to_path_buf())
                    .cli_overrides(
                        overrides
                            .into_iter()
                            .map(|(key, value)| Ok((key, toml::Value::try_from(value)?)))
                            .collect::<Result<_>>()?,
                    )
                    .loader_overrides(
                        codex_config::LoaderOverrides::without_managed_config_for_tests(),
                    )
                    .build()
                    .await?;
                assert_eq!(forwarded.web_search_mode, config.web_search_mode);
                assert_eq!(
                    forwarded.web_search_mode_was_explicit,
                    !enabled && !flags.is_empty()
                );
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn local_indexed_default_preserves_remote_legacy_search_disable() -> Result<()> {
    let client_home = tempfile::tempdir()?;
    let server_home = tempfile::tempdir()?;
    std::fs::write(
        client_home.path().join("config.toml"),
        "[features]\nweb_search_indexed_default = true",
    )?;
    std::fs::write(
        server_home.path().join("config.toml"),
        "web_search = 'live'",
    )?;
    let config = ConfigBuilder::default()
        .codex_home(client_home.path().to_path_buf())
        .cli_overrides(vec![(
            "features.web_search_request".to_string(),
            toml::Value::Boolean(false),
        )])
        .loader_overrides(codex_config::LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await?;
    let overrides = config_request_overrides_from_config(&config, ThreadParamsMode::Remote)
        .expect("config overrides");
    assert_eq!(
        overrides,
        std::collections::HashMap::from([("web_search".to_string(), serde_json::json!("cached"))]),
    );
    let forwarded = ConfigBuilder::default()
        .codex_home(server_home.path().to_path_buf())
        .cli_overrides(
            overrides
                .into_iter()
                .map(|(key, value)| Ok((key, toml::Value::try_from(value)?)))
                .collect::<Result<_>>()?,
        )
        .loader_overrides(codex_config::LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await?;
    assert_eq!(forwarded.web_search_mode.value(), WebSearchMode::Cached);
    assert!(forwarded.web_search_mode_was_explicit);
    Ok(())
}

#[tokio::test]
async fn legacy_search_disables_preserve_server_policy_with_indexed_default() -> Result<()> {
    let client_home = tempfile::tempdir()?;
    let server_home = tempfile::tempdir()?;
    let requirements_path = server_home.path().join("requirements.toml");
    for enabled in [false, true] {
        let config = ConfigBuilder::default()
            .codex_home(client_home.path().to_path_buf())
            .cli_overrides(
                toml::from_str::<toml::Table>(&format!(
                    "[features]\nweb_search_indexed_default = {enabled}\nweb_search_request = false"
                ))?
                .into_iter()
                .collect(),
            )
            .loader_overrides(codex_config::LoaderOverrides::without_managed_config_for_tests())
            .build()
            .await?;
        let overrides = config_request_overrides_from_config(&config, ThreadParamsMode::Remote)
            .expect("config overrides");
        for (settings, requirements, expected, explicit) in [
            ("web_search = 'live'", "", WebSearchMode::Live, true),
            ("web_search = 'indexed'", "", WebSearchMode::Indexed, true),
            ("web_search = 'disabled'", "", WebSearchMode::Disabled, true),
            (
                "",
                "[features]\nweb_search_request = true",
                WebSearchMode::Live,
                false,
            ),
            (
                "",
                "[features]\nweb_search_cached = true",
                WebSearchMode::Cached,
                false,
            ),
            (
                "",
                "allowed_web_search_modes = ['cached']",
                WebSearchMode::Cached,
                false,
            ),
        ] {
            std::fs::write(server_home.path().join("config.toml"), settings)?;
            std::fs::write(&requirements_path, requirements)?;
            let forwarded = ConfigBuilder::default()
                .codex_home(server_home.path().to_path_buf())
                .cli_overrides(
                    overrides
                        .iter()
                        .map(|(key, value)| Ok((key.clone(), toml::Value::try_from(value)?)))
                        .collect::<Result<_>>()?,
                )
                .loader_overrides(codex_config::LoaderOverrides {
                    system_requirements_path: Some(requirements_path.clone()),
                    ..codex_config::LoaderOverrides::without_managed_config_for_tests()
                })
                .build()
                .await?;
            assert_eq!(
                forwarded.web_search_mode.value(),
                if enabled {
                    expected
                } else {
                    WebSearchMode::Cached
                },
            );
            assert_eq!(forwarded.web_search_mode_was_explicit, !enabled || explicit);
        }
    }
    Ok(())
}
