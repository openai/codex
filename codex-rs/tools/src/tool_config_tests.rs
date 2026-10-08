use codex_features::Feature;
use codex_features::Features;
use codex_protocol::config_types::ModeKind;
use pretty_assertions::assert_eq;

use super::*;

#[test]
fn request_user_input_modes_follow_default_mode_feature() {
    let mut features = Features::with_defaults();
    features.disable(Feature::DefaultModeRequestUserInput);
    assert_eq!(
        request_user_input_available_modes(&features),
        vec![ModeKind::Plan]
    );

    features.enable(Feature::DefaultModeRequestUserInput);
    assert_eq!(
        request_user_input_available_modes(&features),
        vec![ModeKind::Default, ModeKind::Plan]
    );
}
