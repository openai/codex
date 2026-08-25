use pretty_assertions::assert_eq;
use sha2::Digest;
use sha2::Sha256;

use super::ContractManifest;
use super::ContractManifestError;
use super::ContractTest;

#[test]
fn canonical_manifest_is_sorted_and_reproducible() -> Result<(), Box<dyn std::error::Error>> {
    let first = ContractManifest::from_tests(
        /*parent_fingerprint*/ None,
        vec![
            ContractTest::new("skills.visible", "fn discover() {}"),
            ContractTest::new("executor.initialize", "fn execute() {}"),
        ],
    )?;
    let second = ContractManifest::from_tests(
        /*parent_fingerprint*/ None,
        vec![
            ContractTest::new("executor.initialize", "fn execute() {}"),
            ContractTest::new("skills.visible", "fn discover() {}"),
        ],
    )?;

    assert_eq!(first, second);
    assert_eq!(first.canonical_bytes()?, second.canonical_bytes()?);

    Ok(())
}

#[test]
fn canonical_manifest_normalizes_source_line_endings() -> Result<(), Box<dyn std::error::Error>> {
    let unix = ContractManifest::from_tests(
        /*parent_fingerprint*/ None,
        vec![ContractTest::new(
            "skills.visible",
            "fn contract() {\n    assert!(visible());\n}\n",
        )],
    )?;
    let windows = ContractManifest::from_tests(
        /*parent_fingerprint*/ None,
        vec![ContractTest::new(
            "skills.visible",
            "fn contract() {\r\n    assert!(visible());\r\n}\r\n",
        )],
    )?;

    assert_eq!(unix.canonical_bytes()?, windows.canonical_bytes()?);

    Ok(())
}

#[test]
fn fingerprint_identifies_compiled_verifier_independently_of_parent()
-> Result<(), Box<dyn std::error::Error>> {
    let mut executable = std::fs::File::open(std::env::current_exe()?)?;
    let mut digest = Sha256::new();
    std::io::copy(&mut executable, &mut digest)?;
    let verifier_fingerprint = format!("sha256:{:x}", digest.finalize());

    let root = ContractManifest::from_tests(
        /*parent_fingerprint*/ None,
        vec![ContractTest::new("skills.visible", "fn discover() {}")],
    )?;
    let successor = ContractManifest::from_tests(
        Some(verifier_fingerprint.clone()),
        vec![ContractTest::new("skills.visible", "fn discover() {}")],
    )?;

    assert_ne!(root.canonical_bytes()?, successor.canonical_bytes()?);
    assert_eq!(root.fingerprint()?, verifier_fingerprint);
    assert_eq!(successor.fingerprint()?, verifier_fingerprint);

    Ok(())
}

#[test]
fn empty_contract_suites_are_rejected() {
    assert_eq!(
        ContractManifest::from_tests(/*parent_fingerprint*/ None, Vec::new()),
        Err(ContractManifestError::EmptySuite)
    );
}

#[test]
fn duplicate_contract_ids_are_rejected() {
    assert_eq!(
        ContractManifest::from_tests(
            /*parent_fingerprint*/ None,
            vec![
                ContractTest::new("skills.visible", "fn discover() {}"),
                ContractTest::new("skills.visible", "fn discover_restricted() {}"),
            ],
        ),
        Err(ContractManifestError::DuplicateTestId {
            id: "skills.visible".to_string(),
        })
    );
}

#[test]
fn manifest_records_same_file_helper_changes() -> Result<(), Box<dyn std::error::Error>> {
    let first = ContractManifest::from_tests(
        /*parent_fingerprint*/ None,
        vec![ContractTest::new(
            "skills.visible",
            "fn helper() -> bool { true }\nfn contract() { assert!(helper()); }",
        )],
    )?;
    let revised = ContractManifest::from_tests(
        /*parent_fingerprint*/ None,
        vec![ContractTest::new(
            "skills.visible",
            "fn helper() -> bool { false }\nfn contract() { assert!(helper()); }",
        )],
    )?;

    assert_ne!(first.canonical_bytes()?, revised.canonical_bytes()?);

    Ok(())
}
