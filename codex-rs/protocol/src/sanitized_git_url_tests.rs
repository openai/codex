use super::SanitizedGitUrl;
use crate::protocol::GitInfo;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use serde::de::value::Error;
use serde::de::value::StrDeserializer;
use serde_json::json;

/// HTTPS usernames and passwords must never survive construction.
#[test]
fn removes_https_username_and_password() {
    let url = SanitizedGitUrl::try_from("https://alice:secret@github.com/org/repo.git")
        .expect("parse remote URL");

    assert_eq!(url.as_str(), "https://github.com/org/repo.git");
}

#[test]
fn http_query_and_fragment_are_outside_the_authority() {
    for (scheme, suffix) in [("http", "?@github.com/a/b"), ("https", "#@github.com/a/b")] {
        let expected = format!("{scheme}://example.com{suffix}");
        for authority in ["example.com", "user:password@example.com"] {
            let remote = format!("{scheme}://{authority}{suffix}");
            let sanitized = SanitizedGitUrl::try_from(remote.as_str()).expect("parse remote URL");
            assert_eq!(sanitized.as_str(), expected);
        }
    }
}

#[test]
fn non_http_credentials_can_contain_query_and_fragment_delimiters() {
    for scheme in ["ssh", "git", "file"] {
        let expected = format!("{scheme}://example.com/org/repo");
        for password in ["synthetic?value", "synthetic#value"] {
            let remote = format!("{scheme}://user:{password}@example.com/org/repo");
            let sanitized = SanitizedGitUrl::try_from(remote.as_str()).expect("parse remote URL");
            assert_eq!(sanitized.as_str(), expected);
        }
    }
}

/// Tokens placed in the username field are credentials too.
#[test]
fn removes_https_username_without_password() {
    let url = SanitizedGitUrl::try_from("https://secret-token@github.com/org/repo.git")
        .expect("parse remote URL");

    assert_eq!(url.as_str(), "https://github.com/org/repo.git");
}

/// Percent-encoded credentials must not bypass git-aware URL parsing.
#[test]
fn removes_percent_encoded_credentials() {
    let url = SanitizedGitUrl::try_from(
        "https://alice%40example.com:secret%3Atoken@github.com/org/repo.git",
    )
    .expect("parse remote URL");

    assert_eq!(url.as_str(), "https://github.com/org/repo.git");
}

/// File URLs must lose credentials even though gix stores their userinfo as host text.
#[test]
fn removes_file_url_credentials() {
    for (remote, expected) in [
        (
            "file://alice:secret@localhost/repo.git",
            "file://localhost/repo.git",
        ),
        (
            "file://secret-token@server/share/a%2Fb.git",
            "file://server/share/a%2Fb.git",
        ),
        (
            "file://git:secret@server/share/repo.git",
            "file://server/share/repo.git",
        ),
    ] {
        let url = SanitizedGitUrl::try_from(remote).expect("parse file remote URL");

        assert_eq!(url.as_str(), expected);
    }
}

/// Git remote-helper transports preserve their prefix and sanitize nested URLs.
#[test]
fn sanitizes_remote_helper_urls() {
    for (remote, expected) in [
        (
            "hg::https://example.invalid/org/repo.git",
            "hg::https://example.invalid/org/repo.git",
        ),
        (
            "hg::https://alice:secret@example.invalid/org/repo.git",
            "hg::https://example.invalid/org/repo.git",
        ),
        (
            "remote-hg::git@github.com:org/repo.git",
            "remote-hg::git@github.com:org/repo.git",
        ),
        (
            "remote-hg::file://alice:secret@server/share/repo.git",
            "remote-hg::file://server/share/repo.git",
        ),
    ] {
        let url = SanitizedGitUrl::try_from(remote).expect("parse remote helper URL");

        assert_eq!(url.as_str(), expected);
    }
}

/// Nested helper prefixes must not overflow the stack or repeatedly rebuild the remote.
#[test]
fn sanitizes_deeply_nested_remote_helpers_without_recursion() {
    let prefix = "hg::".repeat(4_096);
    let remote = format!("{prefix}https://alice:secret@example.invalid/org/repo.git");
    let expected = format!("{prefix}https://example.invalid/org/repo.git");

    let url = SanitizedGitUrl::try_from(remote.as_str()).expect("parse nested remote helper URL");

    assert_eq!(url.as_str(), expected);
}

/// Opaque helper command lines can hide credentials that Git URL parsing cannot identify.
#[test]
fn rejects_remote_helper_command_payloads() {
    for remote in [
        "ext::sshpass -p secret-token ssh alice@example.com git-upload-pack /repo",
        "ext::ssh alice:secret-token@example.com git-upload-pack /repo",
    ] {
        let error = SanitizedGitUrl::try_from(remote).expect_err("reject helper command payload");

        assert_eq!(error, "invalid git remote URL");
    }
}

/// Removing credentials must not decode reserved characters in repository paths.
#[test]
fn preserves_percent_encoded_path_delimiters() {
    for (remote, expected) in [
        (
            "https://example.invalid/org/a%2Fb%3Fc%23d.git",
            "https://example.invalid/org/a%2Fb%3Fc%23d.git",
        ),
        (
            "https://alice:secret@example.invalid/org/a%2Fb%3Fc%23d.git",
            "https://example.invalid/org/a%2Fb%3Fc%23d.git",
        ),
        (
            "hg::https://alice:secret@example.invalid/org/a%2Fb.git",
            "hg::https://example.invalid/org/a%2Fb.git",
        ),
    ] {
        let url = SanitizedGitUrl::try_from(remote).expect("parse percent-encoded remote URL");

        assert_eq!(url.as_str(), expected);
    }
}

