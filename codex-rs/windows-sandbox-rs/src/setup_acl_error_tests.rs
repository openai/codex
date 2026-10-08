use super::WriteAclOperation;
use super::record_acl_failure;
use anyhow::anyhow;
use pretty_assertions::assert_eq;
use std::io;
use std::path::Path;

#[test]
fn records_each_acl_operation_with_its_native_error_chain() {
    let path = Path::new("project/.git");
    let native_error = io::Error::from_raw_os_error(5);
    // The native wording is platform-dependent; the code and cause chain are not.
    let native_message = native_error.to_string();
    let error = anyhow::Error::new(native_error)
        .context("open ACL target (READ_CONTROL | WRITE_DAC)")
        .context("prepare protected path");
    let detail = format!(
        "project/.git: prepare protected path: open ACL target (READ_CONTROL | WRITE_DAC): {native_message}"
    );

    let mut errors = Vec::new();
    let mut log = Vec::new();
    for operation in [
        WriteAclOperation::Check,
        WriteAclOperation::Grant,
        WriteAclOperation::Deny,
    ] {
        record_acl_failure(&mut errors, operation, path, &error, |message| {
            log.push(message.to_owned());
            Ok(())
        })
        .expect("record ACL failure");
    }
    assert_eq!(
        (errors, log),
        (
            vec![
                format!("write ACE check failed on {detail}"),
                format!("write ACE failed on {detail}"),
                format!("deny ACE failed on {detail}"),
            ],
            vec![
                format!("write ACE check failed on {detail}; continuing"),
                format!("write ACE grant failed on {detail}"),
                format!("deny ACE failed on {detail}"),
            ],
        )
    );
}

#[test]
fn log_failure_preserves_acl_failure_and_previous_roots() {
    let mut errors = vec!["previous root failed".to_string()];
    let error = anyhow!("SetSecurityInfo failed: 87").context("grant write ACL");
    let log_error = record_acl_failure(
        &mut errors,
        WriteAclOperation::Grant,
        Path::new("virtual-drive/project"),
        &error,
        |_| Err(anyhow!("log unavailable")),
    )
    .expect_err("report log failure");
    assert_eq!(
        (errors, log_error.to_string()),
        (
            vec![
                "previous root failed".to_string(),
                "write ACE failed on virtual-drive/project: grant write ACL: SetSecurityInfo failed: 87"
                    .to_string(),
            ],
            "log unavailable".to_string(),
        )
    );
}
