use pretty_assertions::assert_eq;

use super::ConnectorMetadata;
use super::ConnectorMetadataStore;
use super::ConnectorToolSummary;

fn metadata(id: &str) -> ConnectorMetadata {
    ConnectorMetadata {
        id: id.to_string(),
        name: format!("{id} name"),
        description: None,
        icon_url: None,
        tool_summaries: None,
    }
}

#[test]
fn records_are_isolated_by_backend_account_user_and_workspace_scope() {
    let requested_scope = ConnectorMetadataStore::new(
        "https://backend-a.example".to_string(),
        Some("account-a".to_string()),
        Some("user-a".to_string()),
        /*is_workspace_account*/ true,
    );
    let other_backend = ConnectorMetadataStore::new(
        "https://backend-b.example".to_string(),
        Some("account-a".to_string()),
        Some("user-a".to_string()),
        /*is_workspace_account*/ true,
    );
    let other_account = ConnectorMetadataStore::new(
        "https://backend-a.example".to_string(),
        Some("account-b".to_string()),
        Some("user-a".to_string()),
        /*is_workspace_account*/ true,
    );
    let other_user = ConnectorMetadataStore::new(
        "https://backend-a.example".to_string(),
        Some("account-a".to_string()),
        Some("user-b".to_string()),
        /*is_workspace_account*/ true,
    );
    let personal_account = ConnectorMetadataStore::new(
        "https://backend-a.example".to_string(),
        Some("account-a".to_string()),
        Some("user-a".to_string()),
        /*is_workspace_account*/ false,
    );
    let ids = vec!["scoped-app".to_string()];

    requested_scope.commit(&[metadata("scoped-app")]);

    assert_eq!(
        requested_scope.fresh_records(&ids, /*include_tools*/ false),
        std::collections::HashMap::from([("scoped-app".to_string(), metadata("scoped-app"))])
    );
    assert_eq!(
        other_backend.fresh_records(&ids, /*include_tools*/ false),
        Default::default()
    );
    assert_eq!(
        other_account.fresh_records(&ids, /*include_tools*/ false),
        Default::default()
    );
    assert_eq!(
        other_user.fresh_records(&ids, /*include_tools*/ false),
        Default::default()
    );
    assert_eq!(
        personal_account.fresh_records(&ids, /*include_tools*/ false),
        Default::default()
    );
}

#[test]
fn tool_inclusive_reads_require_cached_tool_summaries() {
    let store = ConnectorMetadataStore::new(
        "https://backend-tools.example".to_string(),
        Some("account-tools".to_string()),
        Some("user-tools".to_string()),
        /*is_workspace_account*/ false,
    );
    let metadata_only = metadata("metadata-only");
    let mut empty_tools = metadata("empty-tools");
    empty_tools.tool_summaries = Some(Vec::new());
    let mut with_tools = metadata("with-tools");
    with_tools.tool_summaries = Some(vec![ConnectorToolSummary {
        name: "search".to_string(),
        title: Some("Search".to_string()),
        description: "Search the app".to_string(),
    }]);
    let ids = vec![
        "metadata-only".to_string(),
        "empty-tools".to_string(),
        "with-tools".to_string(),
    ];

    store.commit(&[
        metadata_only.clone(),
        empty_tools.clone(),
        with_tools.clone(),
    ]);

    assert_eq!(
        store.fresh_records(&ids, /*include_tools*/ false),
        std::collections::HashMap::from([
            ("metadata-only".to_string(), metadata_only),
            ("empty-tools".to_string(), empty_tools.clone()),
            ("with-tools".to_string(), with_tools.clone()),
        ])
    );
    assert_eq!(
        store.fresh_records(&ids, /*include_tools*/ true),
        std::collections::HashMap::from([
            ("empty-tools".to_string(), empty_tools),
            ("with-tools".to_string(), with_tools),
        ])
    );
}

#[test]
fn metadata_only_commit_does_not_replace_fresh_tool_summaries() {
    let store = ConnectorMetadataStore::new(
        "https://backend-tools-race.example".to_string(),
        Some("account-tools-race".to_string()),
        Some("user-tools-race".to_string()),
        /*is_workspace_account*/ false,
    );
    let mut with_tools = metadata("with-tools");
    with_tools.tool_summaries = Some(vec![ConnectorToolSummary {
        name: "search".to_string(),
        title: Some("Search".to_string()),
        description: "Search the app".to_string(),
    }]);
    let ids = vec!["with-tools".to_string()];

    store.commit(&[with_tools.clone()]);
    store.commit(&[metadata("with-tools")]);

    assert_eq!(
        store.fresh_records(&ids, /*include_tools*/ true),
        std::collections::HashMap::from([("with-tools".to_string(), with_tools)])
    );
}
