use std::collections::BTreeMap;
use std::collections::HashMap;

use codex_config::AbsolutePathBuf;
use codex_config::AppRequirementToml;
use codex_config::AppToolRequirementToml;
use codex_config::AppToolsRequirementsToml;
use codex_config::AppsRequirementsToml;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_config::TomlValue;
use codex_config::types::AppConfig;
use codex_config::types::AppToolApproval;
use codex_config::types::AppToolConfig;
use codex_config::types::AppToolsConfig;
use codex_config::types::AppsConfigToml;
use codex_config::types::AppsDefaultConfig;
use pretty_assertions::assert_eq;

use super::*;

#[test]
fn evaluator_reuses_one_snapshot_across_tools() {
    let apps_config = AppsConfigToml {
        default: None,
        apps: HashMap::from([(
            "calendar".to_string(),
            AppConfig {
                enabled: true,
                default_tools_enabled: Some(false),
                tools: Some(AppToolsConfig {
                    tools: HashMap::from([(
                        "events/create".to_string(),
                        AppToolConfig {
                            enabled: Some(true),
                            approval_mode: Some(AppToolApproval::Prompt),
                        },
                    )]),
                }),
                ..Default::default()
            },
        )]),
    };
    let requirements = AppsRequirementsToml {
        apps: BTreeMap::from([(
            "calendar".to_string(),
            AppRequirementToml {
                enabled: None,
                tools: Some(AppToolsRequirementsToml {
                    tools: BTreeMap::from([(
                        "events/create".to_string(),
                        AppToolRequirementToml {
                            approval_mode: Some(AppToolApproval::Approve),
                        },
                    )]),
                }),
            },
        )]),
    };
    let evaluator = AppToolPolicyEvaluator::from_parts(Some(apps_config), Some(&requirements));

    assert_eq!(
        [
            evaluator.policy(input("events/create", /*tool_title*/ None)),
            evaluator.policy(input("events/list", /*tool_title*/ None)),
            evaluator.policy(input("calendar_events/create", Some("events/create"))),
        ],
        [
            AppToolPolicy {
                enabled: true,
                approval: AppToolApproval::Approve,
            },
            AppToolPolicy {
                enabled: false,
                approval: AppToolApproval::Auto,
            },
            AppToolPolicy {
                enabled: true,
                approval: AppToolApproval::Prompt,
            },
        ]
    );
}

#[test]
fn evaluator_uses_global_defaults_for_destructive_hints() {
    let apps_config = AppsConfigToml {
        default: Some(defaults(
            /*enabled*/ true, /*destructive_enabled*/ false,
            /*open_world_enabled*/ true,
        )),
        apps: HashMap::new(),
    };

    assert_eq!(
        policy_from_apps_config(
            Some(&apps_config),
            Some("calendar"),
            "events/create",
            /*tool_title*/ None,
            Some(true),
            /*open_world_hint*/ None,
            /*managed_approval*/ None,
        ),
        AppToolPolicy {
            enabled: false,
            approval: AppToolApproval::Auto,
        }
    );
}

#[test]
fn evaluator_defaults_missing_destructive_hint_to_true() {
    let apps_config = AppsConfigToml {
        default: Some(defaults(
            /*enabled*/ true, /*destructive_enabled*/ false,
            /*open_world_enabled*/ true,
        )),
        apps: HashMap::new(),
    };

    assert_eq!(
        policy_from_apps_config(
            Some(&apps_config),
            Some("calendar"),
            "events/create",
            /*tool_title*/ None,
            /*destructive_hint*/ None,
            Some(false),
            /*managed_approval*/ None,
        ),
        AppToolPolicy {
            enabled: false,
            approval: AppToolApproval::Auto,
        }
    );
}

