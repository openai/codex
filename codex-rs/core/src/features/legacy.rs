use super::Feature;
use super::Features;
use tracing::info;

#[derive(Clone, Copy)]
struct Alias {
    legacy_key: &'static str,
    feature: Feature,
}

const ALIASES: &[Alias] = &[
    Alias {
        legacy_key: "connectors",
        feature: Feature::Apps,
    },
    Alias {
        legacy_key: "enable_experimental_windows_sandbox",
        feature: Feature::WindowsSandbox,
    },
    Alias {
        legacy_key: "experimental_use_unified_exec_tool",
        feature: Feature::UnifiedExec,
    },
    Alias {
        legacy_key: "experimental_use_freeform_apply_patch",
        feature: Feature::ApplyPatchFreeform,
    },
    Alias {
        legacy_key: "include_apply_patch_tool",
        feature: Feature::ApplyPatchFreeform,
    },
    Alias {
        legacy_key: "web_search",
        feature: Feature::WebSearchRequest,
    },
];

pub(crate) fn legacy_feature_keys() -> impl Iterator<Item = &'static str> {
    ALIASES.iter().map(|alias| alias.legacy_key)
}

pub(crate) fn feature_for_key(key: &str) -> Option<Feature> {
    ALIASES
        .iter()
        .find(|alias| alias.legacy_key == key)
        .map(|alias| {
            log_alias(alias.legacy_key, alias.feature);
            alias.feature
        })
}

#[derive(Debug, Default)]
pub struct LegacyFeatureToggles {
    pub include_apply_patch_tool: Option<bool>,
    pub experimental_use_freeform_apply_patch: Option<bool>,
    pub experimental_use_unified_exec_tool: Option<bool>,
    pub tools_web_search: Option<bool>,
}

impl LegacyFeatureToggles {
    pub fn apply(self, features: &mut Features) {
        set_if_some(
            features,
            Feature::ApplyPatchFreeform,
            self.include_apply_patch_tool,
            "include_apply_patch_tool",
        );
        set_if_some(
            features,
            Feature::ApplyPatchFreeform,
            self.experimental_use_freeform_apply_patch,
            "experimental_use_freeform_apply_patch",
        );
        set_if_some(
            features,
            Feature::UnifiedExec,
            self.experimental_use_unified_exec_tool,
            "experimental_use_unified_exec_tool",
        );
        set_if_some(
            features,
            Feature::WebSearchRequest,
            self.tools_web_search,
            "tools.web_search",
        );
    }
}

fn set_if_some(
    features: &mut Features,
    feature: Feature,
    maybe_value: Option<bool>,
    alias_key: &'static str,
) {
    if let Some(enabled) = maybe_value {
        set_feature(features, feature, enabled);
        log_alias(alias_key, feature);
        features.record_legacy_usage(alias_key, feature);
    }
}

fn set_feature(features: &mut Features, feature: Feature, enabled: bool) {
    if enabled {
        features.enable(feature);
    } else {
        features.disable(feature);
    }
}

fn log_alias(alias: &str, feature: Feature) {
    let canonical = feature.key();
    if alias == canonical {
        return;
    }
    info!(
        %alias,
        canonical,
        "legacy feature toggle detected; prefer `[features].{canonical}`"
    );
}
