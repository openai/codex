import { fail } from "./fail";
import { EnvContext } from "./env-context";
import { renderPromptTemplate } from "./prompt-template";

import { postComment } from "./post-comment";
import { runCodex } from "./run-codex";

import * as github from "@actions/github";
import { Config, LabelConfig } from "./config";
import { maybePublishPRForIssue } from "./git-helpers";

export async function onLabeled(
  config: Config,
  ctx: EnvContext,
): Promise<void> {
  const GITHUB_EVENT_LABEL_NAME = ctx.get("GITHUB_EVENT_LABEL_NAME");
  const labelConfig = config.labels[GITHUB_EVENT_LABEL_NAME] as
    | LabelConfig
    | undefined;
  if (!labelConfig) {
    fail(
      `Label \`${GITHUB_EVENT_LABEL_NAME}\` not found in config: ${JSON.stringify(config)}`,
    );
  }

  await processLabelConfig(ctx, GITHUB_EVENT_LABEL_NAME, labelConfig);
}

/**
 * Wrapper that handles `-in-progress` and `-completed` semantics around the core lint/fix/review
 * processing. It will:
 *
 * - Skip execution if the `-in-progress` or `-completed` label is already present.
 * - Mark the PR/issue as `-in-progress`.
 * - After successful execution, mark the PR/issue as `-completed`.
 */
async function processLabelConfig(
  ctx: EnvContext,
  label: string,
  labelConfig: LabelConfig,
): Promise<void> {
  const octokit = ctx.getOctokit();
  const { owner, repo, issueNumber, labelNames } =
    await getCurrentLabels(octokit);

  const inProgressLabel = `${label}-in-progress`;
  const completedLabel = `${label}-completed`;
  for (const markerLabel of [inProgressLabel, completedLabel]) {
    if (labelNames.includes(markerLabel)) {
      console.log(
        `Label '${markerLabel}' already present on issue/PR #${issueNumber}. Skipping Codex action.`,
      );

      // Clean up: remove the triggering label to avoid confusion and re-runs.
      await addAndRemoveLabels(octokit, {
        owner,
        repo,
        issueNumber,
        remove: markerLabel,
      });

      return;
    }
  }

  // Mark the PR/issue as in progress.
  await addAndRemoveLabels(octokit, {
    owner,
    repo,
    issueNumber,
    add: inProgressLabel,
    remove: label,
  });

  // Run the core Codex processing.
  await processLabel(ctx, label, labelConfig);

  // Mark the PR/issue as completed.
  await addAndRemoveLabels(octokit, {
    owner,
    repo,
    issueNumber,
    add: completedLabel,
    remove: inProgressLabel,
  });
}

async function processLabel(
  ctx: EnvContext,
  label: string,
  labelConfig: LabelConfig,
): Promise<void> {
  const template = labelConfig.getPromptTemplate();

  // If this is a review label, prepend explicit PR-diff scoping guidance to
  // reduce out-of-scope feedback. Do this before rendering so placeholders in
  // the guidance (e.g., {CODEX_ACTION_GITHUB_EVENT_PATH}) are substituted.
  const isReview = label.toLowerCase().includes("review");
  const reviewScopeGuidance = `
PR Diff Scope
- Only review changes between the PR's merge-base and head; do not comment on commits or files outside this range.
- Derive the base/head SHAs from the event JSON at {CODEX_ACTION_GITHUB_EVENT_PATH}, then compute and use the PR diff for all analysis and comments.

Commands to determine scope
- Resolve SHAs:
  - BASE_SHA=$(jq -r '.pull_request.base.sha // .pull_request.base.ref' "{CODEX_ACTION_GITHUB_EVENT_PATH}")
  - HEAD_SHA=$(jq -r '.pull_request.head.sha // .pull_request.head.ref' "{CODEX_ACTION_GITHUB_EVENT_PATH}")
  - BASE_SHA=$(git rev-parse "$BASE_SHA")
  - HEAD_SHA=$(git rev-parse "$HEAD_SHA")
- Prefer triple-dot (merge-base) semantics for PR diffs:
  - Changed commits: git log --oneline "$BASE_SHA...$HEAD_SHA"
  - Changed files: git diff --name-status "$BASE_SHA...$HEAD_SHA"
  - Review hunks: git diff -U0 "$BASE_SHA...$HEAD_SHA"

Review rules
- Anchor every comment to a file and hunk present in git diff "$BASE_SHA...$HEAD_SHA".
- If you mention context outside the diff, label it as "Follow-up (outside this PR scope)" and keep it brief (<=2 bullets).
- Do not critique commits or files not reachable in the PR range (merge-base(base, head) → head).
`.trim();

  const effectiveTemplate = isReview
    ? `${reviewScopeGuidance}\n\n${template}`
    : template;

  const populatedTemplate = await renderPromptTemplate(effectiveTemplate, ctx);

  // Always run Codex and post the resulting message as a comment.
  let commentBody = await runCodex(populatedTemplate, ctx);

  // Current heuristic: only try to create a PR if "attempt" or "fix" is in the
  // label name. (Yes, we plan to evolve this.)
  if (label.indexOf("fix") !== -1 || label.indexOf("attempt") !== -1) {
    console.info(`label ${label} indicates we should attempt to create a PR`);
    const prUrl = await maybeFixIssue(ctx, commentBody);
    if (prUrl) {
      commentBody += `\n\n---\nOpened pull request: ${prUrl}`;
    }
  } else {
    console.info(
      `label ${label} does not indicate we should attempt to create a PR`,
    );
  }

  await postComment(commentBody, ctx);
}

async function maybeFixIssue(
  ctx: EnvContext,
  lastMessage: string,
): Promise<string | undefined> {
  // Attempt to create a PR out of any changes Codex produced.
  const issueNumber = github.context.issue.number!; // exists for issues triggering this path
  try {
    return await maybePublishPRForIssue(issueNumber, lastMessage, ctx);
  } catch (e) {
    console.warn(`Failed to publish PR: ${e}`);
  }
}

async function getCurrentLabels(
  octokit: ReturnType<typeof github.getOctokit>,
): Promise<{
  owner: string;
  repo: string;
  issueNumber: number;
  labelNames: Array<string>;
}> {
  const { owner, repo } = github.context.repo;
  const issueNumber = github.context.issue.number;

  if (!issueNumber) {
    fail("No issue or pull_request number found in GitHub context.");
  }

  const { data: issueData } = await octokit.rest.issues.get({
    owner,
    repo,
    issue_number: issueNumber,
  });

  const labelNames =
    issueData.labels?.map((label: any) =>
      typeof label === "string" ? label : label.name,
    ) ?? [];

  return { owner, repo, issueNumber, labelNames };
}

async function addAndRemoveLabels(
  octokit: ReturnType<typeof github.getOctokit>,
  opts: {
    owner: string;
    repo: string;
    issueNumber: number;
    add?: string;
    remove?: string;
  },
): Promise<void> {
  const { owner, repo, issueNumber, add, remove } = opts;

  if (add) {
    try {
      await octokit.rest.issues.addLabels({
        owner,
        repo,
        issue_number: issueNumber,
        labels: [add],
      });
    } catch (error) {
      console.warn(`Failed to add label '${add}': ${error}`);
    }
  }

  if (remove) {
    try {
      await octokit.rest.issues.removeLabel({
        owner,
        repo,
        issue_number: issueNumber,
        name: remove,
      });
    } catch (error) {
      console.warn(`Failed to remove label '${remove}': ${error}`);
    }
  }
}