#[test]
fn evaluator_defaults_missing_open_world_hint_to_true() {
    let apps_config = AppsConfigToml {
        default: Some(defaults(
            /*enabled*/ true, /*destructive_enabled*/ true,
            /*open_world_enabled*/ false,
        )),
        apps: HashMap::new(),
    };

    assert_eq!(
        policy_from_apps_config(
            Some(&apps_config),
            Some("calendar"),
            "events/create",
            /*tool_title*/ None,
            Some(false),
            /*open_world_hint*/ None,
            /*managed_approval*/ None,
        ),
        AppToolPolicy {
            enabled: false,
            approval: AppToolApproval::Auto,
        }
    );
}

#[test]
fn app_enablement_uses_defaults_and_per_app_overrides() {
    let apps_config = AppsConfigToml {
        default: Some(defaults(
            /*enabled*/ false, /*destructive_enabled*/ true,
            /*open_world_enabled*/ true,
        )),
        apps: HashMap::from([(
            "calendar".to_string(),
            AppConfig {
                enabled: true,
                ..Default::default()
            },
        )]),
    };

    assert_eq!(
        [
            app_is_enabled(&apps_config, Some("calendar")),
            app_is_enabled(&apps_config, Some("drive")),
            app_is_enabled(&apps_config, /*connector_id*/ None),
        ],
        [true, false, false]
    );
}

#[test]
fn managed_disable_overrides_enabled_app() {
    let apps_config = AppsConfigToml {
        default: None,
        apps: HashMap::from([(
            "connector_123123".to_string(),
            AppConfig {
                enabled: true,
                ..Default::default()
            },
        )]),
    };
    let requirements = app_enabled_requirement("connector_123123", /*enabled*/ false);

    assert_eq!(
        policy_from_config_parts(
            Some(&apps_config),
            Some(&requirements),
            Some("connector_123123"),
            "events/list",
            /*tool_title*/ None,
            /*destructive_hint*/ None,
            /*open_world_hint*/ None,
        ),
        AppToolPolicy {
            enabled: false,
            approval: AppToolApproval::Auto,
        }
    );
}

#[test]
fn managed_enable_does_not_override_disabled_app() {
    let apps_config = AppsConfigToml {
        default: None,
        apps: HashMap::from([(
            "connector_123123".to_string(),
            AppConfig {
                enabled: false,
                ..Default::default()
            },
        )]),
    };
    let requirements = app_enabled_requirement("connector_123123", /*enabled*/ true);

    assert_eq!(
        policy_from_config_parts(
            Some(&apps_config),
            Some(&requirements),
            Some("connector_123123"),
            "events/list",
            /*tool_title*/ None,
            /*destructive_hint*/ None,
            /*open_world_hint*/ None,
        ),
        AppToolPolicy {
            enabled: false,
            approval: AppToolApproval::Auto,
        }
    );
}

#[test]
fn managed_disable_applies_without_apps_config() {
    let requirements = app_enabled_requirement("connector_123123", /*enabled*/ false);

    assert_eq!(
        policy_from_config_parts(
            /*apps_config*/ None,
            Some(&requirements),
            Some("connector_123123"),
            "events/list",
            /*tool_title*/ None,
            /*destructive_hint*/ None,
            /*open_world_hint*/ None,
        ),
        AppToolPolicy {
            enabled: false,
            approval: AppToolApproval::Auto,
        }
    );
}

#[test]
fn evaluator_honors_default_app_enabled_false() {
    let apps_config = AppsConfigToml {
        default: Some(defaults(
            /*enabled*/ false, /*destructive_enabled*/ true,
            /*open_world_enabled*/ true,
        )),
        apps: HashMap::new(),
    };

    assert_eq!(
        policy_from_apps_config(
            Some(&apps_config),
            Some("calendar"),
            "events/list",
            /*tool_title*/ None,
            /*destructive_hint*/ None,
            /*open_world_hint*/ None,
            /*managed_approval*/ None,
        ),
        AppToolPolicy {
            enabled: false,
            approval: AppToolApproval::Auto,
        }
    );
}

