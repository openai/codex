//! Snapshot validation and immutable resource reads across authority boundaries.

use super::*;
use crate::provider::SkillReadContext;
use pretty_assertions::assert_eq;
use serde_json::json;

fn snapshot() -> serde_json::Value {
    json!({"skills": [{
        "package": "skill://demo/work",
        "name": "demo:work",
        "description": "Do the work.",
        "alias_root": "skill://demo",
        "main_resource": "skill://demo/work/SKILL.md",
        "resources": {"skill://demo/work/SKILL.md": "# Work\nKeep it readable.\n"}
    }]})
}

fn request() -> SkillReadRequest<'static> {
    SkillReadRequest {
        authority: SkillAuthority::new(SkillSourceKind::Cloud, CODEX_APPS_MCP_SERVER_NAME),
        package: SkillPackageId("skill://demo/work".into()),
        resource: SkillResourceId::new("skill://demo/work/SKILL.md"),
        context: SkillReadContext::Cloud {
            mcp_resources: None,
        },
    }
}

#[tokio::test]
async fn file_is_loaded_once_and_reads_preserve_the_exact_contents() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("skills.json");
    std::fs::write(&path, snapshot().to_string()).unwrap();
    let provider = FrozenCloudSkillProvider::from_file(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        provider.read(request()).await.unwrap(),
        SkillReadResult {
            resource: request().resource,
            contents: "# Work\nKeep it readable.\n".into(),
        }
    );
    assert_eq!(provider.catalog.entries.len(), 1);
    assert!(provider.catalog.entries[0].prompt_visible);
    assert_eq!(
        provider.catalog.entries[0].rendered_path(),
        "skill://demo/work"
    );
}

#[tokio::test]
async fn hidden_skills_remain_readable_without_implicit_prompt_exposure() {
    let mut value = snapshot();
    value["skills"][0]["prompt_visible"] = json!(false);
    let provider = FrozenCloudSkillProvider::from_json(value.to_string().as_bytes()).unwrap();
    assert!(!provider.catalog.entries[0].prompt_visible);
    assert_eq!(
        provider.read(request()).await.unwrap().contents,
        "# Work\nKeep it readable.\n"
    );
}

#[tokio::test]
async fn reads_cannot_cross_authority_or_package_boundaries() {
    let provider = FrozenCloudSkillProvider::from_json(snapshot().to_string().as_bytes()).unwrap();
    let mut other_authority = request();
    other_authority.authority = SkillAuthority::new(SkillSourceKind::Executor, "demo");
    let mut other_package = request();
    other_package.package = SkillPackageId("skill://other/work".into());
    let mut other_resource = request();
    other_resource.resource = SkillResourceId::new("skill://demo/work/../SKILL.md");
    for request in [other_authority, other_package, other_resource] {
        assert!(provider.read(request).await.is_err());
    }
}

#[test]
fn malformed_snapshots_are_rejected_before_inference() {
    let mut missing_main = snapshot();
    missing_main["skills"][0]["main_resource"] = json!("skill://demo/work/missing.md");
    let mut escaped = snapshot();
    escaped["skills"][0]["resources"]["skill://other/secret.md"] = json!("secret");
    let mut oversized = snapshot();
    oversized["skills"][0]["resources"]["skill://demo/work/SKILL.md"] =
        json!("x".repeat(MAX_SKILL_RESOURCE_CONTENT_BYTES + 1));
    let mut duplicate = snapshot();
    duplicate["skills"] = json!([snapshot()["skills"][0], snapshot()["skills"][0]]);
    for value in [missing_main, escaped, oversized, duplicate] {
        assert!(FrozenCloudSkillProvider::from_json(value.to_string().as_bytes()).is_err());
    }
}
