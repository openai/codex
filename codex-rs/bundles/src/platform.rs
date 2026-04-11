use anyhow::Result;
use anyhow::bail;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeTarget {
    pub arch: String,
    pub platform: String,
}

impl RuntimeTarget {
    pub fn current() -> Self {
        Self {
            arch: std::env::consts::ARCH.to_string(),
            platform: std::env::consts::OS.to_string(),
        }
    }
}

pub(crate) fn url_platform_key(target: &RuntimeTarget) -> Result<String> {
    Ok(format!(
        "{}-{}",
        target_url_platform(&target.platform)?,
        target_url_arch(&target.arch)?
    ))
}

pub(crate) fn release_platform_key(target: &RuntimeTarget) -> Result<String> {
    Ok(format!(
        "{}-{}",
        target_release_platform(&target.platform)?,
        target_release_arch(&target.arch)?
    ))
}

pub(crate) fn node_executable_name(target_platform: &str) -> &'static str {
    if target_platform == "windows" || target_platform == "win32" {
        "node.exe"
    } else {
        "node"
    }
}

pub(crate) fn node_repl_executable_name(target_platform: &str) -> &'static str {
    if target_platform == "windows" || target_platform == "win32" {
        "node_repl.exe"
    } else {
        "node_repl"
    }
}

pub(crate) fn python_executable_name(target_platform: &str) -> &'static str {
    if target_platform == "windows" || target_platform == "win32" {
        "python.exe"
    } else {
        "python3"
    }
}

fn target_url_platform(platform: &str) -> Result<&'static str> {
    match platform {
        "macos" | "darwin" => Ok("darwin"),
        "linux" => Ok("linux"),
        "windows" | "win32" => Ok("win32"),
        _ => bail!("unsupported platform: {platform}"),
    }
}

fn target_release_platform(platform: &str) -> Result<&'static str> {
    match platform {
        "macos" | "darwin" => Ok("macos"),
        "linux" => Ok("linux"),
        "windows" | "win32" => Ok("windows"),
        _ => bail!("unsupported platform: {platform}"),
    }
}

fn target_url_arch(arch: &str) -> Result<&'static str> {
    match arch.to_ascii_lowercase().as_str() {
        "x64" | "x86_64" | "amd64" => Ok("x64"),
        "arm64" | "aarch64" => Ok("arm64"),
        _ => bail!("unsupported architecture: {arch}"),
    }
}

fn target_release_arch(arch: &str) -> Result<&'static str> {
    match arch.to_ascii_lowercase().as_str() {
        "x64" | "x86_64" | "amd64" => Ok("x86_64"),
        "arm64" | "aarch64" => Ok("aarch64"),
        _ => bail!("unsupported architecture: {arch}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn test_target() -> RuntimeTarget {
        RuntimeTarget {
            arch: "arm64".to_string(),
            platform: "darwin".to_string(),
        }
    }

    #[test]
    fn release_platform_key_uses_runtime_layer_names() {
        assert_eq!(
            release_platform_key(&test_target()).expect("platform key"),
            "macos-aarch64"
        );
    }

    #[test]
    fn rejects_unsupported_architecture() {
        let target = RuntimeTarget {
            arch: "s390x".to_string(),
            platform: "linux".to_string(),
        };
        let err = url_platform_key(&target).expect_err("unsupported architecture");
        assert!(err.to_string().contains("unsupported architecture"));
    }
}
