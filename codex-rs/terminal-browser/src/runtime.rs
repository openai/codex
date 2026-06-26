use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use codex_utils_absolute_path::AbsolutePathBuf;
use tempfile::TempDir;

use crate::network::BrowserNetworkPolicy;

pub(crate) struct BrowserRuntime {
    _root: TempDir,
    pub(crate) root: AbsolutePathBuf,
    pub(crate) profile: AbsolutePathBuf,
    home: AbsolutePathBuf,
    temporary: AbsolutePathBuf,
    xdg_runtime: AbsolutePathBuf,
}

impl BrowserRuntime {
    pub(crate) fn create(persistent_profile: Option<&AbsolutePathBuf>) -> Result<Self> {
        let root = tempfile::Builder::new()
            .prefix("codex-carbonyl-")
            .tempdir()
            .context("create Carbonyl runtime directory")?;
        let absolute_root = AbsolutePathBuf::from_absolute_path(root.path())
            .context("resolve Carbonyl runtime directory")?;
        let profile = persistent_profile
            .cloned()
            .unwrap_or_else(|| absolute_root.join("profile"));
        let home = absolute_root.join("home");
        let temporary = absolute_root.join("tmp");
        let xdg_runtime = absolute_root.join("runtime");
        for directory in [&profile, &home, &temporary, &xdg_runtime] {
            std::fs::create_dir_all(directory.as_path()).with_context(|| {
                format!(
                    "create Carbonyl runtime path {}",
                    directory.as_path().display()
                )
            })?;
            set_private_permissions(directory.as_path())?;
        }
        Ok(Self {
            _root: root,
            root: absolute_root,
            profile,
            home,
            temporary,
            xdg_runtime,
        })
    }

    pub(crate) fn environment(
        &self,
        network_policy: &BrowserNetworkPolicy,
    ) -> HashMap<String, String> {
        let mut env = HashMap::new();
        for key in [
            "PATH",
            "LANG",
            "LC_ALL",
            "LC_CTYPE",
            "TZ",
            "SSL_CERT_FILE",
            "SSL_CERT_DIR",
        ] {
            if let Ok(value) = std::env::var(key)
                && (!matches!(key, "SSL_CERT_FILE" | "SSL_CERT_DIR")
                    || Path::new(&value).is_absolute())
            {
                env.insert(key.to_string(), value);
            }
        }
        env.insert(
            "HOME".to_string(),
            self.home.as_path().display().to_string(),
        );
        for key in ["TMPDIR", "TMP", "TEMP"] {
            env.insert(
                key.to_string(),
                self.temporary.as_path().display().to_string(),
            );
        }
        env.insert(
            "XDG_RUNTIME_DIR".to_string(),
            self.xdg_runtime.as_path().display().to_string(),
        );
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        env.insert("COLORTERM".to_string(), "truecolor".to_string());
        if let BrowserNetworkPolicy::ManagedProxy { http_addr } = network_policy {
            let proxy = format!("http://{http_addr}");
            env.insert("HTTP_PROXY".to_string(), proxy.clone());
            env.insert("HTTPS_PROXY".to_string(), proxy);
            env.insert("NO_PROXY".to_string(), "127.0.0.1,localhost".to_string());
        }
        env
    }
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(/*mode*/ 0o700))
        .with_context(|| format!("restrict Carbonyl runtime path {}", path.display()))
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
