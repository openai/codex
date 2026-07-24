use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn cloud_managed_layers_map_to_complete_api_objects() {
    for (domain_layer, api_layer) in [
        (CloudManagedLayer::Baseline, ApiCloudManagedLayer::Baseline),
        (
            CloudManagedLayer::SystemOverlay,
            ApiCloudManagedLayer::SystemOverlay,
        ),
    ] {
        let layer = ConfigLayer {
            name: ConfigLayerSource::CloudManaged {
                layer: domain_layer,
                id: "policy-1".to_string(),
                name: "Workspace policy".to_string(),
            },
            version: "etag-1".to_string(),
            config: json!({"model": "gpt-5"}),
            disabled_reason: Some("conflicts with another managed fragment".to_string()),
        };

        assert_eq!(
            config_layer_to_api(layer),
            ApiConfigLayer {
                name: ApiConfigLayerSource::CloudManaged {
                    layer: api_layer,
                    id: "policy-1".to_string(),
                    name: "Workspace policy".to_string(),
                },
                version: "etag-1".to_string(),
                config: json!({"model": "gpt-5"}),
                disabled_reason: Some("conflicts with another managed fragment".to_string()),
            }
        );
    }
}
