//! V2 discovery requests and responses round-trip through JSON.

use super::*;
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
        name: "example".to_string(),
        namespace: Some("nested".to_string()),
        description: "Example skill".to_string(),
        short_description: None,
        path: PathUri::parse("file:///plugin/skills/example/SKILL.md")?,
        metadata: Some(text_file.clone()),
    };
    let response = DiscoverV2CapabilitiesResponse {
        plugins: vec![ExecutorPlugin {
            id: "example@company".to_string(),
            remote_plugin_id: None,
            version: "local".to_string(),
            root: PathUri::parse("file:///plugin")?,
            files: DiscoveredPluginFiles {
                manifest: text_file.clone(),
                mcp_config: Some(text_file.clone()),
                apps_config: Some(text_file),
            },
            skills: vec![skill.clone()],
        }],
        skills: vec![skill],
        warnings: Vec::new(),
    };
    let wire = serde_json::to_value(&response)?;
    assert_eq!(
        serde_json::from_value::<DiscoverV2CapabilitiesResponse>(wire)?,
        response
    );
    Ok(())
}
