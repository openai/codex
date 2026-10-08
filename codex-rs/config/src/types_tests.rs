use super::*;
use pretty_assertions::assert_eq;

#[test]
fn plugin_activation_inherits_defaults_without_explicit_enablement() {
    for default_enabled in [None, Some(false), Some(true)] {
        let default_config = default_enabled.map_or_else(String::new, |enabled| {
            format!("[_default]\nenabled = {enabled}\n")
        });
        let config: PluginsConfigToml = toml::from_str(&format!(
            r#"{default_config}
["empty@market"]
["mcp@market".mcp_servers.example]
enabled = false
["approval@market".mcp_servers.example]
default_tools_approval_mode = "approve"
["enabled@market"]
enabled = true
["disabled@market"]
enabled = false
"#
        ))
        .expect("plugin activation config");
        assert_eq!(
            [
                "empty@market",
                "mcp@market",
                "approval@market",
                "enabled@market",
                "disabled@market"
            ]
            .map(|plugin_id| config.plugins[plugin_id].enabled),
            [None, None, None, Some(true), Some(false)],
        );
        assert_eq!(
            [
                "missing@market",
                "empty@market",
                "mcp@market",
                "approval@market",
                "enabled@market",
                "disabled@market"
            ]
            .map(|plugin_id| config.allows_plugin(plugin_id)),
            match default_enabled {
                None => [true, true, true, true, true, true],
                Some(false) => [false, false, false, false, true, false],
                Some(true) => [true, true, true, true, true, false],
            },
        );
        let round_trip: PluginsConfigToml =
            toml::from_str(&toml::to_string(&config).expect("serialize plugin config"))
                .expect("deserialize plugin config");
        assert_eq!(round_trip, config);
    }
}

#[test]
fn mouse_scroll_speed_accepts_integer_and_fractional_multipliers() {
    for (value, expected) in [("1", 1.0), ("0.5", 0.5), ("3.0", 3.0)] {
        let tui: Tui = toml::from_str(&format!("mouse_scroll_speed = {value}")).unwrap();
        assert_eq!(tui.mouse_scroll_speed, Some(expected));
    }
}

#[test]
fn mouse_scroll_speed_rejects_nonpositive_and_nonfinite_values() {
    for value in ["0", "-0.5", "nan", "inf", "-inf"] {
        let error = toml::from_str::<Tui>(&format!("mouse_scroll_speed = {value}")).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("tui.mouse_scroll_speed must be a finite positive number"),
            "{error}"
        );
    }
}

#[test]
fn deserialize_skill_config_with_name_selector() {
    let cfg: SkillConfig = toml::from_str(
        r#"
            name = "github:yeet"
            enabled = false
        "#,
    )
    .expect("should deserialize skill config with name selector");

    assert_eq!(cfg.name.as_deref(), Some("github:yeet"));
    assert_eq!(cfg.path, None);
    assert!(!cfg.enabled);
}

#[test]
fn deserialize_skill_config_with_path_selector() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let skill_path = tempdir.path().join("skills").join("demo").join("SKILL.md");
    let cfg: SkillConfig = toml::from_str(&format!(
        r#"
            path = {path:?}
            enabled = false
        "#,
        path = skill_path.display().to_string(),
    ))
    .expect("should deserialize skill config with path selector");

    assert_eq!(
        cfg,
        SkillConfig {
            path: Some(
                AbsolutePathBuf::from_absolute_path(&skill_path)
                    .expect("skill path should be absolute"),
            ),
            name: None,
            enabled: false,
        }
    );
}

#[test]
fn memories_config_clamps_count_limits_to_nonzero_values() {
    let config = MemoriesConfig::from(MemoriesToml {
        max_raw_memories_for_consolidation: Some(0),
        max_rollouts_per_startup: Some(0),
        ..Default::default()
    });

    assert_eq!(
        config,
        MemoriesConfig {
            max_raw_memories_for_consolidation: 1,
            max_rollouts_per_startup: 1,
            ..MemoriesConfig::default()
        }
    );
}

#[test]
fn memories_config_clamps_rate_limit_remaining_threshold() {
    let config = MemoriesConfig::from(MemoriesToml {
        min_rate_limit_remaining_percent: Some(101),
        ..Default::default()
    });
    assert_eq!(
        config,
        MemoriesConfig {
            min_rate_limit_remaining_percent: 100,
            ..MemoriesConfig::default()
        }
    );

    let config = MemoriesConfig::from(MemoriesToml {
        min_rate_limit_remaining_percent: Some(-1),
        ..Default::default()
    });
    assert_eq!(
        config,
        MemoriesConfig {
            min_rate_limit_remaining_percent: 0,
            ..MemoriesConfig::default()
        }
    );
}

#[test]
fn memories_version_selects_pipeline_without_changing_other_defaults() {
    for (source, version) in [
        ("", MemoryVersion::V1),
        ("version = \"v2\"", MemoryVersion::V2),
    ] {
        let parsed: MemoriesToml = toml::from_str(source).expect("parse memories config");
        assert_eq!(
            MemoriesConfig::from(parsed),
            MemoriesConfig {
                version,
                ..Default::default()
            }
        );
    }
    assert!(toml::from_str::<MemoriesToml>("version = \"v3\"").is_err());
}

#[test]
fn rendering_preferences_default_individually_and_ignore_animation_switch() {
    for key in ["mermaid", "math", "tables", "lists"] {
        let tui: Tui =
            toml::from_str(&format!("animations = false\n[rendering]\n{key} = false\n")).unwrap();
        assert_eq!(
            tui.rendering,
            TuiRendering {
                mermaid: key != "mermaid",
                math: key != "math",
                tables: key != "tables",
                lists: key != "lists",
            }
        );
    }
}
