use codex_internal_orchestrator_executor_contracts::ContractManifest;
use codex_internal_orchestrator_executor_contracts::ContractTest;
use codex_internal_orchestrator_executor_contracts::behavioral_contract;
use pretty_assertions::assert_eq;

behavioral_contract!(skills, "contracts/skills.rs");

#[test]
fn annotated_contract_files_are_discovered_automatically() -> anyhow::Result<()> {
    let manifest = ContractManifest::root()?;

    assert_eq!(
        manifest.tests(),
        &[ContractTest::new(
            "skills",
            include_str!("contracts/skills.rs"),
        )]
    );

    Ok(())
}
