use super::Redactor;
use super::truncate_chars;
use std::path::Path;

#[test]
fn redactor_removes_roots_credentials_query_and_deep_url_paths() {
    let redactor = Redactor::new(
        Path::new("/private/customer/marketplace"),
        Path::new("/tmp/diagnostic-home"),
    );
    let detail = redactor.detail(
        "failed in /private/customer/marketplace/plugins/demo via \
         https://user:secret@example.com/private/repo.git?token=secret and \
         git@github.example.com:private/repo.git and \
         /outside/customer/path and \
         /tmp/diagnostic-home/plugins/cache; \
         api_key=sk-123456789012345678901234567890",
    );

    assert!(detail.contains("<marketplace-root>/plugins/demo"));
    assert!(detail.contains("<diagnostic-home>/plugins/cache"));
    assert!(detail.contains("<redacted-url>"));
    assert!(detail.contains("<git-remote>:<redacted>"));
    assert!(!detail.contains("/outside/customer/path"));
    assert!(!detail.contains("user:secret"));
    assert!(!detail.contains("token=secret"));
    assert!(!detail.contains("sk-123456789012345678901234567890"));
    assert!(detail.contains("api_key=[REDACTED_SECRET]"));
}

#[test]
fn detail_truncation_is_bounded() {
    let truncated = truncate_chars("x".repeat(20), 8);
    assert_eq!(truncated, "xxxxxxxx...[truncated]");
}

#[test]
fn redactor_removes_git_command_arguments_and_remote_output() {
    let redactor = Redactor::new(
        Path::new("/private/customer/marketplace"),
        Path::new("/tmp/diagnostic-home"),
    );
    let detail = redactor.detail(
        "git checkout private-ref\nsecond-private-ref \
         /tmp/diagnostic-home/staging failed with status exit status: 128\n\
         stdout:\n\
         private account output\n\
         stderr:\n\
         authentication failed",
    );

    assert!(detail.contains("git <redacted arguments> failed with status exit status: 128"));
    assert!(detail.contains("failure class: authentication_or_authorization"));
    assert!(detail.contains("git output omitted"));
    assert!(!detail.contains("private-ref"));
    assert!(!detail.contains("private account output"));
    assert!(!detail.contains("authentication failed"));
}

#[test]
fn redactor_removes_parser_values_and_private_hosts() {
    let redactor = Redactor::new(
        Path::new("/private/customer/marketplace"),
        Path::new("/tmp/diagnostic-home"),
    );
    let detail = redactor.detail(
        "invalid type: integer `8675309`; unknown variant `private-transport`; fetch from \
         https://internal.example/private/config failed",
    );

    assert!(detail.contains("integer <redacted-value>"));
    assert!(detail.contains("variant <redacted-value>"));
    assert!(detail.contains("<redacted-url>"));
    assert!(!detail.contains("8675309"));
    assert!(!detail.contains("private-transport"));
    assert!(!detail.contains("internal.example"));
}