#[test]
fn evaluator_allows_per_app_enable_when_default_is_disabled() {
    let apps_config = AppsConfigToml {
        default: Some(defaults(
            /*enabled*/ false, /*destructive_enabled*/ true,
            /*open_world_enabled*/ true,
        )),
        apps: HashMap::from([(
            "calendar".to_string(),
            AppConfig {
                enabled: true,
                ..Default::default()
            },
        )]),
    };

    assert_eq!(
        policy_from_apps_config(
            Some(&apps_config),
            Some("calendar"),
            "events/list",
            /*tool_title*/ None,
            /*destructive_hint*/ None,
            /*open_world_hint*/ None,
            /*managed_approval*/ None,
        ),
        AppToolPolicy::default()
    );
}

#[test]
fn evaluator_uses_managed_approval_without_apps_config() {
    assert_eq!(
        policy_from_apps_config(
            /*apps_config*/ None,
            Some("calendar"),
            "events/list",
            /*tool_title*/ None,
            /*destructive_hint*/ None,
            /*open_world_hint*/ None,
            Some(AppToolApproval::Approve),
        ),
        AppToolPolicy {
            enabled: true,
            approval: AppToolApproval::Approve,
        }
    );
}

#[test]
fn managed_approval_uses_raw_tool_name() {
    let requirements = app_tool_requirements(
        "connector_123123",
        "calendar/list_events",
        AppToolApproval::Approve,
    );

    assert_eq!(
        [
            policy_from_config_parts(
                /*apps_config*/ None,
                Some(&requirements),
                Some("connector_123123"),
                "calendar/list_events",
                /*tool_title*/ None,
                /*destructive_hint*/ None,
                /*open_world_hint*/ None,
            ),
            policy_from_config_parts(
                /*apps_config*/ None,
                Some(&requirements),
                Some("connector_123123"),
                "calendar/create_event",
                Some("calendar/list_events"),
                /*destructive_hint*/ None,
                /*open_world_hint*/ None,
            ),
        ],
        [
            AppToolPolicy {
                enabled: true,
                approval: AppToolApproval::Approve,
            },
            AppToolPolicy::default(),
        ]
    );
}

#[test]
fn managed_approval_overrides_user_tool_approval() {
    let apps_config = AppsConfigToml {
        default: None,
        apps: HashMap::from([(
            "connector_123123".to_string(),
            AppConfig {
                enabled: true,
                tools: Some(AppToolsConfig {
                    tools: HashMap::from([(
                        "calendar/list_events".to_string(),
                        AppToolConfig {
                            enabled: None,
                            approval_mode: Some(AppToolApproval::Prompt),
                        },
                    )]),
                }),
                ..Default::default()
            },
        )]),
    };
    let requirements = app_tool_requirements(
        "connector_123123",
        "calendar/list_events",
        AppToolApproval::Approve,
    );

    assert_eq!(
        policy_from_config_parts(
            Some(&apps_config),
            Some(&requirements),
            Some("connector_123123"),
            "calendar/list_events",
            /*tool_title*/ None,
            /*destructive_hint*/ None,
            /*open_world_hint*/ None,
        ),
        AppToolPolicy {
            enabled: true,
            approval: AppToolApproval::Approve,
        }
    );
}

#[test]
fn per_tool_enable_overrides_app_level_hints() {
    let apps_config = AppsConfigToml {
        default: None,
        apps: HashMap::from([(
            "calendar".to_string(),
            AppConfig {
                enabled: true,
                destructive_enabled: Some(false),
                open_world_enabled: Some(false),
                tools: Some(AppToolsConfig {
                    tools: HashMap::from([(
                        "events/create".to_string(),
                        AppToolConfig {
                            enabled: Some(true),
                            approval_mode: None,
                        },
                    )]),
                }),
                ..Default::default()
            },
        )]),
    };

    assert_eq!(
        policy_from_apps_config(
            Some(&apps_config),
            Some("calendar"),
            "events/create",
            /*tool_title*/ None,
            Some(true),
            Some(true),
            /*managed_approval*/ None,
        ),
        AppToolPolicy::default()
    );
}

