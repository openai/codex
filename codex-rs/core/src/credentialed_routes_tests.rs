use super::*;

#[test]
fn developer_instructions_deduplicate_and_sort_route_prefixes() {
    let credentialed_routes = CredentialedRoutesConfig {
        routes: vec![
            CredentialedRoute {
                connector_id: "connector_b".to_string(),
                link_id: "link_b".to_string(),
                base_url: "https://b.example.com/v1".to_string(),
            },
            CredentialedRoute {
                connector_id: "connector_a".to_string(),
                link_id: "link_a".to_string(),
                base_url: "https://a.example.com/v1".to_string(),
            },
            CredentialedRoute {
                connector_id: "connector_a".to_string(),
                link_id: "link_a".to_string(),
                base_url: "https://a.example.com/v1".to_string(),
            },
        ],
        ..CredentialedRoutesConfig::default()
    };

    assert_eq!(
        developer_instructions(&credentialed_routes),
        Some(
            "The managed network proxy automatically attaches stored credentials when you call these HTTPS URL prefixes directly:\n- https://a.example.com/v1\n- https://b.example.com/v1".to_string()
        )
    );
}

#[test]
fn developer_instructions_cap_route_prefixes() {
    let credentialed_routes = CredentialedRoutesConfig {
        routes: (0..=MAX_CREDENTIALED_ROUTE_INSTRUCTION_PREFIXES)
            .map(|index| CredentialedRoute {
                connector_id: format!("connector_{index}"),
                link_id: format!("link_{index}"),
                base_url: format!("https://{index}.example.com/v1"),
            })
            .collect(),
        ..CredentialedRoutesConfig::default()
    };

    let instructions =
        developer_instructions(&credentialed_routes).expect("routes should render instructions");

    assert!(instructions.len() <= MAX_CREDENTIALED_ROUTE_INSTRUCTION_CHARS);
    assert!(instructions.contains("[additional credentialed routes omitted]"));
}
