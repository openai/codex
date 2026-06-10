use super::*;
use pretty_assertions::assert_eq;

#[test]
fn review_prompt_template_renders_base_branch_backup_variant() {
    assert_eq!(
        render_review_prompt(&BASE_BRANCH_PROMPT_BACKUP_TEMPLATE, [("branch", "main")]),
        "Review the code changes against the base branch 'main'. Start by finding the merge diff between the current branch and main's upstream e.g. (`git merge-base HEAD \"$(git rev-parse --abbrev-ref \"main@{upstream}\")\"`), then run `git diff` against that SHA to see what changes we would merge into the main branch. Provide prioritized, actionable findings."
    );
}

#[test]
fn review_prompt_template_renders_base_branch_variant() {
    assert_eq!(
        render_review_prompt(
            &BASE_BRANCH_PROMPT_TEMPLATE,
            [("base_branch", "main"), ("merge_base_sha", "abc123")]
        ),
        "Review the code changes against the base branch 'main'. The merge base commit for this comparison is abc123. Run `git diff abc123` to inspect the changes relative to main. Provide prioritized, actionable findings."
    );
}

#[test]
fn review_prompt_template_renders_commit_variant() {
    assert_eq!(
        review_prompt(
            &ReviewTarget::Commit {
                sha: "deadbeef".to_string(),
                title: None,
            },
            &AbsolutePathBuf::current_dir().expect("cwd"),
        )
        .expect("commit prompt should render"),
        "Review the code changes introduced by commit deadbeef. Provide prioritized, actionable findings."
    );
}

#[test]
fn review_prompt_template_renders_commit_variant_with_title() {
    assert_eq!(
        review_prompt(
            &ReviewTarget::Commit {
                sha: "deadbeef".to_string(),
                title: Some("Fix bug".to_string()),
            },
            &AbsolutePathBuf::current_dir().expect("cwd"),
        )
        .expect("commit prompt should render"),
        "Review the code changes introduced by commit deadbeef (\"Fix bug\"). Provide prioritized, actionable findings."
    );
}