#[test]
fn default_tools_enable_overrides_app_level_hints() {
    let mut app = AppConfig {
        enabled: true,
        destructive_enabled: Some(false),
        open_world_enabled: Some(false),
        default_tools_enabled: Some(true),
        ..Default::default()
    };
    let apps_config = |app: AppConfig| AppsConfigToml {
        default: None,
        apps: HashMap::from([("calendar".to_string(), app)]),
    };

    let enabled_policy = policy_from_apps_config(
        Some(&apps_config(app.clone())),
        Some("calendar"),
        "events/create",
        /*tool_title*/ None,
        Some(true),
        Some(true),
        /*managed_approval*/ None,
    );
    app.destructive_enabled = Some(true);
    app.open_world_enabled = Some(true);
    app.default_tools_enabled = Some(false);
    app.default_tools_approval_mode = Some(AppToolApproval::Approve);
    let disabled_policy = policy_from_apps_config(
        Some(&apps_config(app)),
        Some("calendar"),
        "events/list",
        /*tool_title*/ None,
        /*destructive_hint*/ None,
        /*open_world_hint*/ None,
        /*managed_approval*/ None,
    );

    assert_eq!(
        [enabled_policy, disabled_policy],
        [
            AppToolPolicy::default(),
            AppToolPolicy {
                enabled: false,
                approval: AppToolApproval::Approve,
            },
        ]
    );
}

#[test]
fn evaluator_uses_apps_default_tools_approval_mode() {
    let apps_config = AppsConfigToml {
        default: Some(AppsDefaultConfig {
            default_tools_approval_mode: Some(AppToolApproval::Prompt),
            ..defaults(
                /*enabled*/ true,
                /*destructive_enabled*/ true,
                /*open_world_enabled*/ true,
            )
        }),
        apps: HashMap::new(),
    };

    for connector_id in [Some("calendar"), None] {
        assert_eq!(
            policy_from_apps_config(
                Some(&apps_config),
                connector_id,
                "events/list",
                /*tool_title*/ None,
                /*destructive_hint*/ None,
                /*open_world_hint*/ None,
                /*managed_approval*/ None,
            ),
            AppToolPolicy {
                enabled: true,
                approval: AppToolApproval::Prompt,
            }
        );
    }
}

#[test]
fn evaluator_prefers_app_default_tools_approval_mode_over_apps_default() {
    let apps_config = AppsConfigToml {
        default: Some(AppsDefaultConfig {
            default_tools_approval_mode: Some(AppToolApproval::Approve),
            ..defaults(
                /*enabled*/ true,
                /*destructive_enabled*/ true,
                /*open_world_enabled*/ true,
            )
        }),
        apps: HashMap::from([(
            "calendar".to_string(),
            AppConfig {
                enabled: true,
                default_tools_approval_mode: Some(AppToolApproval::Prompt),
                tools: Some(AppToolsConfig {
                    tools: HashMap::new(),
                }),
                ..Default::default()
            },
        )]),
    };

    assert_eq!(
        policy_from_apps_config(
            Some(&apps_config),
            Some("calendar"),
            "events/list",
            /*tool_title*/ None,
            /*destructive_hint*/ None,
            /*open_world_hint*/ None,
            /*managed_approval*/ None,
        ),
        AppToolPolicy {
            enabled: true,
            approval: AppToolApproval::Prompt,
        }
    );
}

#[test]
fn evaluator_matches_tool_title_for_user_config() {
    let apps_config = AppsConfigToml {
        default: None,
        apps: HashMap::from([(
            "calendar".to_string(),
            AppConfig {
                enabled: true,
                destructive_enabled: Some(false),
                open_world_enabled: Some(false),
                default_tools_approval_mode: Some(AppToolApproval::Auto),
                default_tools_enabled: Some(false),
                tools: Some(AppToolsConfig {
                    tools: HashMap::from([(
                        "events/create".to_string(),
                        AppToolConfig {
                            enabled: Some(true),
                            approval_mode: Some(AppToolApproval::Approve),
                        },
                    )]),
                }),
                ..Default::default()
            },
        )]),
    };

    assert_eq!(
        policy_from_apps_config(
            Some(&apps_config),
            Some("calendar"),
            "calendar_events/create",
            Some("events/create"),
            Some(true),
            Some(true),
            /*managed_approval*/ None,
        ),
        AppToolPolicy {
            enabled: true,
            approval: AppToolApproval::Approve,
        }
    );
}

