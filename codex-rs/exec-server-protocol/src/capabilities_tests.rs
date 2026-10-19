//! V2 discovery wire contracts.

use super::*;
use crate::CapabilityTextFile;
use pretty_assertions::assert_eq;

#[test]
fn discovery_contract_round_trips_json() -> Result<(), Box<dyn std::error::Error>> {
    let request = DiscoverV2CapabilitiesRequest {
        cwd: PathUri::parse("file:///workspace")?,
        sandbox: None,
    };
    let wire = serde_json::to_value(&request)?;
    assert_eq!(
        serde_json::from_value::<DiscoverV2CapabilitiesRequest>(wire)?,
        request
    );
    let text_file = CapabilityTextFile {
        path: PathUri::parse("file:///plugin/config.json")?,
        contents: "{}".to_string(),
    };
    let skill = ExecutorSkill {
        name: "nested:example".to_string(),
        description: "Example skill".to_string(),
        short_description: None,
        path: PathUri::parse("file:///plugin/skills/example/SKILL.md")?,
        discovery_root: None,
        dependencies: Some(codex_skills::SkillDependencies {
            tools: vec![codex_skills::SkillToolDependency {
                r#type: "mcp".into(),
                value: "deploy".into(),
                description: None,
                transport: Some("stdio".into()),
                command: Some("deploy-mcp".into()),
                url: None,
                oauth_callback_port: Some(3118),
            }],
        }),
        policy: Some(codex_skills::SkillPolicy {
            allow_implicit_invocation: Some(false),
            products: vec![],
        }),
    };
    let mut standalone = skill.clone();
    standalone.path = PathUri::parse("file:///home/skills/nested/example/SKILL.md")?;
    standalone.discovery_root = Some(SkillDiscoveryRoot {
        id: "codex-home-skills".to_string(),
        path: PathUri::parse("file:///home/skills")?,
    });
    let response = DiscoverV2CapabilitiesResponse {
        plugins: vec![ExecutorPlugin {
            id: "example@company".to_string(),
            display_name: "Example".to_string(),
            remote_plugin_id: None,
            version: "local".to_string(),
            root: PathUri::parse("file:///plugin")?,
            files: DiscoveredPluginFiles {
                manifest: text_file.clone(),
                mcp_config: Some(text_file.clone()),
                apps_config: Some(text_file),
            },
            skills: vec![skill],
        }],
        skills: vec![standalone],
        warnings: Vec::new(),
    };
    let mut wire = serde_json::to_value(&response)?;
    assert_eq!(
        wire["skills"][0]["discoveryRoot"],
        serde_json::json!({
            "id": "codex-home-skills",
            "path": "file:///home/skills",
        })
    );
    assert_eq!(wire["plugins"][0]["skills"][0].get("discoveryRoot"), None);
    assert_eq!(
        serde_json::from_value::<DiscoverV2CapabilitiesResponse>(wire.clone())?,
        response
    );
    // Older executors omit the root metadata.
    wire["skills"][0]
        .as_object_mut()
        .expect("skill object")
        .remove("discoveryRoot");
    let mut legacy_response = response;
    legacy_response.skills[0].discovery_root = None;
    assert_eq!(
        serde_json::from_value::<DiscoverV2CapabilitiesResponse>(wire)?,
        legacy_response
    );
    Ok(())
}
