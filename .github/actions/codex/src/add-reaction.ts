import * as github from "@actions/github";
import type { EnvContext } from "./env-context";

/**
 * Add an "eyes" reaction to the entity (issue, issue comment, or pull request
 * review comment) that triggered the current Codex invocation.
 *
 * The purpose is to provide immediate feedback to the user – similar to the
 * *-in-progress label flow – indicating that the bot has acknowledged the
 * request and is working on it.
 *
 * We attempt to add the reaction best suited for the current GitHub event:
 *
 *   • issues              → POST /repos/{owner}/{repo}/issues/{issue_number}/reactions
 *   • issue_comment       → POST /repos/{owner}/{repo}/issues/comments/{comment_id}/reactions
 *   • pull_request_review_comment → POST /repos/{owner}/{repo}/pulls/comments/{comment_id}/reactions
 *
 * If the specific target is unavailable (e.g. unexpected payload shape) we
 * silently skip instead of failing the whole action because the reaction is
 * merely cosmetic.
 */
export async function addEyesReaction(ctx: EnvContext): Promise<void> {
  const octokit = ctx.getOctokit();
  const { owner, repo } = github.context.repo;
  const eventName = github.context.eventName;

  try {
    switch (eventName) {
      case "issue_comment": {
        const commentId = (github.context.payload as any)?.comment?.id;
        if (commentId) {
          await octokit.rest.reactions.createForIssueComment({
            owner,
            repo,
            comment_id: commentId,
            content: "eyes",
          });
          return;
        }
        break;
      }
      case "pull_request_review_comment": {
        const commentId = (github.context.payload as any)?.comment?.id;
        if (commentId) {
          await octokit.rest.reactions.createForPullRequestReviewComment({
            owner,
            repo,
            comment_id: commentId,
            content: "eyes",
          });
          return;
        }
        break;
      }
      case "issues": {
        const issueNumber = github.context.issue.number;
        if (issueNumber) {
          await octokit.rest.reactions.createForIssue({
            owner,
            repo,
            issue_number: issueNumber,
            content: "eyes",
          });
          return;
        }
        break;
      }
      default: {
        // Fallback: try to react to the issue/PR if we have a number.
        const issueNumber = github.context.issue.number;
        if (issueNumber) {
          await octokit.rest.reactions.createForIssue({
            owner,
            repo,
            issue_number: issueNumber,
            content: "eyes",
          });
        }
      }
    }
  } catch (error) {
    // Do not fail the action if reaction creation fails – log and continue.
    console.warn(`Failed to add \"eyes\" reaction: ${error}`);
  }
}
