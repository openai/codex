use super::*;

#[test]
fn unreadable_metadata_is_invalid_not_a_proven_policy_crossing() {
    let marker = PathBuf::from("repository/.git/commondir");
    let error = super::helpers::invalid_metadata(
        &marker,
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "metadata marker is unreadable",
        ),
    );
    assert_eq!(
        error,
        crate::errors::GitReadError::InvalidRepositoryMetadata {
            path: marker,
            reason: "metadata marker is unreadable".to_string(),
        }
    );
    assert_eq!(error.io_kind(), io::ErrorKind::InvalidData);
}

fn write_common_config(body: &[u8]) -> tempfile::TempDir {
    let common = tempfile::tempdir().expect("common dir");
    std::fs::write(common.path().join("config"), body).expect("write common config");
    common
}

fn git_config_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    #[cfg(windows)]
    let path = path.replace('\\', "/");
    #[cfg(not(windows))]
    let path = path.into_owned();
    format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""))
}

fn native_bare_value(config: &Path, includes: bool) -> io::Result<Option<bool>> {
    let mut command = std::process::Command::new("git");
    crate::safe_git::isolate_git_command_environment(&mut command);
    command
        .args(["config", "--file"])
        .arg(config)
        .arg(if includes {
            "--includes"
        } else {
            "--no-includes"
        })
        .args(["--type=bool", "--get", "core.bare"]);
    let output = command.output()?;
    match output.status.code() {
        Some(0) => Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim() == "true",
        )),
        Some(1) if output.stdout.is_empty() => Ok(None),
        _ => Err(io::Error::other(format!(
            "native core.bare query failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))),
    }
}

#[test]
fn plain_common_bare_parser_matches_native_boolean_syntax_and_is_stricter_on_duplicates() {
    for (name, body, expected, native) in [
        (
            "explicit true",
            b"[core]\n\tbare = true\n".as_slice(),
            CommonConfigAuthority::Bare,
            Some(true),
        ),
        (
            "implicit true",
            b"[core]\n\tbare\n".as_slice(),
            CommonConfigAuthority::Unproven,
            Some(true),
        ),
        (
            "quoted true",
            b"[core]\n\tbare = \"true\"\n".as_slice(),
            CommonConfigAuthority::Bare,
            Some(true),
        ),
        (
            "false",
            b"[core]\n\tbare = false\n".as_slice(),
            CommonConfigAuthority::Unproven,
            Some(false),
        ),
        (
            "duplicate last true",
            b"[core]\n\tbare = false\n\tbare = true\n".as_slice(),
            CommonConfigAuthority::Unproven,
            Some(true),
        ),
        (
            "duplicate last false",
            b"[core]\n\tbare = true\n\tbare = false\n".as_slice(),
            CommonConfigAuthority::Unproven,
            Some(false),
        ),
        (
            "mixed implicit then explicit",
            b"[core]\n\tbare\n\tbare = true\n".as_slice(),
            CommonConfigAuthority::Unproven,
            Some(true),
        ),
        (
            "mixed explicit then implicit",
            b"[core]\n\tbare = false\n\tbare\n".as_slice(),
            CommonConfigAuthority::Unproven,
            Some(true),
        ),
        (
            "mixed case duplicate",
            b"[core]\n\tbare = false\n\tBaRe = true\n".as_slice(),
            CommonConfigAuthority::Unproven,
            Some(true),
        ),
    ] {
        let common = write_common_config(body);
        assert_eq!(
            inspect_plain_common_config_authority(common.path()).expect(name),
            expected,
            "authority result for {name}"
        );
        assert_eq!(
            native_bare_value(&common.path().join("config"), /*includes*/ false).expect(name),
            native,
            "native result for {name}"
        );
    }
}

#[test]
fn common_bare_proof_accepts_only_explicit_boolean_literals() {
    for (name, body) in [
        ("empty", b"[core]\n\tbare =\n".as_slice()),
        ("yes", b"[core]\n\tbare = yes\n".as_slice()),
        ("on", b"[core]\n\tbare = on\n".as_slice()),
        ("one", b"[core]\n\tbare = 1\n".as_slice()),
        ("leading zero", b"[core]\n\tbare = 08\n".as_slice()),
        (
            "positive overflow",
            b"[core]\n\tbare = 2147483648\n".as_slice(),
        ),
        (
            "negative overflow",
            b"[core]\n\tbare = -2147483649\n".as_slice(),
        ),
        (
            "i64 max",
            b"[core]\n\tbare = 9223372036854775807\n".as_slice(),
        ),
    ] {
        let common = write_common_config(body);
        assert_eq!(
            inspect_plain_common_config_authority(common.path()).expect(name),
            CommonConfigAuthority::Unproven,
            "authority result for {name}"
        );
    }
}

#[test]
fn common_bare_proof_rejects_includes_worktree_config_and_relative_worktree() {
    let include_target = tempfile::NamedTempFile::new().expect("included config");
    std::fs::write(include_target.path(), "[core]\n\tbare = false\n")
        .expect("write included config");
    let common = write_common_config(
        format!(
            "[core]\n\tbare = true\n[include]\n\tpath = {}\n",
            git_config_path(include_target.path())
        )
        .as_bytes(),
    );
    assert_eq!(
        native_bare_value(&common.path().join("config"), /*includes*/ false)
            .expect("direct native bare"),
        Some(true)
    );
    assert_eq!(
        native_bare_value(&common.path().join("config"), /*includes*/ true)
            .expect("included native bare"),
        Some(false)
    );
    assert_eq!(
        inspect_plain_common_config_authority(common.path()).expect("include authority"),
        CommonConfigAuthority::Unproven
    );

    for body in [
        b"[core]\n\tbare = true\n[includeIf \"gitdir:**\"]\n\tpath = /tmp/ignored\n".as_slice(),
        b"[core]\n\tbare = true\n[extensions]\n\tworktreeConfig = true\n".as_slice(),
        b"[core]\n\tbare = true\n\tworktree = relative/path\n".as_slice(),
        b"[core]\n\tbare = true\n[extensions]\n\tworktreeConfig\n".as_slice(),
        b"[core]\n\tbare = true\n[extensions]\n\tworktreeConfig = false\n\tworktreeConfig\n"
            .as_slice(),
        b"[core]\n\tbare = true\n\tworktree\n".as_slice(),
        b"[core]\n\tbare = true\n\tworktree = /tmp/one\n\tworktree\n".as_slice(),
        b"[core \"unexpected\"]\n\tbare = true\n".as_slice(),
        b"[extensions \"unexpected\"]\n\tworktreeConfig = false\n[core]\n\tbare = true\n"
            .as_slice(),
    ] {
        let common = write_common_config(body);
        assert_eq!(
            inspect_plain_common_config_authority(common.path()).expect("ambiguous authority"),
            CommonConfigAuthority::Unproven
        );
    }
}

#[test]
fn common_config_absolute_worktree_is_returned_as_authority() {
    let worktree = tempfile::tempdir().expect("worktree");
    let common = write_common_config(
        format!(
            "[core]\n\tworktree = {}\n",
            git_config_path(worktree.path())
        )
        .as_bytes(),
    );
    assert_eq!(
        inspect_plain_common_config_authority(common.path()).expect("worktree authority"),
        CommonConfigAuthority::Worktree(worktree.path().to_path_buf())
    );
}

#[test]
fn common_config_rejects_contradictory_or_malformed_bare_with_absolute_worktree() {
    let worktree = tempfile::tempdir().expect("worktree");
    for (name, bare) in [("contradictory bare", "true"), ("malformed bare", "08")] {
        let common = write_common_config(
            format!(
                "[core]\n\tbare = {bare}\n\tworktree = {}\n",
                git_config_path(worktree.path())
            )
            .as_bytes(),
        );
        assert_eq!(
            inspect_plain_common_config_authority(common.path()).expect(name),
            CommonConfigAuthority::Unproven,
            "authority result for {name}"
        );
    }
}

#[test]
fn windows_authority_path_grammar_rejects_ambiguous_primary_spellings() {
    for path in [
        r"C:\primary.\repo",
        r"C:\primary \repo",
        r"C:\primary:stream\repo",
        r"C:\NUL\repo",
        r"\\?\GLOBALROOT\Device\HarddiskVolume1\repo",
        r"\??\C:\primary\repo",
        r"C:\outside\..\primary",
    ] {
        assert!(
            crate::path_authority::windows_authority_path_is_ambiguous(path),
            "ambiguous Windows authority path accepted: {path:?}"
        );
    }
    for path in [r"C:\primary\repo", r"\\server\share\primary\repo"] {
        assert!(
            !crate::path_authority::windows_authority_path_is_ambiguous(path),
            "ordinary Windows authority path rejected: {path:?}"
        );
    }
}
