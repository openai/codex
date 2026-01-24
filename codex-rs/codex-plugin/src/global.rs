//! Global plugin service registry.
//!
//! Provides thread-safe global access to the plugin service.
//! The service is initialized once and cached for subsequent access.

use crate::error::Result;
use crate::service::PluginService;
use std::path::Path;
use std::sync::Arc;
use std::sync::LazyLock;
use tokio::sync::RwLock;
use tracing::debug;

/// Global plugin service instance.
static PLUGIN_SERVICE: LazyLock<RwLock<Option<Arc<PluginService>>>> =
    LazyLock::new(|| RwLock::new(None));

/// Get or initialize the global plugin service.
///
/// If the service hasn't been initialized, it creates a new one with the
/// given codex_home path. Subsequent calls return the cached service.
///
/// # Arguments
///
/// * `codex_home` - Path to the codex home directory (~/.codex)
///
/// # Returns
///
/// Arc to the plugin service, or error if initialization fails.
pub async fn get_or_init_plugin_service(codex_home: &Path) -> Result<Arc<PluginService>> {
    // Check if already initialized
    {
        let service = PLUGIN_SERVICE.read().await;
        if let Some(ref s) = *service {
            return Ok(s.clone());
        }
    }

    // Initialize
    debug!(
        "Initializing global plugin service with home: {}",
        codex_home.display()
    );
    let service = Arc::new(PluginService::new(codex_home).await?);

    {
        let mut global = PLUGIN_SERVICE.write().await;
        *global = Some(service.clone());
    }

    Ok(service)
}

/// Get the global plugin service if initialized.
///
/// Returns None if the service hasn't been initialized yet.
pub async fn get_plugin_service() -> Option<Arc<PluginService>> {
    PLUGIN_SERVICE.read().await.clone()
}

/// Clear the global plugin service.
///
/// Use this to force re-initialization on next access.
pub async fn clear_plugin_service() {
    let mut service = PLUGIN_SERVICE.write().await;
    *service = None;
    debug!("Cleared global plugin service");
}

/// Check if the global plugin service is initialized.
pub async fn is_plugin_service_initialized() -> bool {
    PLUGIN_SERVICE.read().await.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;

    #[tokio::test]
    #[serial]
    async fn test_get_or_init() {
        // Clear any existing state
        clear_plugin_service().await;

        let dir = tempdir().unwrap();
        let service = get_or_init_plugin_service(dir.path()).await.unwrap();

        assert!(is_plugin_service_initialized().await);
        assert_eq!(service.codex_home(), dir.path());

        // Subsequent calls return the same instance
        let service2 = get_or_init_plugin_service(dir.path()).await.unwrap();
        assert!(Arc::ptr_eq(&service, &service2));

        // Clean up
        clear_plugin_service().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_get_uninitialized() {
        clear_plugin_service().await;

        let service = get_plugin_service().await;
        assert!(service.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_clear() {
        let dir = tempdir().unwrap();
        get_or_init_plugin_service(dir.path()).await.unwrap();

        assert!(is_plugin_service_initialized().await);

        clear_plugin_service().await;

        assert!(!is_plugin_service_initialized().await);
    }
}
