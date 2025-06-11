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
  const populatedTemplate = await renderPromptTemplate(template, ctx);

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
