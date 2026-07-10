use pretty_assertions::assert_eq;

use super::HostCapabilities;

#[test]
fn capability_names_are_normalized_deterministically() {
    let capabilities =
        HostCapabilities::from_names([" capability-b ", "", "capability-a", "capability-b"]);

    assert_eq!(
        capabilities.iter().collect::<Vec<_>>(),
        vec!["capability-a", "capability-b"]
    );
}
