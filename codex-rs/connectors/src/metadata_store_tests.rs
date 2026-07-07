use pretty_assertions::assert_eq;

use super::ConnectorMetadata;
use super::ConnectorMetadataStore;

fn metadata(id: &str) -> ConnectorMetadata {
    ConnectorMetadata {
        id: id.to_string(),
        name: format!("{id} name"),
        description: None,
        distribution_channel: None,
        branding: None,
        app_metadata: None,
        labels: None,
        install_url: None,
    }
}

#[test]
fn records_are_isolated_by_backend_account_user_and_workspace_scope() {
    let requested_scope = ConnectorMetadataStore::new(
        "https://backend-a.example".to_string(),
        Some("account-a".to_string()),
        Some("user-a".to_string()),
        true,
    );
    let other_backend = ConnectorMetadataStore::new(
        "https://backend-b.example".to_string(),
        Some("account-a".to_string()),
        Some("user-a".to_string()),
        true,
    );
    let other_account = ConnectorMetadataStore::new(
        "https://backend-a.example".to_string(),
        Some("account-b".to_string()),
        Some("user-a".to_string()),
        true,
    );
    let other_user = ConnectorMetadataStore::new(
        "https://backend-a.example".to_string(),
        Some("account-a".to_string()),
        Some("user-b".to_string()),
        true,
    );
    let personal_account = ConnectorMetadataStore::new(
        "https://backend-a.example".to_string(),
        Some("account-a".to_string()),
        Some("user-a".to_string()),
        false,
    );
    let ids = vec!["scoped-app".to_string()];

    requested_scope.commit(&[metadata("scoped-app")]);

    assert_eq!(
        requested_scope.fresh_records(&ids),
        std::collections::HashMap::from([("scoped-app".to_string(), metadata("scoped-app"))])
    );
    assert_eq!(other_backend.fresh_records(&ids), Default::default());
    assert_eq!(other_account.fresh_records(&ids), Default::default());
    assert_eq!(other_user.fresh_records(&ids), Default::default());
    assert_eq!(personal_account.fresh_records(&ids), Default::default());
}
