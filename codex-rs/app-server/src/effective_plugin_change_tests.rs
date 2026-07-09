use super::*;
use codex_plugin::PluginId;
use pretty_assertions::assert_eq;

#[test]
fn only_workspace_listed_materializations_are_eligible() {
    let materialization =
        |name: &str,
         scope: RemotePluginScope,
         discoverability: Option<RemotePluginShareDiscoverability>| {
            RemotePluginMaterialization {
                plugin_id: PluginId::new(name.to_string(), "test".to_string())
                    .expect("valid plugin id"),
                scope,
                discoverability,
                authenticated_account_id: Some("account-123".to_string()),
            }
        };

    let mut materializations = vec![
        materialization(
            "eligible",
            RemotePluginScope::Workspace,
            Some(RemotePluginShareDiscoverability::Listed),
        ),
        materialization(
            "unlisted",
            RemotePluginScope::Workspace,
            Some(RemotePluginShareDiscoverability::Unlisted),
        ),
        materialization(
            "private",
            RemotePluginScope::Workspace,
            Some(RemotePluginShareDiscoverability::Private),
        ),
        materialization("workspace-missing", RemotePluginScope::Workspace, None),
        materialization("global", RemotePluginScope::Global, None),
        materialization("user", RemotePluginScope::User, None),
    ];
    let mut wrong_account = materialization(
        "wrong-account",
        RemotePluginScope::Workspace,
        Some(RemotePluginShareDiscoverability::Listed),
    );
    wrong_account.authenticated_account_id = Some("account-456".to_string());
    materializations.push(wrong_account);

    let plugin_ids = workspace_listed_plugin_ids(materializations, "account-123");

    assert_eq!(plugin_ids, BTreeSet::from(["eligible@test".to_string()]));
}
