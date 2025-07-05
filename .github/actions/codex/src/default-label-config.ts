import type { Config } from "./config";

export function getDefaultConfig(): Config {
  return {
    labels: {
      "codex-investigate-issue": {
        getPromptTemplate: () =>
          `
Troubleshoot whether the reported issue is valid.

Provide a concise and respectful comment summarizing the findings.

### {CODEX_ACTION_ISSUE_TITLE}

{CODEX_ACTION_ISSUE_BODY}
`.trim(),
      },
      "codex-code-review": {
        getPromptTemplate: () =>
          `
Review this PR and respond with a very concise final message, formatted in Markdown.

There should be a summary of the changes (1-2 sentences) and a few bullet points if necessary.

Then provide the **review** (1-2 sentences plus bullet points, friendly tone).

{CODEX_ACTION_GITHUB_EVENT_PATH} contains the JSON that triggered this GitHub workflow. It contains the \`base\` and \`head\` refs that define this PR. Both refs are available locally.
`.trim(),
      },
      "codex-attempt-fix": {
        getPromptTemplate: () =>
          `
Attempt to solve the reported issue.

If a code change is required, create a new branch, commit the fix, and open a pull-request that resolves the problem.

### {CODEX_ACTION_ISSUE_TITLE}

{CODEX_ACTION_ISSUE_BODY}
`.trim(),
      },
    },
  };
}
