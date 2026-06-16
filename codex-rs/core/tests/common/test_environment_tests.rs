use std::ffi::OsStr;

use pretty_assertions::assert_eq;

use super::*;

#[test]
fn defaults_to_local() {
    assert_eq!(
        parse_test_environment(
            /*configured_environment*/ None, /*legacy_remote_environment*/ None,
            /*docker_container*/ None,
        ),
        Ok(TestEnvironment::Local)
    );
}

#[test]
fn parses_each_explicit_environment() {
    assert_eq!(
        parse_test_environment(
            Some(OsStr::new("local")),
            /*legacy_remote_environment*/ None,
            /*docker_container*/ None,
        ),
        Ok(TestEnvironment::Local)
    );
    assert_eq!(
        parse_test_environment(
            Some(OsStr::new("docker")),
            /*legacy_remote_environment*/ None,
            Some(OsStr::new("container-1")),
        ),
        Ok(TestEnvironment::Docker {
            container_name: "container-1".to_string(),
        })
    );
    assert_eq!(
        parse_test_environment(
            Some(OsStr::new("wine-exec")),
            /*legacy_remote_environment*/ None,
            /*docker_container*/ None,
        ),
        Ok(TestEnvironment::WineExec)
    );
}

#[test]
fn treats_the_legacy_remote_value_as_a_docker_container() {
    assert_eq!(
        parse_test_environment(
            /*configured_environment*/ None,
            Some(OsStr::new("legacy-container")),
            /*docker_container*/ None,
        ),
        Ok(TestEnvironment::Docker {
            container_name: "legacy-container".to_string(),
        })
    );
}

#[test]
fn explicit_docker_accepts_the_legacy_container_value() {
    assert_eq!(
        parse_test_environment(
            Some(OsStr::new("docker")),
            Some(OsStr::new("legacy-container")),
            /*docker_container*/ None,
        ),
        Ok(TestEnvironment::Docker {
            container_name: "legacy-container".to_string(),
        })
    );
    assert_eq!(
        parse_test_environment(
            Some(OsStr::new("docker")),
            Some(OsStr::new("")),
            /*docker_container*/ None,
        ),
        Err(format!("{LEGACY_REMOTE_ENV_ENV_VAR} must not be empty"))
    );
}

#[test]
fn explicit_local_ignores_stale_remote_metadata() {
    assert_eq!(
        parse_test_environment(
            Some(OsStr::new("local")),
            Some(OsStr::new("legacy-container")),
            Some(OsStr::new("container-1")),
        ),
        Ok(TestEnvironment::Local)
    );
}

#[test]
fn rejects_invalid_or_incomplete_configuration() {
    assert_eq!(
        parse_test_environment(
            Some(OsStr::new("docker")),
            /*legacy_remote_environment*/ None,
            /*docker_container*/ None,
        ),
        Err(format!(
            "{DOCKER_CONTAINER_ENV_VAR} must be set when {TEST_ENVIRONMENT_ENV_VAR}=docker"
        ))
    );
    assert_eq!(
        parse_test_environment(
            Some(OsStr::new("other")),
            /*legacy_remote_environment*/ None,
            /*docker_container*/ None,
        ),
        Err(format!(
            "{TEST_ENVIRONMENT_ENV_VAR} must be one of local, docker, or wine-exec; got \"other\""
        ))
    );
}
