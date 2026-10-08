//! Preserve native ACL failure causes in both refresh summaries and setup logs.
//! Recording a diagnostic never changes whether a failed ACL operation is fatal.

use anyhow::Error;
use anyhow::Result;
use std::path::Path;

pub(crate) enum WriteAclOperation {
    Check,
    Grant,
    Deny,
}

pub(crate) fn record_acl_failure(
    errors: &mut Vec<String>,
    operation: WriteAclOperation,
    path: &Path,
    error: &Error,
    write_log: impl FnOnce(&str) -> Result<()>,
) -> Result<()> {
    let detail = format!("{}: {error:#}", path.display());
    let (summary, log) = match operation {
        WriteAclOperation::Check => (
            format!("write ACE check failed on {detail}"),
            format!("write ACE check failed on {detail}; continuing"),
        ),
        WriteAclOperation::Grant => (
            format!("write ACE failed on {detail}"),
            format!("write ACE grant failed on {detail}"),
        ),
        WriteAclOperation::Deny => (
            format!("deny ACE failed on {detail}"),
            format!("deny ACE failed on {detail}"),
        ),
    };
    // Retain the ACL failure even if writing its individual log line fails.
    errors.push(summary);
    write_log(&log)
}

#[cfg(test)]
#[path = "setup_acl_error_tests.rs"]
mod tests;
