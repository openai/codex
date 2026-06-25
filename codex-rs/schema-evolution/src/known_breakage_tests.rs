use super::*;
use crate::test_support::request_schema;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn only_new_exact_entries_acknowledge_breakages() -> Result<()> {
    let (before, after, breakage) = type_narrowing()?;
    let before_log = log(Vec::new());
    let after_log = log(vec![known(/*id*/ 1, &breakage)]);

    assert_eq!(
        check_request_narrowing(&before, &after, &before_log, &after_log)?,
        vec![breakage]
    );
    Ok(())
}

#[test]
fn existing_entries_form_a_complete_sequential_prefix() {
    let first = old_breakage(/*id*/ 1, "test/old");
    let before = log(vec![first.clone()]);
    let after = log(Vec::new());

    assert_eq!(
        known_breakage_problems(&before, &after, &[]),
        vec!["known breakage 1 was deleted; existing entries are append-only".to_string()]
    );

    let out_of_sequence = log(vec![KnownBreakage { id: 2, ..first }]);
    assert_eq!(
        validate_log("after", &out_of_sequence),
        vec!["after known-breakage entry 2 must have id 1".to_string()]
    );
}

#[test]
fn historical_entries_cannot_acknowledge_a_new_diff_and_cannot_be_edited() -> Result<()> {
    let (_, _, breakage) = type_narrowing()?;
    let historical = known(/*id*/ 1, &breakage);
    let before = log(vec![historical.clone()]);

    assert_eq!(
        known_breakage_problems(
            &before,
            &log(vec![historical.clone()]),
            std::slice::from_ref(&breakage),
        ),
        vec![
            "missing a new known-breakage entry for test/method TypeNarrowed at params".to_string()
        ]
    );

    let edited = KnownBreakage {
        justification: "a different justification".to_string(),
        ..historical
    };
    assert_eq!(
        known_breakage_problems(&before, &log(vec![edited]), &[]),
        vec![
            "known breakage 1 was edited or reordered; existing entries are append-only"
                .to_string()
        ]
    );
    Ok(())
}

#[test]
fn stale_mismatched_and_duplicate_new_entries_are_rejected() -> Result<()> {
    let (_, _, breakage) = type_narrowing()?;
    let stale = KnownBreakage {
        method: "test/other".to_string(),
        ..known(/*id*/ 1, &breakage)
    };
    assert_eq!(
        known_breakage_problems(
            &log(Vec::new()),
            &log(vec![stale]),
            std::slice::from_ref(&breakage),
        ),
        vec![
            "missing a new known-breakage entry for test/method TypeNarrowed at params".to_string(),
            "new known breakage 1 does not match a detected request-schema breakage".to_string(),
        ]
    );

    assert_eq!(
        known_breakage_problems(
            &log(Vec::new()),
            &log(vec![known(/*id*/ 1, &breakage), known(/*id*/ 2, &breakage),]),
            &[breakage],
        ),
        vec!["multiple new known-breakage entries match test/method at params".to_string()]
    );
    Ok(())
}

#[test]
fn log_metadata_and_json_snapshots_are_validated() {
    let invalid = KnownBreakageLog {
        version: 2,
        breakages: vec![KnownBreakage {
            id: 2,
            kind: ViolationKind::MethodRemoved,
            method: String::new(),
            path: String::new(),
            before_json: "not json".to_string(),
            after_json: "also not json".to_string(),
            justification: String::new(),
        }],
    };

    assert_eq!(
        validate_log("after", &invalid),
        vec![
            "after known-breakage log must use version 1".to_string(),
            "after known-breakage entry 2 must have id 1".to_string(),
            "after known breakage 2 needs a justification".to_string(),
            "after known breakage 2 needs a method".to_string(),
            "after known breakage 2 needs a path".to_string(),
            "after known breakage 2 has invalid JSON in before_json".to_string(),
            "after known breakage 2 has invalid JSON in after_json".to_string(),
        ]
    );
}

#[test]
fn templates_are_numbered_after_a_valid_prefix_only() -> Result<()> {
    let (_, _, breakage) = type_narrowing()?;
    let before = log(vec![old_breakage(/*id*/ 1, "test/old")]);
    let valid_after = before.clone();
    let valid_report = failure_report(
        &["missing".to_string()],
        std::slice::from_ref(&breakage),
        &before,
        &valid_after,
    )?;
    assert!(valid_report.contains("[[breakages]]\nid = 2"));

    let invalid_after = log(vec![
        old_breakage(/*id*/ 1, "test/old"),
        old_breakage(/*id*/ 2, "test/stale"),
    ]);
    let invalid_report =
        failure_report(&["stale".to_string()], &[breakage], &before, &invalid_after)?;
    assert!(!invalid_report.contains("[[breakages]]"));
    Ok(())
}

fn type_narrowing() -> Result<(ApiSchema, ApiSchema, SchemaBreakage)> {
    let before = ApiSchema::parse(&request_schema(json!({ "type": ["null", "string"] })))?;
    let after = ApiSchema::parse(&request_schema(json!({ "type": "string" })))?;
    let breakage = find_request_narrowing(&before, &after)?
        .into_iter()
        .next()
        .context("expected a type narrowing")?;
    Ok((before, after, breakage))
}

fn known(id: u64, breakage: &SchemaBreakage) -> KnownBreakage {
    KnownBreakage {
        justification: "documents the accepted wire-format break".to_string(),
        ..KnownBreakage::from_breakage(id, breakage)
    }
}

fn old_breakage(id: u64, method: &str) -> KnownBreakage {
    KnownBreakage {
        id,
        kind: ViolationKind::MethodRemoved,
        method: method.to_string(),
        path: "request".to_string(),
        before_json: "true".to_string(),
        after_json: "false".to_string(),
        justification: "documents the accepted wire-format break".to_string(),
    }
}

fn log(breakages: Vec<KnownBreakage>) -> KnownBreakageLog {
    KnownBreakageLog {
        version: KNOWN_BREAKAGE_LOG_VERSION,
        breakages,
    }
}
