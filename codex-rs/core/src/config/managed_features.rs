use std::collections::BTreeMap;

use codex_config::Constrained;
use codex_config::ConstrainedWithSource;
use codex_config::ConstraintError;
use codex_config::ConstraintResult;
use codex_config::FeatureRequirementsToml;
use codex_config::RequirementSource;
use codex_config::Sourced;

use crate::config::ConfigToml;
use crate::config::profile::ConfigProfile;
use crate::features::Feature;
use crate::features::FeatureOverrides;
use crate::features::Features;
use crate::features::canonical_feature_for_key;
use crate::features::feature_for_key;

/// Wrapper around [`Features`] which enforces constraints defined in
/// `FeatureRequirementsToml` and provides normalization to ensure constraints
/// are satisfied. Constraints are enforced on construction and mutation of
/// `ManagedFeatures`.
#[derive(Debug, Clone, PartialEq)]
pub struct ManagedFeatures {
    value: ConstrainedWithSource<Features>,
    pinned_features: BTreeMap<Feature, bool>,
}

impl ManagedFeatures {
    pub(crate) fn from_configured(
        configured_features: Features,
        feature_requirements: Option<Sourced<FeatureRequirementsToml>>,
    ) -> std::io::Result<Self> {
        let (pinned_features, source) = match feature_requirements {
            Some(Sourced {
                value: feature_requirements,
                source,
            }) => (
                parse_feature_requirements(feature_requirements, &source)?,
                Some(source),
            ),
            None => (BTreeMap::new(), None),
        };

        let normalized_features = normalize_candidate(configured_features, &pinned_features);
        validate_pinned_features(&normalized_features, &pinned_features, source.as_ref())?;
        Ok(Self {
            value: ConstrainedWithSource::new(Constrained::allow_any(normalized_features), source),
            pinned_features,
        })
    }

    pub fn get(&self) -> &Features {
        self.value.get()
    }

    fn normalize_and_validate(&self, candidate: Features) -> ConstraintResult<Features> {
        let normalized = normalize_candidate(candidate, &self.pinned_features);
        self.value.can_set(&normalized)?;
        validate_pinned_features_constraint(
            &normalized,
            &self.pinned_features,
            self.value.source.as_ref(),
        )?;
        Ok(normalized)
    }

    pub fn can_set(&self, candidate: &Features) -> ConstraintResult<()> {
        self.normalize_and_validate(candidate.clone()).map(|_| ())
    }

    pub fn set(&mut self, candidate: Features) -> ConstraintResult<()> {
        let normalized = self.normalize_and_validate(candidate)?;
        self.value.value.set(normalized)
    }

    pub fn set_enabled(&mut self, feature: Feature, enabled: bool) -> ConstraintResult<()> {
        let mut next = self.get().clone();
        next.set_enabled(feature, enabled);
        self.set(next)
    }

    pub fn enable(&mut self, feature: Feature) -> ConstraintResult<()> {
        self.set_enabled(feature, true)
    }

    pub fn disable(&mut self, feature: Feature) -> ConstraintResult<()> {
        self.set_enabled(feature, false)
    }
}

/// Only available for tests to ensure `ManagedFeatures` is constructed with
/// any required constraints taken into account.
#[cfg(test)]
impl From<Features> for ManagedFeatures {
    fn from(features: Features) -> Self {
        Self {
            value: ConstrainedWithSource::new(Constrained::allow_any(features), None),
            pinned_features: BTreeMap::new(),
        }
    }
}

impl std::ops::Deref for ManagedFeatures {
    type Target = Features;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

fn normalize_candidate(
    mut candidate: Features,
    pinned_features: &BTreeMap<Feature, bool>,
) -> Features {
    for (feature, enabled) in pinned_features {
        candidate.set_enabled(*feature, *enabled);
    }
    candidate.normalize_dependencies();
    candidate
}

fn validate_pinned_features_constraint(
    normalized_features: &Features,
    pinned_features: &BTreeMap<Feature, bool>,
    source: Option<&RequirementSource>,
) -> ConstraintResult<()> {
    let Some(source) = source else {
        return Ok(());
    };
    let allowed = feature_requirements_display(pinned_features);
    for (feature, enabled) in pinned_features {
        if normalized_features.enabled(*feature) != *enabled {
            return Err(ConstraintError::InvalidValue {
                field_name: "features",
                candidate: format!(
                    "{}={}",
                    feature.key(),
                    normalized_features.enabled(*feature)
                ),
                allowed,
                requirement_source: source.clone(),
            });
        }
    }

    Ok(())
}

fn validate_pinned_features(
    normalized_features: &Features,
    pinned_features: &BTreeMap<Feature, bool>,
    source: Option<&RequirementSource>,
) -> std::io::Result<()> {
    validate_pinned_features_constraint(normalized_features, pinned_features, source)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}

fn feature_requirements_display(feature_requirements: &BTreeMap<Feature, bool>) -> String {
    let values = feature_requirements
        .iter()
        .map(|(feature, enabled)| format!("{}={enabled}", feature.key()))
        .collect::<Vec<_>>();
    format!("[{}]", values.join(", "))
}

fn parse_feature_requirements(
    feature_requirements: FeatureRequirementsToml,
    source: &RequirementSource,
) -> std::io::Result<BTreeMap<Feature, bool>> {
    let mut pinned_features = BTreeMap::new();
    for (key, enabled) in feature_requirements.entries {
        if let Some(feature) = canonical_feature_for_key(&key) {
            pinned_features.insert(feature, enabled);
            continue;
        }

        if let Some(feature) = feature_for_key(&key) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "invalid `features` requirement `{key}` from {source}: use canonical feature key `{}`",
                    feature.key()
                ),
            ));
        }

        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid `features` requirement `{key}` from {source}"),
        ));
    }

    Ok(pinned_features)
}

