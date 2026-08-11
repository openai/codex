use super::*;
use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillResourceId;

fn detect_executor_skill(command: &str, environments: &[&str]) -> Option<SkillInvocation> {
    let skill_path = PathUri::parse("file:///skills/demo/SKILL.md").expect("valid skill URI");
    let catalog = SkillCatalog {
        entries: environments
            .iter()
            .map(|environment| {
                SkillCatalogEntry::new(
                    SkillPackageId(format!("skill://{environment}/demo")),
                    SkillAuthority::new(SkillSourceKind::Executor, *environment),
                    format!("{environment}-skill"),
                    "test skill",
                    SkillResourceId::environment(
                        format!("skill://{environment}/demo/SKILL.md"),
                        *environment,
                        skill_path.clone(),
                    ),
                )
            })
            .collect(),
        warnings: Vec::new(),
    };
    let turn_store = ExtensionData::new("test-turn");
    turn_store.insert(ExecutorSkillsStepState(catalog));

    detect_implicit_skill_invocation(
        &turn_store,
        "executor-a",
        command,
        &PathUri::parse("file:///skills").expect("valid workdir URI"),
        /*native_workdir*/ None,
    )
}

#[test]
fn detects_executor_skill_document_reads() {
    let invocation = detect_executor_skill("cat demo/SKILL.md", &["executor-a"])
        .expect("document read should identify the executor skill");

    assert_eq!(invocation.skill_name, "executor-a-skill");
    assert!(matches!(
        invocation.location,
        SkillInvocationLocation::Resource { id, .. }
            if id == "skill://executor-a/demo/SKILL.md"
    ));
}

#[test]
fn detects_executor_skill_script_runs() {
    let invocation = detect_executor_skill("python demo/scripts/run.py", &["executor-a"])
        .expect("script execution should identify the executor skill");

    assert_eq!(invocation.skill_name, "executor-a-skill");
}

#[test]
fn ignores_matching_paths_from_other_environments() {
    let invocation = detect_executor_skill("cat demo/SKILL.md", &["executor-b", "executor-a"])
        .expect("matching path should resolve to the execution environment");

    assert_eq!(invocation.skill_name, "executor-a-skill");
    assert!(detect_executor_skill("cat demo/SKILL.md", &["executor-b"]).is_none());
}
