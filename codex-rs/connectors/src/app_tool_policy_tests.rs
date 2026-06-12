use std::collections::BTreeMap;
use std::collections::HashMap;

use codex_config::AppRequirementToml;
use codex_config::AppToolRequirementToml;
use codex_config::AppToolsRequirementsToml;
use codex_config::AppsRequirementsToml;
use codex_config::types::AppConfig;
use codex_config::types::AppToolApproval;
use codex_config::types::AppToolConfig;
use codex_config::types::AppToolsConfig;
use codex_config::types::AppsConfigToml;
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
    let evaluator =
        AppToolPolicyEvaluator::from_apps_config_and_requirements(apps_config, &requirements);

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

fn input<'a>(tool_name: &'a str, tool_title: Option<&'a str>) -> AppToolPolicyInput<'a> {
    AppToolPolicyInput {
        connector_id: Some("calendar"),
        tool_name,
        tool_title,
        destructive_hint: Some(true),
        open_world_hint: Some(true),
    }
}
