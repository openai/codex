//! Prepare config edits and preserve provider definitions across source remapping in a write batch.
//! Explicit replacements and deletions remain ordered; only automatic sibling eviction is
//! ignored when retaining definitions for a later source upsert.

use super::ConfigManagerError;
use super::MergeError;
use super::apply_merge;
use super::shell_environment_policy_representation_switch;
use super::sparse_overlay;
use super::toml_value_to_item;
use super::value_at_path;
use codex_app_server_protocol::ConfigWriteErrorCode;
use codex_app_server_protocol::MergeStrategy;
use codex_config::ConfigLayerStack;
use codex_config::merge_toml_values;
use codex_core::config::edit::ConfigEdit;
use codex_network_proxy::CredentialProviderConfig;
use codex_network_proxy::validate_credential_provider;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use toml::Value as TomlValue;

const PROVIDERS_PATH: [&str; 3] = ["features", "network_proxy", "credentials"];

pub(super) struct CredentialProviderEdits {
    definitions: TomlValue,
    displaced_provider_ids: BTreeSet<String>,
}

impl CredentialProviderEdits {
    pub(super) fn new(config: &TomlValue) -> Self {
        Self {
            definitions: providers(config)
                .cloned()
                .unwrap_or_else(|| TomlValue::Table(Default::default())),
            displaced_provider_ids: BTreeSet::new(),
        }
    }

    pub(super) fn apply(
        &mut self,
        config: &mut TomlValue,
        segments: &[String],
        value: Option<&TomlValue>,
        strategy: MergeStrategy,
    ) -> Result<Option<ConfigEdit>, ConfigManagerError> {
        let persist_segments = if matches!(strategy, MergeStrategy::Upsert)
            && value.is_some_and(|value| {
                shell_environment_policy_representation_switch(config, segments, value)
            }) {
            vec!["shell_environment_policy".to_string()]
        } else {
            segments.to_vec()
        };
        let original_value = value_at_path(config, &persist_segments).cloned();
        self.apply_provider_merge(config, segments, value, strategy)
            .map_err(|err| match err {
                MergeError::Validation(message) => {
                    ConfigManagerError::write(ConfigWriteErrorCode::ConfigValidationError, message)
                }
            })?;
        if original_value.as_ref() == value_at_path(config, &persist_segments) {
            Ok(None)
        } else {
            config_edit_at_path(config, persist_segments).map(Some)
        }
    }

    pub(super) fn remapping_edits(
        &self,
        config: &TomlValue,
    ) -> Result<Vec<ConfigEdit>, ConfigManagerError> {
        // Use final definitions so later explicit edits and restored providers retain batch order.
        self.displaced_provider_ids
            .iter()
            .map(|id| {
                let mut segments = PROVIDERS_PATH.map(str::to_string).to_vec();
                segments.push(id.clone());
                config_edit_at_path(config, segments)
            })
            .collect()
    }

