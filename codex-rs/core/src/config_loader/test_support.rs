#![cfg(test)]

use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;

static MANAGED_CONFIG_PATH_OVERRIDE: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
static MANAGED_CONFIG_PATH_SERIALIZER: OnceLock<Mutex<()>> = OnceLock::new();

fn managed_config_path_override_storage() -> &'static Mutex<Option<PathBuf>> {
    MANAGED_CONFIG_PATH_OVERRIDE.get_or_init(|| Mutex::new(None))
}

fn managed_config_path_serializer() -> &'static Mutex<()> {
    MANAGED_CONFIG_PATH_SERIALIZER.get_or_init(|| Mutex::new(()))
}

pub(super) fn current_managed_config_path_override() -> Option<PathBuf> {
    let guard = managed_config_path_override_storage()
        .lock()
        .unwrap_or_else(|_| panic!("managed config path override mutex poisoned"));
    guard.clone()
}

pub(crate) struct ManagedConfigPathOverrideGuard {
    previous: Option<PathBuf>,
    serializer_guard: Option<std::sync::MutexGuard<'static, ()>>,
}

impl Drop for ManagedConfigPathOverrideGuard {
    fn drop(&mut self) {
        let mut guard = managed_config_path_override_storage()
            .lock()
            .unwrap_or_else(|_| panic!("managed config path override mutex poisoned"));
        *guard = self.previous.take();
        drop(guard);
        if let Some(serializer_guard) = self.serializer_guard.take() {
            drop(serializer_guard);
        }
    }
}

pub(crate) fn with_managed_config_path_override(
    path: Option<&Path>,
) -> ManagedConfigPathOverrideGuard {
    let serializer_guard = managed_config_path_serializer()
        .lock()
        .unwrap_or_else(|_| panic!("managed config path serializer mutex poisoned"));

    let mut guard = managed_config_path_override_storage()
        .lock()
        .unwrap_or_else(|_| panic!("managed config path override mutex poisoned"));
    let previous = guard.clone();
    *guard = path.map(Path::to_path_buf);
    drop(guard);

    ManagedConfigPathOverrideGuard {
        previous,
        serializer_guard: Some(serializer_guard),
    }
}
