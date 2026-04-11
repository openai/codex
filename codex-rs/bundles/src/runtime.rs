use crate::platform::RuntimeTarget;
use crate::platform::node_executable_name;
use crate::platform::node_repl_executable_name;
use crate::platform::python_executable_name;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use tokio::process::Command;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimePaths {
    pub node_modules_path: PathBuf,
    pub node_path: PathBuf,
    pub node_repl_path: PathBuf,
    pub python_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeMetadata {
    bundle_version: Option<String>,
}

pub(crate) async fn validate_runtime_root(
    runtime_root: &Path,
    bundle_format_version: u32,
    target: &RuntimeTarget,
) -> Result<RuntimePaths> {
    let _metadata = read_runtime_metadata(runtime_root).await?;
    let node_root = if bundle_format_version >= 2 {
        runtime_root.join("dependencies").join("node")
    } else {
        runtime_root.to_path_buf()
    };
    let bin_dir = if bundle_format_version >= 2 {
        runtime_root.join("dependencies").join("bin")
    } else {
        runtime_root.join("bin")
    };
    let node_path = node_root
        .join("bin")
        .join(node_executable_name(&target.platform));
    let node_repl_path = bin_dir.join(node_repl_executable_name(&target.platform));
    let node_modules_path = node_root.join("node_modules");
    let python_path = find_python_path(runtime_root, bundle_format_version, &target.platform).await;

    ensure_file(&node_path, "Node binary").await?;
    ensure_file(&node_repl_path, "node_repl binary").await?;
    ensure_directory(&node_modules_path, "node_modules directory").await?;
    ensure_file(&python_path, "Python binary").await?;
    if target.platform != "windows" {
        ensure_executable(&node_path, "Node binary").await?;
        ensure_executable(&node_repl_path, "node_repl binary").await?;
        ensure_executable(&python_path, "Python binary").await?;
    }

    let output = Command::new(&python_path)
        .arg("-c")
        .arg("import artifact_tool_v2; print(artifact_tool_v2.__version__)")
        .output()
        .await
        .with_context(|| format!("failed to run {}", python_path.display()))?;
    if !output.status.success() {
        bail!(
            "failed to validate artifact_tool_v2: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(RuntimePaths {
        node_modules_path,
        node_path,
        node_repl_path,
        python_path,
    })
}

async fn read_runtime_metadata(runtime_root: &Path) -> Result<RuntimeMetadata> {
    let path = runtime_root.join("runtime.json");
    let raw = fs::read_to_string(&path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("failed to parse {}", path.display()))
}

async fn find_python_path(
    runtime_root: &Path,
    bundle_format_version: u32,
    target_platform: &str,
) -> PathBuf {
    let python_root = if bundle_format_version >= 2 {
        runtime_root.join("dependencies").join("python")
    } else {
        runtime_root.join("python")
    };
    let executable_name = python_executable_name(target_platform);
    let candidates = if target_platform == "windows" {
        vec![
            python_root.join(executable_name),
            python_root.join("python").join(executable_name),
            python_root.join("bin").join(executable_name),
        ]
    } else {
        vec![
            python_root.join("bin").join(executable_name),
            python_root.join("bin").join("python"),
        ]
    };
    for candidate in &candidates {
        if path_exists(candidate).await {
            return candidate.clone();
        }
    }
    candidates[0].clone()
}

async fn ensure_file(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::metadata(path)
        .await
        .with_context(|| format!("missing {label}: {}", path.display()))?;
    if !metadata.is_file() {
        bail!("{label} is not a file: {}", path.display());
    }
    Ok(())
}

async fn ensure_directory(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::metadata(path)
        .await
        .with_context(|| format!("missing {label}: {}", path.display()))?;
    if !metadata.is_dir() {
        bail!("{label} is not a directory: {}", path.display());
    }
    Ok(())
}

#[cfg(unix)]
async fn ensure_executable(path: &Path, label: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path)
        .await
        .with_context(|| format!("missing {label}: {}", path.display()))?;
    if metadata.permissions().mode() & 0o111 == 0 {
        bail!("{label} is not executable: {}", path.display());
    }
    Ok(())
}

#[cfg(not(unix))]
async fn ensure_executable(_path: &Path, _label: &str) -> Result<()> {
    Ok(())
}

async fn path_exists(path: &Path) -> bool {
    fs::metadata(path).await.is_ok()
}
