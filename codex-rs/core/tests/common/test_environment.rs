use std::ffi::OsStr;

use anyhow::Result;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;

pub const TEST_ENVIRONMENT_ENV_VAR: &str = "CODEX_TEST_ENVIRONMENT";
pub const LEGACY_REMOTE_ENV_ENV_VAR: &str = "CODEX_TEST_REMOTE_ENV";
pub const DOCKER_CONTAINER_ENV_VAR: &str = "CODEX_TEST_REMOTE_ENV_CONTAINER_NAME";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestTargetOs {
    Linux,
    MacOs,
    Windows,
}

impl TestTargetOs {
    const fn host() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            unreachable!()
        }
    }

    const fn path_convention(self) -> PathConvention {
        match self {
            Self::Linux | Self::MacOs => PathConvention::Posix,
            Self::Windows => PathConvention::Windows,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TestEnvironment {
    Local,
    Docker { container_name: String },
    WineExec,
}

impl TestEnvironment {
    pub(crate) fn is_remote(&self) -> bool {
        !matches!(self, Self::Local)
    }

    pub(crate) fn docker_container_name(&self) -> Option<&str> {
        match self {
            Self::Docker { container_name } => Some(container_name),
            Self::Local | Self::WineExec => None,
        }
    }

    pub(crate) const fn target_os(&self) -> TestTargetOs {
        match self {
            Self::Local => TestTargetOs::host(),
            Self::Docker { .. } => TestTargetOs::Linux,
            Self::WineExec => TestTargetOs::Windows,
        }
    }

    pub(crate) fn remote_cwd(&self, instance_id: &str) -> Result<Option<LegacyAppPathString>> {
        let path_uri = match self {
            Self::Local => return Ok(None),
            Self::Docker { .. } => {
                PathUri::parse(&format!("file:///tmp/codex-core-test-cwd-{instance_id}"))?
            }
            Self::WineExec => {
                // Each Wine-exec test process has an isolated filesystem root, so this drive-root
                // path cannot collide with a different Bazel shard.
                PathUri::parse(&format!("file:///C:/codex-core-test-cwd-{instance_id}"))?
            }
        };
        Ok(Some(LegacyAppPathString::from_path_uri(
            &path_uri,
            self.path_convention(),
        )?))
    }

    pub(crate) fn path_convention(&self) -> PathConvention {
        self.target_os().path_convention()
    }
}

pub(crate) fn test_environment() -> TestEnvironment {
    let environment = parse_test_environment(
        std::env::var_os(TEST_ENVIRONMENT_ENV_VAR).as_deref(),
        std::env::var_os(LEGACY_REMOTE_ENV_ENV_VAR).as_deref(),
        std::env::var_os(DOCKER_CONTAINER_ENV_VAR).as_deref(),
    )
    .expect("invalid test environment configuration");

    if matches!(environment, TestEnvironment::WineExec) && !cfg!(target_os = "linux") {
        panic!("{TEST_ENVIRONMENT_ENV_VAR}=wine-exec is only supported on Linux");
    }

    environment
}

/// Returns the operating system used by the selected test execution environment.
pub fn test_target_os() -> TestTargetOs {
    test_environment().target_os()
}

/// Returns whether the selected test execution environment is remote.
pub fn is_remote_test_environment() -> bool {
    test_environment().is_remote()
}

/// Returns the selected Docker test container, when the harness requires direct access to it.
#[doc(hidden)]
pub fn test_docker_container_name() -> Option<String> {
    match test_environment() {
        TestEnvironment::Docker { container_name } => Some(container_name),
        TestEnvironment::Local | TestEnvironment::WineExec => None,
    }
}

/// Returns whether the Wine-backed executor is selected.
#[doc(hidden)]
pub fn is_wine_exec_test_environment() -> bool {
    matches!(test_environment(), TestEnvironment::WineExec)
}

fn parse_test_environment(
    configured_environment: Option<&OsStr>,
    legacy_remote_environment: Option<&OsStr>,
    docker_container: Option<&OsStr>,
) -> Result<TestEnvironment, String> {
    let configured_environment = configured_environment
        .map(|value| {
            value
                .to_str()
                .ok_or_else(|| format!("{TEST_ENVIRONMENT_ENV_VAR} must contain valid UTF-8"))
        })
        .transpose()?;

    match configured_environment {
        None => match legacy_remote_environment {
            Some(container_name) => Ok(TestEnvironment::Docker {
                container_name: non_empty_utf8(LEGACY_REMOTE_ENV_ENV_VAR, container_name)?,
            }),
            None => Ok(TestEnvironment::Local),
        },
        Some("local") => Ok(TestEnvironment::Local),
        Some("docker") => {
            let (container_name_env_var, container_name) = match docker_container {
                Some(container_name) => (DOCKER_CONTAINER_ENV_VAR, container_name),
                None => (
                    LEGACY_REMOTE_ENV_ENV_VAR,
                    legacy_remote_environment.ok_or_else(|| {
                        format!(
                            "{DOCKER_CONTAINER_ENV_VAR} must be set when {TEST_ENVIRONMENT_ENV_VAR}=docker"
                        )
                    })?,
                ),
            };
            Ok(TestEnvironment::Docker {
                container_name: non_empty_utf8(container_name_env_var, container_name)?,
            })
        }
        Some("wine-exec") => Ok(TestEnvironment::WineExec),
        Some(value) => Err(format!(
            "{TEST_ENVIRONMENT_ENV_VAR} must be one of local, docker, or wine-exec; got {value:?}"
        )),
    }
}

fn non_empty_utf8(name: &str, value: &OsStr) -> Result<String, String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{name} must contain valid UTF-8"))?;
    if value.trim().is_empty() {
        return Err(format!("{name} must not be empty"));
    }
    Ok(value.to_string())
}

#[cfg(test)]
#[path = "test_environment_tests.rs"]
mod tests;
