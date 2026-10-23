//! Applies runtime provider defaults using the resolved session working directory.

use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::config_types::ModelProviderAuthInfo;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::num::NonZeroU64;

/// Supplies GCP command auth when no explicit auth override is configured.
pub fn apply_default_gcp_auth(provider: &mut ModelProviderInfo, cwd: &AbsolutePathBuf) {
    provider.auth.get_or_insert_with(|| ModelProviderAuthInfo {
        command: if cfg!(windows) {
            "gcloud.cmd"
        } else {
            "gcloud"
        }
        .to_string(),
        args: vec![
            "auth".into(),
            "application-default".into(),
            "print-access-token".into(),
        ],
        timeout_ms: NonZeroU64::new(30_000).unwrap_or(NonZeroU64::MIN),
        refresh_interval_ms: 3_000_000,
        // Registry construction must not depend on the process cwd, which may be deleted.
        cwd: cwd.clone(),
    });
}
