use super::*;
use pretty_assertions::assert_eq;
use std::fs;
use tempfile::tempdir;

const REQUESTS_CA_BUNDLE_ENV_KEY: &str = "REQUESTS_CA_BUNDLE";

fn test_mitm_ca_trust_bundle(
    dir: &tempfile::TempDir,
    startup_env_values: HashMap<&'static str, String>,
) -> ManagedMitmCaTrustBundle {
    let path = dir.path().join("ca-bundle.pem");
    fs::write(&path, "managed ca\n").unwrap();
    ManagedMitmCaTrustBundle {
        path,
        startup_env_values,
        startup_cwd: dir.path().to_path_buf(),
    }
}

fn requests_ca_bundle_env(value: impl Into<String>) -> HashMap<String, String> {
    HashMap::from([(REQUESTS_CA_BUNDLE_ENV_KEY.to_string(), value.into())])
}

fn requests_ca_bundle_contents(env: &HashMap<String, String>) -> String {
    fs::read_to_string(&env[REQUESTS_CA_BUNDLE_ENV_KEY]).unwrap()
}

fn ssl_cert_dir_env(
    dir: &tempfile::TempDir,
    contents: [String; 2],
) -> (HashMap<String, String>, ManagedMitmCaTrustBundle) {
    let ssl_cert_dir_paths = [dir.path().join("certs-a"), dir.path().join("certs-b")];
    for (path, contents) in ssl_cert_dir_paths.iter().zip(contents) {
        fs::create_dir(path).unwrap();
        fs::write(path.join("ordinary-ca.pem"), contents).unwrap();
    }
    let mitm_ca_trust_bundle_path = dir.path().join("ca-bundle.pem");
    fs::write(&mitm_ca_trust_bundle_path, "managed ca\n").unwrap();
    let ssl_cert_dir = std::env::join_paths(["certs-a", "certs-b"]).unwrap();
    let ssl_cert_dir = ssl_cert_dir.to_string_lossy().into_owned();
    (
        HashMap::from([
            (
                "SSL_CERT_FILE".to_string(),
                mitm_ca_trust_bundle_path.display().to_string(),
            ),
            (
                REQUESTS_CA_BUNDLE_ENV_KEY.to_string(),
                mitm_ca_trust_bundle_path.display().to_string(),
            ),
            (
                crate::certs::SSL_CERT_DIR_ENV_KEY.to_string(),
                ssl_cert_dir.clone(),
            ),
        ]),
        ManagedMitmCaTrustBundle {
            path: mitm_ca_trust_bundle_path,
            startup_env_values: HashMap::from([(crate::certs::SSL_CERT_DIR_ENV_KEY, ssl_cert_dir)]),
            startup_cwd: dir.path().to_path_buf(),
        },
    )
}

#[test]
fn materializes_readable_startup_ca_override() {
    let dir = tempdir().unwrap();
    let startup_ca_bundle_path = dir.path().join("startup-ca.pem");
    let command_cwd = dir.path().join("command-cwd");
    fs::create_dir(&command_cwd).unwrap();
    fs::write(&startup_ca_bundle_path, "startup ca\n").unwrap();
    let mitm_ca_trust_bundle = test_mitm_ca_trust_bundle(
        &dir,
        HashMap::from([(REQUESTS_CA_BUNDLE_ENV_KEY, "startup-ca.pem".to_string())]),
    );
    let mut env = requests_ca_bundle_env("startup-ca.pem");
    let bundle_paths = prepare_mitm_ca_trust_bundle_env(
        &mitm_ca_trust_bundle,
        &mut env,
        &command_cwd,
        &[REQUESTS_CA_BUNDLE_ENV_KEY],
        |_| true,
    );

    assert_eq!(
        requests_ca_bundle_contents(&env),
        "startup ca\nmanaged ca\n"
    );
    assert_eq!(bundle_paths.len(), 1);
}

#[test]
fn materializes_readable_ssl_cert_dir() {
    let dir = tempdir().unwrap();
    let (mut env, mitm_ca_trust_bundle) =
        ssl_cert_dir_env(&dir, ["dir ca a\n".to_string(), "dir ca b\n".to_string()]);

    prepare_mitm_ca_trust_bundle_env(
        &mitm_ca_trust_bundle,
        &mut env,
        dir.path(),
        &[crate::certs::SSL_CERT_DIR_ENV_KEY],
        |_| true,
    );

    for key in ["SSL_CERT_FILE", REQUESTS_CA_BUNDLE_ENV_KEY] {
        assert_eq!(
            fs::read_to_string(&env[key]).unwrap(),
            "dir ca a\ndir ca b\nmanaged ca\n"
        );
    }
    assert_eq!(env.get(crate::certs::SSL_CERT_DIR_ENV_KEY), None);
}

#[test]
fn bounds_aggregate_ssl_cert_dir_contents() {
    let dir = tempdir().unwrap();
    let oversized_dir_contents = "a".repeat(2_200_000);
    let (mut env, mitm_ca_trust_bundle) = ssl_cert_dir_env(
        &dir,
        [oversized_dir_contents.clone(), oversized_dir_contents],
    );

    prepare_mitm_ca_trust_bundle_env(
        &mitm_ca_trust_bundle,
        &mut env,
        dir.path(),
        &[crate::certs::SSL_CERT_DIR_ENV_KEY],
        |_| true,
    );

    assert_eq!(
        env.get("SSL_CERT_FILE"),
        Some(&mitm_ca_trust_bundle.path.display().to_string())
    );
    assert!(env.contains_key(crate::certs::SSL_CERT_DIR_ENV_KEY));
}

#[test]
fn preserves_unreadable_command_scoped_override() {
    let dir = tempdir().unwrap();
    let command_ca_bundle_path = dir.path().join("command-ca.pem");
    fs::write(&command_ca_bundle_path, "command ca\n").unwrap();
    let mut env = requests_ca_bundle_env("command-ca.pem");
    let mitm_ca_trust_bundle = test_mitm_ca_trust_bundle(&dir, HashMap::new());

    let bundle_paths =
        prepare_mitm_ca_trust_bundle_env(&mitm_ca_trust_bundle, &mut env, dir.path(), &[], |_| {
            false
        });

    assert_eq!(
        env.get(REQUESTS_CA_BUNDLE_ENV_KEY),
        Some(&"command-ca.pem".to_string())
    );
    assert!(bundle_paths.is_empty());
}