    fn apply_provider_merge(
        &mut self,
        config: &mut TomlValue,
        segments: &[String],
        value: Option<&TomlValue>,
        strategy: MergeStrategy,
    ) -> Result<(), MergeError> {
        if !segments
            .iter()
            .zip(PROVIDERS_PATH)
            .all(|(segment, expected)| segment == expected)
        {
            return apply_merge(config, segments, value, strategy).map(|_| ());
        }

        let previous_ids = providers(config)
            .and_then(TomlValue::as_table)
            .filter(|_| matches!(strategy, MergeStrategy::Upsert) && value.is_some())
            .into_iter()
            .flat_map(|table| table.keys().cloned())
            .collect::<BTreeSet<_>>();
        let mut overlay = value.map(|value| sparse_overlay(segments, value));
        let restored_ids = overlay
            .as_ref()
            .and_then(providers)
            .and_then(TomlValue::as_table)
            .into_iter()
            .flatten()
            .filter(|(id, update)| {
                matches!(strategy, MergeStrategy::Upsert)
                    && update.get("env").is_some()
                    && self.definitions.get(id.as_str()).is_some()
                    && self.definitions.get(id.as_str())
                        != providers(config).and_then(|current| current.get(id.as_str()))
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();

        if segments.len() > PROVIDERS_PATH.len() {
            apply_merge(
                &mut self.definitions,
                &segments[PROVIDERS_PATH.len()..],
                value,
                strategy.clone(),
            )?;
        } else {
            let update = value.and_then(|value| {
                PROVIDERS_PATH[segments.len()..]
                    .iter()
                    .try_fold(value, |value, key| value.get(*key))
            });
            if let Some(update) = update {
                if matches!(strategy, MergeStrategy::Upsert) {
                    merge_toml_values(&mut self.definitions, update);
                } else {
                    self.definitions = update.clone();
                }
            } else if value.is_none()
                || matches!(strategy, MergeStrategy::Replace)
                    && !value.is_some_and(TomlValue::is_bool)
            {
                self.definitions = TomlValue::Table(Default::default());
            }
        }

        if !restored_ids.is_empty()
            && let Some(overlay) = overlay.as_mut()
            && let Some(updates) = overlay
                .get_mut("features")
                .and_then(|features| features.get_mut("network_proxy"))
                .and_then(|proxy| proxy.get_mut("credentials"))
                .and_then(TomlValue::as_table_mut)
        {
            for id in restored_ids {
                if let Some(definition) = self.definitions.get(&id) {
                    self.displaced_provider_ids.insert(id.clone());
                    updates.insert(id, definition.clone());
                }
            }
            merge_toml_values(config, overlay);
        } else {
            apply_merge(config, segments, value, strategy)?;
        }
        self.displaced_provider_ids.extend(
            previous_ids
                .into_iter()
                .filter(|id| providers(config).is_none_or(|table| table.get(id).is_none())),
        );
        Ok(())
    }

    pub(super) fn validate_remapping(
        &self,
        original_user_config: &TomlValue,
        user_config: &TomlValue,
        layers: &ConfigLayerStack,
    ) -> anyhow::Result<()> {
        let mut previous = TomlValue::Table(Default::default());
        let mut effective = previous.clone();
        let mut user_reached = false;
        let mut to_validate = BTreeMap::new();
        for layer in layers.layers_low_to_high() {
            let is_user = layers
                .get_active_user_layer()
                .is_some_and(|user| std::ptr::eq(layer, user));
            merge_toml_values(
                &mut previous,
                if is_user {
                    original_user_config
                } else {
                    &layer.config
                },
            );
            merge_toml_values(&mut effective, &layer.config);
            user_reached |= is_user;
            if user_reached {
                for (id, old) in providers(&previous)
                    .and_then(TomlValue::as_table)
                    .into_iter()
                    .flatten()
                {
                    // Deleting an override and continuing an incomplete draft remain allowed.
                    if providers(user_config).is_none_or(|map| map.get(id).is_none()) {
                        continue;
                    }
                    let Some(current) = providers(&effective).and_then(|map| map.get(id)) else {
                        continue;
                    };
                    if current == old {
                        continue;
                    }
                    let Ok(old): Result<CredentialProviderConfig, _> = old.clone().try_into()
                    else {
                        continue;
                    };
                    if validate_credential_provider(id, &old).is_ok() {
                        to_validate.insert(id.clone(), current.clone());
                    }
                }
                // Compare final ownership, not temporary source moves within the write batch.
                for (id, definition) in providers(&effective)
                    .and_then(TomlValue::as_table)
                    .into_iter()
                    .flatten()
                {
                    let receives_source = providers(&previous)
                        .and_then(TomlValue::as_table)
                        .into_iter()
                        .flatten()
                        .any(|(old_id, old)| {
                            // Retained definitions exclude explicit deletes, not automatic eviction.
                            (self.definitions.get(old_id).is_some()
                                || self.definitions.get(id).is_some()
                                    && providers(original_user_config).and_then(|map| map.get(id))
                                        != providers(user_config).and_then(|map| map.get(id)))
                                && sources(old).any(|old_source| {
                                    !providers(&effective)
                                        .and_then(|map| map.get(old_id))
                                        .is_some_and(|current| {
                                            sources(current)
                                                .any(|source| same_source(source, old_source))
                                        })
                                        && sources(definition)
                                            .any(|source| same_source(source, old_source))
                                })
                        });
                    if receives_source || to_validate.contains_key(id) {
                        // A higher same-ID layer may complete a fragment; a different ID may not.
                        to_validate.insert(id.clone(), definition.clone());
                    }
                }
            }
        }
        for (id, definition) in to_validate {
            let provider: CredentialProviderConfig = definition.try_into()?;
            validate_credential_provider(&id, &provider)?;
        }
        Ok(())
    }
}

fn config_edit_at_path(
    config: &TomlValue,
    segments: Vec<String>,
) -> Result<ConfigEdit, ConfigManagerError> {
    match value_at_path(config, &segments) {
        Some(value) => Ok(ConfigEdit::SetPath {
            value: toml_value_to_item(value)
                .map_err(|err| ConfigManagerError::anyhow("failed to build config edits", err))?,
            segments,
        }),
        None => Ok(ConfigEdit::ClearPath { segments }),
    }
}

fn sources(provider: &TomlValue) -> impl Iterator<Item = &str> {
    provider
        .get("env")
        .and_then(TomlValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(TomlValue::as_str)
}

fn same_source(left: &str, right: &str) -> bool {
    left == right || cfg!(windows) && left.eq_ignore_ascii_case(right)
}

fn providers(config: &TomlValue) -> Option<&TomlValue> {
    PROVIDERS_PATH
        .iter()
        .try_fold(config, |value, key| value.get(*key))
}