fn explicit_feature_settings_in_config(cfg: &ConfigToml) -> Vec<(String, Feature, bool)> {
    let mut explicit_settings = Vec::new();

    if let Some(features) = cfg.features.as_ref() {
        for (key, enabled) in &features.entries {
            if let Some(feature) = feature_for_key(key) {
                explicit_settings.push((format!("features.{key}"), feature, *enabled));
            }
        }
    }
    if let Some(enabled) = cfg.experimental_use_unified_exec_tool {
        explicit_settings.push((
            "experimental_use_unified_exec_tool".to_string(),
            Feature::UnifiedExec,
            enabled,
        ));
    }
    if let Some(enabled) = cfg.experimental_use_freeform_apply_patch {
        explicit_settings.push((
            "experimental_use_freeform_apply_patch".to_string(),
            Feature::ApplyPatchFreeform,
            enabled,
        ));
    }
    if let Some(enabled) = cfg.tools.as_ref().and_then(|tools| tools.web_search) {
        explicit_settings.push((
            "tools.web_search".to_string(),
            Feature::WebSearchRequest,
            enabled,
        ));
    }

    for (profile_name, profile) in &cfg.profiles {
        if let Some(features) = profile.features.as_ref() {
            for (key, enabled) in &features.entries {
                if let Some(feature) = feature_for_key(key) {
                    explicit_settings.push((
                        format!("profiles.{profile_name}.features.{key}"),
                        feature,
                        *enabled,
                    ));
                }
            }
        }
        if let Some(enabled) = profile.include_apply_patch_tool {
            explicit_settings.push((
                format!("profiles.{profile_name}.include_apply_patch_tool"),
                Feature::ApplyPatchFreeform,
                enabled,
            ));
        }
        if let Some(enabled) = profile.experimental_use_unified_exec_tool {
            explicit_settings.push((
                format!("profiles.{profile_name}.experimental_use_unified_exec_tool"),
                Feature::UnifiedExec,
                enabled,
            ));
        }
        if let Some(enabled) = profile.experimental_use_freeform_apply_patch {
            explicit_settings.push((
                format!("profiles.{profile_name}.experimental_use_freeform_apply_patch"),
                Feature::ApplyPatchFreeform,
                enabled,
            ));
        }
        if let Some(enabled) = profile.tools_web_search {
            explicit_settings.push((
                format!("profiles.{profile_name}.tools_web_search"),
                Feature::WebSearchRequest,
                enabled,
            ));
        }
    }

    explicit_settings
}

pub(crate) fn validate_explicit_feature_settings_in_config_toml(
    cfg: &ConfigToml,
    feature_requirements: Option<&Sourced<FeatureRequirementsToml>>,
) -> std::io::Result<()> {
    let Some(Sourced {
        value: feature_requirements,
        source,
    }) = feature_requirements
    else {
        return Ok(());
    };

    let pinned_features = parse_feature_requirements(feature_requirements.clone(), source)?;
    if pinned_features.is_empty() {
        return Ok(());
    }

    let allowed = feature_requirements_display(&pinned_features);
    for (path, feature, enabled) in explicit_feature_settings_in_config(cfg) {
        if pinned_features
            .get(&feature)
            .is_some_and(|required| *required != enabled)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                ConstraintError::InvalidValue {
                    field_name: "features",
                    candidate: format!("{path}={enabled}"),
                    allowed,
                    requirement_source: source.clone(),
                },
            ));
        }
    }

    Ok(())
}

pub(crate) fn validate_feature_requirements_in_config_toml(
    cfg: &ConfigToml,
    feature_requirements: Option<&Sourced<FeatureRequirementsToml>>,
) -> std::io::Result<()> {
    fn validate_profile(
        cfg: &ConfigToml,
        profile_name: Option<&str>,
        profile: &ConfigProfile,
        feature_requirements: Option<&Sourced<FeatureRequirementsToml>>,
    ) -> std::io::Result<()> {
        let configured_features = Features::from_config(cfg, profile, FeatureOverrides::default());
        ManagedFeatures::from_configured(configured_features, feature_requirements.cloned())
            .map(|_| ())
            .map_err(|err| {
                if let Some(profile_name) = profile_name {
                    std::io::Error::new(
                        err.kind(),
                        format!(
                            "invalid feature configuration for profile `{profile_name}`: {err}"
                        ),
                    )
                } else {
                    err
                }
            })
    }

    validate_profile(cfg, None, &ConfigProfile::default(), feature_requirements)?;
    for (profile_name, profile) in &cfg.profiles {
        validate_profile(cfg, Some(profile_name), profile, feature_requirements)?;
    }
    Ok(())
}
