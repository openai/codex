use super::*;

#[test]
fn instructions_deduplicate_sort_and_mark_route_prefixes() {
    let route_prefixes = vec![
        "https://b.example.com/v1".to_string(),
        "https://a.example.com/v1".to_string(),
        "https://a.example.com/v1".to_string(),
    ];

    let instructions =
        CredentialedRoutesInstructions::new(&route_prefixes).expect("routes should render");

    assert_eq!(
        instructions.render(),
        "<credentialed_routes>\nThe managed network proxy automatically attaches stored credentials when you call these HTTPS URL prefixes directly:\n- https://a.example.com/v1\n- https://b.example.com/v1\n</credentialed_routes>"
    );
}

#[test]
fn instructions_have_a_hard_size_cap() {
    let route_prefixes = (0..=MAX_ROUTE_PREFIXES)
        .map(|index| format!("https://{index}.example.com/a/long/credentialed/route"))
        .collect::<Vec<_>>();

    let instructions =
        CredentialedRoutesInstructions::new(&route_prefixes).expect("routes should render");

    assert!(instructions.body().len() <= MAX_INSTRUCTION_CHARS);
    assert!(
        instructions
            .body()
            .contains("[additional credentialed routes omitted]")
    );
}