/// The conventional non-secret SSH username is preserved in both Git URL forms.
#[test]
fn preserves_git_ssh_username() {
    for remote in [
        "git@github.com:org/repo.git",
        "ssh://git@github.com/org/repo.git",
    ] {
        let url = SanitizedGitUrl::try_from(remote).expect("parse remote URL");

        assert_eq!(url.as_str(), remote);
    }
}

/// SSH passwords are always removed even for the preserved `git` username.
#[test]
fn removes_ssh_password_for_git_username() {
    let url = SanitizedGitUrl::try_from("ssh://git:secret@github.com/org/repo.git")
        .expect("parse remote URL");

    assert_eq!(url.as_str(), "ssh://git@github.com/org/repo.git");
}

/// Only the exact SSH username `git` is exempt from credential removal.
#[test]
fn removes_other_ssh_usernames() {
    for (remote, expected) in [
        ("alice@github.com:org/repo.git", "github.com:org/repo.git"),
        (
            "ssh://alice@github.com/org/repo.git",
            "ssh://github.com/org/repo.git",
        ),
        (
            "https://git@github.com/org/repo.git",
            "https://github.com/org/repo.git",
        ),
    ] {
        let url = SanitizedGitUrl::try_from(remote).expect("parse remote URL");

        assert_eq!(url.as_str(), expected);
    }
}

/// gix-url misidentifies an IPv6 colon as the SCP path separator when a user precedes the host.
#[test]
fn sanitizes_scp_style_ipv6_remotes_with_usernames() {
    for (remote, expected) in [
        (
            "git@[2001:db8::1]:org/repo.git",
            "git@[2001:db8::1]:org/repo.git",
        ),
        (
            "alice@[2001:db8::1]:org/repo.git",
            "[2001:db8::1]:org/repo.git",
        ),
    ] {
        let url = SanitizedGitUrl::try_from(remote).expect("parse SCP-style IPv6 remote URL");

        assert_eq!(url.as_str(), expected);
    }
}

/// IPv6 fallback validation must not interpret opaque SCP paths as URL paths.
#[test]
fn preserves_opaque_scp_style_ipv6_paths() {
    for (remote, expected) in [
        ("alice@[::1]:dir name/repo", "[::1]:dir name/repo"),
        ("alice@[::1]:repo%FF.git", "[::1]:repo%FF.git"),
        ("git@[::1]:dir name/repo", "git@[::1]:dir name/repo"),
    ] {
        let url = SanitizedGitUrl::try_from(remote).expect("parse opaque SCP-style IPv6 path");

        assert_eq!(url.as_str(), expected);
    }
}

/// Invalid remotes fail closed without including their embedded secret in errors.
#[test]
fn rejects_malformed_urls_without_exposing_input() {
    let error = SanitizedGitUrl::try_from("https://alice:secret-token@[invalid")
        .expect_err("reject malformed remote URL");

    assert_eq!(error, "invalid git remote URL");
    assert!(!error.contains("secret-token"));
}

/// Deserializing persisted metadata must enforce the constructor invariant.
#[test]
fn deserialization_sanitizes_credentials() {
    let deserializer =
        StrDeserializer::<Error>::new("https://alice:secret-token@github.com/org/repo.git");
    let url = SanitizedGitUrl::deserialize(deserializer).expect("deserialize remote URL");

    assert_eq!(url.as_str(), "https://github.com/org/repo.git");
}

/// Ordinary HTTPS remotes retain their existing wire representation.
#[test]
fn preserves_https_remote_without_credentials() {
    let remote = "https://github.com/org/repo.git";
    let url = SanitizedGitUrl::try_from(remote.to_string()).expect("parse remote URL");

    assert_eq!(String::from(url), remote);
}

/// Legacy rollout origins must be sanitized as soon as their protocol metadata is parsed.
#[test]
fn git_info_deserialization_sanitizes_legacy_remote_credentials() {
    let info: GitInfo = serde_json::from_value(json!({
        "repository_url": "https://alice:secret-token@github.com/org/repo.git",
    }))
    .expect("deserialize legacy Git metadata");

    assert_eq!(
        info.repository_url.as_deref(),
        Some("https://github.com/org/repo.git")
    );
}

/// An invalid historical remote must not make its entire rollout impossible to resume.
#[test]
fn git_info_deserialization_discards_malformed_legacy_remote() {
    let info: GitInfo = serde_json::from_value(json!({
        "repository_url": "https://alice:secret-token@[invalid",
    }))
    .expect("deserialize Git metadata despite malformed legacy remote");

    assert_eq!(info.repository_url, None);
}

/// Older rollout metadata may omit repository URLs entirely.
#[test]
fn git_info_deserialization_accepts_missing_repository_url() {
    let info: GitInfo = serde_json::from_value(json!({}))
        .expect("deserialize legacy Git metadata without a repository URL");

    assert_eq!(info.repository_url, None);
}

/// Explicitly absent repository URLs retain their existing protocol representation.
#[test]
fn git_info_deserialization_accepts_null_repository_url() {
    let info: GitInfo = serde_json::from_value(json!({ "repository_url": null }))
        .expect("deserialize legacy Git metadata with a null repository URL");

    assert_eq!(info.repository_url, None);
}
