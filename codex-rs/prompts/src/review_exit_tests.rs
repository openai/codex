use super::*;
use pretty_assertions::assert_eq;

#[test]
fn render_review_exit_success_replaces_results_placeholder() {
    assert_eq!(
        render_review_exit_success("Finding A\nFinding B"),
        "<user_action>\n  <context>User initiated a review task. Here's the full review output from reviewer model. User may select one or more comments to resolve.</context>\n  <action>review</action>\n  <results>\n  Finding A\nFinding B\n  </results>\n  </user_action>\n"
    );
}

#[test]
fn normalize_review_template_line_endings_rewrites_crlf() {
    assert_eq!(
        normalize_review_template_line_endings("<user_action>\r\n  <results>\r\n  None.\r\n"),
        "<user_action>\n  <results>\n  None.\n"
    );
}
