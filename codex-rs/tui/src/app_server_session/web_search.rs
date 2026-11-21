//! Forward only winning launch search choices, preserving legacy precedence and embedded pins.

use super::ThreadParamsMode;
use crate::legacy_core::config::Config;
use codex_config::ConfigLayerMetadata;
use codex_config::ConfigLayerSource;
use codex_features::Feature;
use std::collections::HashMap;

pub(super) fn apply_launch_override(
    config: &Config,
    thread_params_mode: ThreadParamsMode,
    effective: &toml::Value,
    origins: &HashMap<String, ConfigLayerMetadata>,
    overrides: &mut HashMap<String, serde_json::Value>,
) {
    // Losing legacy flags must not independently change the server's search mode.
    if let Some(features) = overrides
        .get_mut("features")
        .and_then(serde_json::Value::as_object_mut)
    {
        for key in ["web_search", "web_search_cached", "web_search_request"] {
            features.remove(key);
        }
        if features.is_empty() {
            overrides.remove("features");
        }
    }
    let is_launch = |key: &str| {
        origins.get(key).is_some_and(|origin| {
            matches!(
                origin.name,
                ConfigLayerSource::SessionFlags
                    | ConfigLayerSource::User {
                        profile: Some(_),
                        ..
                    }
            )
        })
    };
    if is_launch("features.web_search_indexed_default") {
        overrides
            .entry("features".to_string())
            .or_insert_with(|| serde_json::json!({}))["web_search_indexed_default"] =
            serde_json::json!(effective["features"]["web_search_indexed_default"]);
    }
    // A canonical setting always takes precedence, even when its origin is implicit.
    if let Some(value) = effective.get("web_search") {
        if is_launch("web_search") {
            overrides.insert("web_search".to_string(), serde_json::json!(value));
        }
        return;
    }
    let Some(features) = effective.get("features").and_then(toml::Value::as_table) else {
        return;
    };
    let request_key = if features.contains_key("web_search_request") {
        "web_search_request"
    } else {
        "web_search"
    };
    let cached = config.features.enabled(Feature::WebSearchCached);
    let live = config.features.enabled(Feature::WebSearchRequest);
    let active = [("web_search_cached", "cached"), (request_key, "live")]
        .into_iter()
        .find(|(key, mode)| {
            features.get(*key).and_then(toml::Value::as_bool) == Some(true)
                && (thread_params_mode == ThreadParamsMode::Remote
                    || if *mode == "cached" {
                        cached
                    } else {
                        live && !cached
                    })
        });
    let (key, mode) = match active {
        Some(choice) => choice,
        None => {
            if thread_params_mode == ThreadParamsMode::Embedded && (cached || live) {
                return;
            }
            let Some(key) = ["web_search_cached", request_key].into_iter().find(|key| {
                features.get(*key).and_then(toml::Value::as_bool) == Some(false)
                    && is_launch(&format!("features.{key}"))
            }) else {
                return;
            };
            let indexed_default_enabled = if thread_params_mode == ThreadParamsMode::Embedded {
                config.features.enabled(Feature::WebSearchIndexedDefault)
            } else {
                is_launch("features.web_search_indexed_default")
                    && features
                        .get("web_search_indexed_default")
                        .and_then(toml::Value::as_bool)
                        == Some(true)
            };
            if indexed_default_enabled {
                // Keep the default implicit so full-access turns can still promote it to live.
                let features = overrides
                    .entry("features".to_string())
                    .or_insert_with(|| serde_json::json!({}));
                features["web_search_cached"] = false.into();
                features["web_search_request"] = false.into();
                return;
            }
            // Preserve the legacy cached fallback while the rollout is disabled.
            (key, "cached")
        }
    };
    // Remote servers enforce their own search-mode requirements on this raw choice.
    if is_launch(&format!("features.{key}"))
        || (mode == "live"
            && features
                .get("web_search_cached")
                .and_then(toml::Value::as_bool)
                == Some(false)
            && is_launch("features.web_search_cached"))
    {
        overrides.insert("web_search".to_string(), mode.into());
    }
}

#[cfg(test)]
#[path = "web_search_tests.rs"]
mod tests;
