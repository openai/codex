use codex_protocol::protocol::ReviewRequest;
use std::cmp::min;

/// Target to review against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewTarget {
    /// Review the working tree: staged, unstaged, and untracked files.
    UncommittedChanges,

    /// Review changes between the current branch and the given base branch.
    BaseBranch { branch: String },

    /// Review the changes introduced by a specific commit.
    Commit {
        sha: String,
        /// Optional human-readable label (e.g., commit subject) for UIs.
        title: Option<String>,
    },

    /// Arbitrary instructions, equivalent to the old free-form prompt.
    Custom { instructions: String },
}

/// Validation errors for review targets.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReviewTargetError {
    #[error("branch must not be empty")]
    EmptyBranch,
    #[error("sha must not be empty")]
    EmptySha,
    #[error("instructions must not be empty")]
    EmptyInstructions,
}

/// Built review request plus a user-visible description of the target.
#[derive(Clone, Debug, PartialEq)]
pub struct BuiltReviewRequest {
    pub review_request: ReviewRequest,
    pub display_text: String,
}

/// Build the review prompt and user-facing hint for uncommitted changes.
pub fn review_uncommitted_prompt() -> (String, String) {
    let prompt = "Review the current code changes (staged, unstaged, and untracked files) and provide prioritized findings.".to_string();
    let hint = "current changes".to_string();
    (prompt, hint)
}

/// Build the review prompt and hint for reviewing against a base branch.
pub fn review_branch_prompt(branch: &str) -> (String, String) {
    let prompt = format!(
        "Review the code changes against the base branch '{branch}'. Start by finding the merge diff between the current branch and {branch}'s upstream e.g. (`git merge-base HEAD \"$(git rev-parse --abbrev-ref \"{branch}@{{upstream}}\")\"`), then run `git diff` against that SHA to see what changes we would merge into the {branch} branch. Provide prioritized, actionable findings."
    );
    let hint = format!("changes against '{branch}'");
    (prompt, hint)
}

/// Build the review prompt and hint for a specific commit.
/// If `subject_opt` is provided, it will be included in the prompt; otherwise it is omitted.
pub fn review_commit_prompt(sha: &str, subject_opt: Option<&str>) -> (String, String) {
    let short: String = sha.chars().take(min(7, sha.len())).collect();
    let prompt = if let Some(subject) = subject_opt {
        format!(
            "Review the code changes introduced by commit {sha} (\"{subject}\"). Provide prioritized, actionable findings."
        )
    } else {
        format!(
            "Review the code changes introduced by commit {sha}. Provide prioritized, actionable findings."
        )
    };
    let hint = format!("commit {short}");
    (prompt, hint)
}

/// Build the review prompt and hint for custom free-form instructions.
pub fn review_custom_prompt(custom: &str) -> (String, String) {
    let prompt = custom.trim().to_string();
    (prompt.clone(), prompt)
}

pub fn review_request_from_target(
    target: ReviewTarget,
    append_to_original_thread: bool,
) -> Result<BuiltReviewRequest, ReviewTargetError> {
    match target {
        ReviewTarget::UncommittedChanges => {
            let (prompt, hint) = review_uncommitted_prompt();
            Ok(BuiltReviewRequest {
                review_request: ReviewRequest {
                    prompt,
                    user_facing_hint: hint,
                    append_to_original_thread,
                },
                display_text: "Review uncommitted changes".to_string(),
            })
        }
        ReviewTarget::BaseBranch { branch } => {
            let branch = branch.trim().to_string();
            if branch.is_empty() {
                return Err(ReviewTargetError::EmptyBranch);
            }
            let (prompt, hint) = review_branch_prompt(&branch);
            Ok(BuiltReviewRequest {
                review_request: ReviewRequest {
                    prompt,
                    user_facing_hint: hint,
                    append_to_original_thread,
                },
                display_text: format!("Review changes against base branch '{branch}'"),
            })
        }
        ReviewTarget::Commit { sha, title } => {
            let sha = sha.trim().to_string();
            if sha.is_empty() {
                return Err(ReviewTargetError::EmptySha);
            }
            let title = title
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty());
            let (prompt, hint) = review_commit_prompt(&sha, title.as_deref());
            let short_sha: String = sha.chars().take(min(7, sha.len())).collect();
            let display_text = if let Some(title) = title {
                format!("Review commit {short_sha}: {title}")
            } else {
                format!("Review commit {short_sha}")
            };
            Ok(BuiltReviewRequest {
                review_request: ReviewRequest {
                    prompt,
                    user_facing_hint: hint,
                    append_to_original_thread,
                },
                display_text,
            })
        }
        ReviewTarget::Custom { instructions } => {
            let trimmed = instructions.trim().to_string();
            if trimmed.is_empty() {
                return Err(ReviewTargetError::EmptyInstructions);
            }
            let (prompt, hint) = review_custom_prompt(&trimmed);
            Ok(BuiltReviewRequest {
                review_request: ReviewRequest {
                    prompt,
                    user_facing_hint: hint,
                    append_to_original_thread,
                },
                display_text: trimmed,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_target_includes_title_and_short_sha() {
        let built = review_request_from_target(
            ReviewTarget::Commit {
                sha: "123456789".to_string(),
                title: Some("Refactor colors".to_string()),
            },
            true,
        )
        .expect("valid commit target");

        assert!(built.display_text.contains("1234567"));
        assert!(built.display_text.contains("Refactor colors"));
        assert_eq!(built.review_request.user_facing_hint, "commit 1234567");
        assert!(built.review_request.append_to_original_thread);
    }

    #[test]
    fn empty_inputs_reject() {
        assert_eq!(
            review_request_from_target(
                ReviewTarget::BaseBranch {
                    branch: "   ".to_string()
                },
                false
            )
            .unwrap_err(),
            ReviewTargetError::EmptyBranch
        );
        assert_eq!(
            review_request_from_target(
                ReviewTarget::Commit {
                    sha: "".to_string(),
                    title: None
                },
                false
            )
            .unwrap_err(),
            ReviewTargetError::EmptySha
        );
        assert_eq!(
            review_request_from_target(
                ReviewTarget::Custom {
                    instructions: "\n".to_string()
                },
                false
            )
            .unwrap_err(),
            ReviewTargetError::EmptyInstructions
        );
    }
}
