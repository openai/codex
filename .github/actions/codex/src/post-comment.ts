import { fail } from "./fail";
import * as github from "@actions/github";
import { EnvContext } from "./env-context";

/**
 * Post a comment to the issue / pull request currently in scope.
 *
 * Provide the environment context so that token lookup (inside getOctokit) does
 * not rely on global state.
 */
export async function postComment(
  commentBody: string,
  ctx: EnvContext,
): Promise<void> {
  // Append a footer with a link back to the workflow run, if available.
  const footer = buildWorkflowRunFooter(ctx);
  const bodyWithFooter = footer ? `${commentBody}${footer}` : commentBody;

  const octokit = ctx.getOctokit();
  console.info("Got Octokit instance for posting comment");
  const { owner, repo } = github.context.repo;
  const issueNumber = github.context.issue.number;

  if (!issueNumber) {
    console.warn(
      "No issue or pull_request number found in GitHub context; skipping comment creation.",
    );
    return;
  }

  try {
    console.info("Calling octokit.rest.issues.createComment()");
    await octokit.rest.issues.createComment({
      owner,
      repo,
      issue_number: issueNumber,
      body: bodyWithFooter,
    });
  } catch (error) {
    fail(`Failed to create comment via GitHub API: ${error}`);
  }
}

/**
 * Helper to build a Markdown fragment linking back to the workflow run that
 * generated the current comment. Returns `undefined` if required environment
 * variables are missing – e.g. when running outside of GitHub Actions – so we
 * can gracefully skip the footer in those cases.
 */
function buildWorkflowRunFooter(ctx: EnvContext): string | undefined {
  const serverUrl =
    ctx.tryGetNonEmpty("GITHUB_SERVER_URL") ?? "https://github.com";
  const repository = ctx.tryGetNonEmpty("GITHUB_REPOSITORY");
  const runId = ctx.tryGetNonEmpty("GITHUB_RUN_ID");

  if (!repository || !runId) {
    return undefined;
  }

  const url = `${serverUrl}/${repository}/actions/runs/${runId}`;
  return `\n\n---\n*[_View workflow run_](${url})*`;
}