fn input<'a>(tool_name: &'a str, tool_title: Option<&'a str>) -> AppToolPolicyInput<'a> {
    AppToolPolicyInput {
        connector_id: Some("calendar"),
        tool_name,
        tool_title,
        destructive_hint: Some(true),
        open_world_hint: Some(true),
    }
}

fn policy_from_apps_config(
    apps_config: Option<&AppsConfigToml>,
    connector_id: Option<&str>,
    tool_name: &str,
    tool_title: Option<&str>,
    destructive_hint: Option<bool>,
    open_world_hint: Option<bool>,
    managed_approval: Option<AppToolApproval>,
) -> AppToolPolicy {
    let requirements = managed_approval.map(|approval| {
        app_tool_requirements(
            connector_id.expect("managed approval requires a connector id"),
            tool_name,
            approval,
        )
    });
    policy_from_config_parts(
        apps_config,
        requirements.as_ref(),
        connector_id,
        tool_name,
        tool_title,
        destructive_hint,
        open_world_hint,
    )
}

fn policy_from_config_parts(
    apps_config: Option<&AppsConfigToml>,
    requirements_apps_config: Option<&AppsRequirementsToml>,
    connector_id: Option<&str>,
    tool_name: &str,
    tool_title: Option<&str>,
    destructive_hint: Option<bool>,
    open_world_hint: Option<bool>,
) -> AppToolPolicy {
    let requirements = ConfigRequirementsToml {
        apps: requirements_apps_config.cloned(),
        ..Default::default()
    };
    let config_layer_stack =
        ConfigLayerStack::new(Vec::new(), ConfigRequirements::default(), requirements)
            .expect("config layer stack");
    let config_layer_stack = if let Some(apps_config) = apps_config {
        let mut user_config = TomlValue::Table(Default::default());
        user_config
            .as_table_mut()
            .expect("user config table")
            .insert(
                "apps".to_string(),
                TomlValue::try_from(apps_config).expect("serialize apps config"),
            );
        let config_toml_path =
            AbsolutePathBuf::try_from(std::env::temp_dir().join(CONFIG_TOML_FILE))
                .expect("absolute config path");
        config_layer_stack.with_user_config(&config_toml_path, user_config)
    } else {
        config_layer_stack
    };
    AppToolPolicyEvaluator::new(&config_layer_stack).policy(AppToolPolicyInput {
        connector_id,
        tool_name,
        tool_title,
        destructive_hint,
        open_world_hint,
    })
}

fn app_enabled_requirement(app_id: &str, enabled: bool) -> AppsRequirementsToml {
    AppsRequirementsToml {
        apps: BTreeMap::from([(
            app_id.to_string(),
            AppRequirementToml {
                enabled: Some(enabled),
                tools: None,
            },
        )]),
    }
}

fn app_tool_requirements(
    app_id: &str,
    tool_name: &str,
    approval_mode: AppToolApproval,
) -> AppsRequirementsToml {
    AppsRequirementsToml {
        apps: BTreeMap::from([(
            app_id.to_string(),
            AppRequirementToml {
                enabled: None,
                tools: Some(AppToolsRequirementsToml {
                    tools: BTreeMap::from([(
                        tool_name.to_string(),
                        AppToolRequirementToml {
                            approval_mode: Some(approval_mode),
                        },
                    )]),
                }),
            },
        )]),
    }
}

fn defaults(
    enabled: bool,
    destructive_enabled: bool,
    open_world_enabled: bool,
) -> AppsDefaultConfig {
    AppsDefaultConfig {
        enabled,
        approvals_reviewer: None,
        destructive_enabled,
        open_world_enabled,
        default_tools_approval_mode: None,
    }
}
