use pretty_assertions::assert_eq;
use serde::Deserialize;

use super::try_parse_powershell_commands;

#[derive(Debug, Deserialize)]
struct FixtureCase {
    name: String,
    script: String,
    expected: Option<Vec<Vec<String>>>,
}

#[test]
fn lowers_compact_literal_fixture() {
    // Supported outputs were captured from the PowerShell 7 AST subprocess parser. Rare forms
    // that need additional PowerShell-specific lowering are intentionally recorded as unsupported.
    let cases: Vec<FixtureCase> =
        serde_json::from_str(include_str!("fixtures/powershell_lowering.json"))
            .expect("valid PowerShell lowering fixture");

    for case in cases {
        assert_eq!(
            try_parse_powershell_commands(&case.script),
            case.expected,
            "fixture case: {}",
            case.name
        );
    }
}

#[test]
fn rejects_requires_directives() {
    assert_eq!(
        try_parse_powershell_commands("#requires -Modules Evil\nGet-Location"),
        None
    );
}
