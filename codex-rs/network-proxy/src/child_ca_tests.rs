use super::*;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
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
    fs::read_to_string(Path::new(
        env.get(REQUESTS_CA_BUNDLE_ENV_KEY)
            .expect("REQUESTS_CA_BUNDLE should be set"),
    ))
    .unwrap()
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
fn does_not_restore_filtered_startup_override() {
    let dir = tempdir().unwrap();
    let mitm_ca_trust_bundle = test_mitm_ca_trust_bundle(
        &dir,
        HashMap::from([(REQUESTS_CA_BUNDLE_ENV_KEY, "startup-ca.pem".to_string())]),
    );
    let mut env = requests_ca_bundle_env(mitm_ca_trust_bundle.path.display().to_string());

    let bundle_paths =
        prepare_mitm_ca_trust_bundle_env(&mitm_ca_trust_bundle, &mut env, dir.path(), &[], |_| {
            true
        });

    assert_eq!(
        env.get(REQUESTS_CA_BUNDLE_ENV_KEY),
        Some(&mitm_ca_trust_bundle.path.display().to_string())
    );
    assert_eq!(bundle_paths.len(), 1);
}

#[test]
fn materializes_readable_command_scoped_override() {
    let dir = tempdir().unwrap();
    let command_ca_bundle_path = dir.path().join("command-ca.pem");
    fs::write(&command_ca_bundle_path, "command ca\n").unwrap();
    let mut env = requests_ca_bundle_env("command-ca.pem");
    let mitm_ca_trust_bundle = test_mitm_ca_trust_bundle(&dir, HashMap::new());

    prepare_mitm_ca_trust_bundle_env(&mitm_ca_trust_bundle, &mut env, dir.path(), &[], |_| true);

    assert_eq!(
        requests_ca_bundle_contents(&env),
        "command ca\nmanaged ca\n"
    );
}

#[test]
fn materializes_readable_ssl_cert_dir() {
    let dir = tempdir().unwrap();
    let ssl_cert_dir_paths = [dir.path().join("certs-a"), dir.path().join("certs-b")];
    for (path, contents) in ssl_cert_dir_paths.iter().zip(["dir ca a\n", "dir ca b\n"]) {
        fs::create_dir(path).unwrap();
        fs::write(path.join("ordinary-ca.pem"), contents).unwrap();
    }
    let mitm_ca_trust_bundle_path = dir.path().join("ca-bundle.pem");
    fs::write(&mitm_ca_trust_bundle_path, "managed ca\n").unwrap();
    let ssl_cert_dir = std::env::join_paths(["certs-a", "certs-b"]).unwrap();
    let mut env = HashMap::from([
        (
            "SSL_CERT_FILE".to_string(),
            mitm_ca_trust_bundle_path.display().to_string(),
        ),
        (
            crate::certs::SSL_CERT_DIR_ENV_KEY.to_string(),
            ssl_cert_dir.to_string_lossy().into_owned(),
        ),
    ]);
    let mitm_ca_trust_bundle = ManagedMitmCaTrustBundle {
        path: mitm_ca_trust_bundle_path,
        startup_env_values: HashMap::from([(
            crate::certs::SSL_CERT_DIR_ENV_KEY,
            ssl_cert_dir.to_string_lossy().into_owned(),
        )]),
        startup_cwd: dir.path().to_path_buf(),
    };

    prepare_mitm_ca_trust_bundle_env(
        &mitm_ca_trust_bundle,
        &mut env,
        dir.path(),
        &[crate::certs::SSL_CERT_DIR_ENV_KEY],
        |_| true,
    );

    let ssl_cert_file_path = Path::new(
        env.get("SSL_CERT_FILE")
            .expect("SSL_CERT_FILE should be set"),
    );
    assert_eq!(
        fs::read_to_string(ssl_cert_file_path).unwrap(),
        "dir ca a\ndir ca b\nmanaged ca\n"
    );
    assert_eq!(env.get(crate::certs::SSL_CERT_DIR_ENV_KEY), None);
}

#[test]
fn bounds_aggregate_ssl_cert_dir_contents() {
    let dir = tempdir().unwrap();
    let ssl_cert_dir_paths = [dir.path().join("certs-a"), dir.path().join("certs-b")];
    for path in &ssl_cert_dir_paths {
        fs::create_dir(path).unwrap();
        fs::write(path.join("ordinary-ca.pem"), "a".repeat(2_200_000)).unwrap();
    }
    let mitm_ca_trust_bundle_path = dir.path().join("ca-bundle.pem");
    fs::write(&mitm_ca_trust_bundle_path, "managed ca\n").unwrap();
    let ssl_cert_dir = std::env::join_paths(["certs-a", "certs-b"]).unwrap();
    let mut env = HashMap::from([
        (
            "SSL_CERT_FILE".to_string(),
            mitm_ca_trust_bundle_path.display().to_string(),
        ),
        (
            crate::certs::SSL_CERT_DIR_ENV_KEY.to_string(),
            ssl_cert_dir.to_string_lossy().into_owned(),
        ),
    ]);
    let mitm_ca_trust_bundle = ManagedMitmCaTrustBundle {
        path: mitm_ca_trust_bundle_path,
        startup_env_values: HashMap::from([(
            crate::certs::SSL_CERT_DIR_ENV_KEY,
            ssl_cert_dir.to_string_lossy().into_owned(),
        )]),
        startup_cwd: dir.path().to_path_buf(),
    };

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
    assert_eq!(env.get(crate::certs::SSL_CERT_DIR_ENV_KEY), None);
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

#[test]
fn does_not_whitelist_existing_generated_bundle_override() {
    let dir = tempdir().unwrap();
    let generated_ca_bundle_path = dir.path().join("ca-bundle-handcrafted.pem");
    fs::write(&generated_ca_bundle_path, "extra ca\nmanaged ca\n").unwrap();
    let mut env = requests_ca_bundle_env(generated_ca_bundle_path.display().to_string());
    let mitm_ca_trust_bundle = test_mitm_ca_trust_bundle(&dir, HashMap::new());

    let bundle_paths =
        prepare_mitm_ca_trust_bundle_env(&mitm_ca_trust_bundle, &mut env, dir.path(), &[], |_| {
            false
        });

    assert_eq!(
        env.get(REQUESTS_CA_BUNDLE_ENV_KEY),
        Some(&generated_ca_bundle_path.display().to_string())
    );
    assert!(bundle_paths.is_empty());
}
