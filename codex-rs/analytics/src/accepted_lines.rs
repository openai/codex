use crate::events::CodexAcceptedLineFingerprintsEventParams;
use crate::events::CodexAcceptedLineFingerprintsEventRequest;
use crate::events::TrackEventRequest;
use crate::facts::AcceptedLineFingerprint;
use codex_git_utils::canonicalize_git_remote_url;
use codex_git_utils::get_git_remote_urls_assume_git_repo;
use sha1::Digest;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedLineFingerprintSummary {
    pub accepted_added_lines: u64,
    pub accepted_deleted_lines: u64,
    pub line_fingerprints: Vec<AcceptedLineFingerprint>,
}

pub(crate) struct AcceptedLineFingerprintEventInput {
    pub(crate) event_type: &'static str,
    pub(crate) turn_id: String,
    pub(crate) thread_id: String,
    pub(crate) product_surface: Option<String>,
    pub(crate) model_slug: Option<String>,
    pub(crate) completed_at: u64,
    pub(crate) repo_hash: Option<String>,
    pub(crate) accepted_added_lines: u64,
    pub(crate) accepted_deleted_lines: u64,
    pub(crate) line_fingerprints: Vec<AcceptedLineFingerprint>,
}

pub fn accepted_line_fingerprints_from_unified_diff(
    unified_diff: &str,
) -> AcceptedLineFingerprintSummary {
    let mut current_path: Option<String> = None;
    let mut in_hunk = false;
    let mut accepted_added_lines = 0;
    let mut accepted_deleted_lines = 0;
    let mut line_fingerprints = Vec::new();

    for line in unified_diff.lines() {
        if line.starts_with("diff --git ") {
            current_path = None;
            in_hunk = false;
            continue;
        }

        if line.starts_with("@@ ") {
            in_hunk = true;
            continue;
        }

        if !in_hunk && let Some(path) = line.strip_prefix("+++ ") {
            current_path = normalize_diff_path(path);
            continue;
        }

        if !in_hunk && line.starts_with("--- ") {
            continue;
        }

        if let Some(added_line) = line.strip_prefix('+') {
            accepted_added_lines += 1;
            if let Some(path) = current_path.as_deref()
                && let Some(normalized_line) = normalize_effective_line(added_line)
            {
                line_fingerprints.push(AcceptedLineFingerprint {
                    path_hash: fingerprint_hash("path", path),
                    line_hash: fingerprint_hash("line", &normalized_line),
                });
            }
            continue;
        }

        if line.starts_with('-') {
            accepted_deleted_lines += 1;
        }
    }

    AcceptedLineFingerprintSummary {
        accepted_added_lines,
        accepted_deleted_lines,
        line_fingerprints,
    }
}

pub fn fingerprint_hash(domain: &str, value: &str) -> String {
    let mut hasher = sha1::Sha1::new();
    hasher.update(b"file-line-v1\0");
    hasher.update(domain.as_bytes());
    hasher.update(b"\0");
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) fn accepted_line_fingerprint_event_requests(
    input: AcceptedLineFingerprintEventInput,
) -> Vec<TrackEventRequest> {
    let AcceptedLineFingerprintEventInput {
        event_type,
        turn_id,
        thread_id,
        product_surface,
        model_slug,
        completed_at,
        repo_hash,
        accepted_added_lines,
        accepted_deleted_lines,
        line_fingerprints: _line_fingerprints,
    } = input;

    vec![TrackEventRequest::AcceptedLineFingerprints(Box::new(
        CodexAcceptedLineFingerprintsEventRequest {
            event_type: "codex_accepted_line_fingerprints",
            event_params: CodexAcceptedLineFingerprintsEventParams {
                event_type,
                turn_id,
                thread_id,
                product_surface,
                model_slug,
                completed_at,
                repo_hash,
                accepted_added_lines,
                accepted_deleted_lines,
                // Keep computing local fingerprints for parsing tests and future attribution,
                // but do not upload path/line hashes in the analytics event payload.
                line_fingerprints: Vec::new(),
            },
        },
    ))]
}

pub async fn accepted_line_repo_hash_for_cwd(cwd: &Path) -> Option<String> {
    let remotes = get_git_remote_urls_assume_git_repo(cwd).await?;
    remotes
        .get("origin")
        .or_else(|| remotes.values().next())
        .map(|remote_url| {
            let canonical_remote_url =
                canonicalize_git_remote_url(remote_url).unwrap_or_else(|| remote_url.to_string());
            fingerprint_hash("repo", &canonical_remote_url)
        })
}

fn normalize_diff_path(path: &str) -> Option<String> {
    let path = path.trim();
    if path == "/dev/null" {
        return None;
    }

    Some(
        path.strip_prefix("b/")
            .or_else(|| path.strip_prefix("a/"))
            .unwrap_or(path)
            .to_string(),
    )
}

fn normalize_effective_line(line: &str) -> Option<String> {
    let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.len() <= 3 {
        return None;
    }
    if !normalized
        .chars()
        .any(|ch| ch.is_alphanumeric() || ch == '_')
    {
        return None;
    }
    Some(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_counts_and_effective_added_fingerprints() {
        let diff = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,5 @@
-old line
+fn useful() {
+}
+    return user.id;
 context
";

        let summary = accepted_line_fingerprints_from_unified_diff(diff);

        assert_eq!(
            summary,
            AcceptedLineFingerprintSummary {
                accepted_added_lines: 3,
                accepted_deleted_lines: 1,
                line_fingerprints: vec![
                    AcceptedLineFingerprint {
                        path_hash: fingerprint_hash("path", "src/lib.rs"),
                        line_hash: fingerprint_hash("line", "fn useful() {"),
                    },
                    AcceptedLineFingerprint {
                        path_hash: fingerprint_hash("path", "src/lib.rs"),
                        line_hash: fingerprint_hash("line", "return user.id;"),
                    },
                ],
            }
        );
    }

    #[test]
    fn skips_added_file_metadata_headers() {
        let diff = "\
diff --git a/new.py b/new.py
new file mode 100644
index 0000000..1111111
--- /dev/null
+++ b/new.py
@@ -0,0 +1 @@
+print('hello')
";

        let summary = accepted_line_fingerprints_from_unified_diff(diff);

        assert_eq!(summary.accepted_added_lines, 1);
        assert_eq!(summary.accepted_deleted_lines, 0);
        assert_eq!(summary.line_fingerprints.len(), 1);
    }

    #[test]
    fn parses_hunk_lines_that_look_like_file_headers() {
        let diff = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,2 +1,2 @@
--- old value
+++ new value
";

        let summary = accepted_line_fingerprints_from_unified_diff(diff);

        assert_eq!(
            summary,
            AcceptedLineFingerprintSummary {
                accepted_added_lines: 1,
                accepted_deleted_lines: 1,
                line_fingerprints: vec![AcceptedLineFingerprint {
                    path_hash: fingerprint_hash("path", "src/lib.rs"),
                    line_hash: fingerprint_hash("line", "++ new value"),
                }],
            }
        );
    }
}
