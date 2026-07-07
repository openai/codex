use std::io::Write;

use super::*;
use crate::git_command::GitRunner;
use pretty_assertions::assert_eq;

fn init_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("repository");
    let mut command = std::process::Command::new("git");
    crate::safe_git::isolate_git_command_environment(&mut command);
    let output = command
        .args(["init", "-q"])
        .current_dir(repo.path())
        .output()
        .expect("initialize repository");
    assert!(
        output.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    repo
}

#[tokio::test]
async fn status_config_normalizes_only_implicit_boolean_keys() {
    let repo = init_repo();
    let config_path = repo.path().join(".git/config");
    let mut config = std::fs::OpenOptions::new()
        .append(true)
        .open(&config_path)
        .expect("open repository config");
    writeln!(config, "[core]").expect("start core section");
    for key in STATUS_IMPLICIT_BOOLEAN_KEYS {
        if let Some(name) = key.strip_prefix("core.") {
            writeln!(config, "\t{name}").expect("append implicit core Boolean");
        }
    }
    writeln!(config, "[index]\n\tsparse").expect("append implicit index Boolean");
    drop(config);

    let git = GitRunner::for_cwd_io(repo.path()).expect("Git runner");
    let entries = read_effective_config_with_implicit_booleans_async(
        &git,
        repo.path(),
        &[],
        STATUS_SAFE_CONFIG_PATTERN,
        "status allowlist test",
        STATUS_IMPLICIT_BOOLEAN_KEYS,
    )
    .await
    .expect("read implicit Status Booleans");
    for key in STATUS_IMPLICIT_BOOLEAN_KEYS {
        assert_eq!(
            entries.get(*key).map(|entry| entry.value.as_str()),
            Some("true"),
            "implicit {key}"
        );
    }

    let mut config = std::fs::OpenOptions::new()
        .append(true)
        .open(&config_path)
        .expect("reopen repository config");
    write!(
        config,
        "[core]\n\tfilemode =\n\tcheckstat\n\teol\n\tcheckroundtripencoding\n\tbigfilethreshold\n\tabbrev\n\
         [attr]\n\ttree\n\
         [index]\n\tversion\n"
    )
    .expect("append explicit empty Boolean and strict implicit values");
    drop(config);

    let explicit_empty = read_effective_config_with_implicit_booleans_async(
        &git,
        repo.path(),
        &[],
        r"^core\.filemode$",
        "explicit empty Boolean test",
        STATUS_IMPLICIT_BOOLEAN_KEYS,
    )
    .await
    .expect("read explicit empty Boolean");
    assert_eq!(
        explicit_empty
            .get("core.filemode")
            .map(|entry| entry.value.as_str()),
        Some("")
    );

    for key in [
        "attr.tree",
        "core.checkstat",
        "core.eol",
        "core.checkroundtripencoding",
        "core.bigfilethreshold",
        "core.abbrev",
        "index.version",
    ] {
        let pattern = format!(r"^{}$", key.replace('.', r"\."));
        let error = read_effective_config_with_implicit_booleans_async(
            &git,
            repo.path(),
            &[],
            &pattern,
            "strict implicit Status value test",
            STATUS_IMPLICIT_BOOLEAN_KEYS,
        )
        .await
        .expect_err("non-Boolean implicit Status value must remain invalid");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData, "implicit {key}");
    }
}
