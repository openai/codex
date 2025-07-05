# openai/codex-action

`openai/codex-action` is a GitHub Action that facilitates the use of [Codex](https://github.com/openai/codex) on GitHub issues and pull requests. Using the action, associate **labels** to run Codex with the appropriate prompt for the given context. Codex will respond by posting comments or creating PRs, whichever you specify!

Here is a sample workflow that uses `openai/codex-action`:

```yaml
name: Codex

on:
  issues:
    types: [opened, labeled]
  pull_request:
    branches: [main]
    types: [labeled]

jobs:
  codex:
    if: ... # optional, but can be effective in conserving CI resources
    runs-on: ubuntu-latest
    # TODO(mbolin): Need to verify if/when `write` is necessary.
    permissions:
      contents: write
      issues: write
      pull-requests: write
    steps:
      # By default, Codex runs network disabled using --full-auto, so perform
      # any setup that requires network (such as installing dependencies)
      # before openai/codex-action.
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Run Codex
        uses: openai/codex-action@latest
        with:
          openai_api_key: ${{ secrets.CODEX_OPENAI_API_KEY }}
          github_token: ${{ secrets.GITHUB_TOKEN }}
```

See sample usage in [`codex.yml`](../../workflows/codex.yml).

## Triggering the Action

Using the sample workflow above, we have:

```yaml
on:
  issues:
    types: [opened, labeled]
  pull_request:
    branches: [main]
    types: [labeled]
```

which means our workflow will be triggered when any of the following events occur:

- a label is added to an issue
- a label is added to a pull request against the `main` branch

### Label-Based Triggers

To define a GitHub label that should trigger Codex, create a file named `.github/codex/labels/LABEL-NAME.md` in your repository where `LABEL-NAME` is the name of the label. The content of the file is the prompt template to use when the label is added (see more on [Prompt Template Variables](#prompt-template-variables) below).

For example, if the file `.github/codex/labels/codex-review.md` exists, then:

- Adding the `codex-review` label will trigger the workflow containing the `openai/codex-action` GitHub Action.
- When `openai/codex-action` starts, it will replace the `codex-review` label with `codex-review-in-progress`.
- When `openai/codex-action` is finished, it will replace the `codex-review-in-progress` label with `codex-review-completed`.

If Codex sees that either `codex-review-in-progress` or `codex-review-completed` is already present, it will not perform the action.

As determined by the [default config](./src/default-label-config.ts), Codex will act on the following labels by default:

- Adding the `codex-review` label to a pull request will have Codex review the PR and add it to the PR as a comment.
- Adding the `codex-triage` label to an issue will have Codex investigate the issue and report its findings as a comment.
- Adding the `codex-issue-fix` label to an issue will have Codex attempt to fix the issue and create a PR wit the fix, if any.

## Action Inputs

The `openai/codex-action` GitHub Action takes the following inputs

### `openai_api_key` (required)

Set your `OPENAI_API_KEY` as a [repository secret](https://docs.github.com/en/actions/security-for-github-actions/security-guides/using-secrets-in-github-actions). See **Secrets and varaibles** then **Actions** in the settings for your GitHub repo.

Note that the secret name does not have to be `OPENAI_API_KEY`. For example, you might want to name it `CODEX_OPENAI_API_KEY` and then configure it on `openai/codex-action` as follows:

```yaml
openai_api_key: ${{ secrets.CODEX_OPENAI_API_KEY }}
```

### `github_token` (required)

This is required so that Codex can post a comment or create a PR. Set this value on the action as follows:

```yaml
github_token: ${{ secrets.GITHUB_TOKEN }}
```

### `codex_args`

A whitespace-delimited list of arguments to pass to Codex. Defaults to `--full-auto`, but if you want to override the default model to use `o3`:

```yaml
codex_args: "--full-auto --model o3"
```

For more complex configurations, use the `codex_home` input.

### `codex_home`

If set, the value to use for the `$CODEX_HOME` environment variable when running Codex. As explained [in the docs](https://github.com/openai/codex/tree/main/codex-rs#readme), this folder can contain the `config.toml` to configure Codex, custom instructions, and log files.

This should be a relative path within your repo.

## Prompt Template Variables

As shown above, `"prompt"` and `"promptPath"` are used to define prompt templates that will be populated and passed to Codex in response to certain events. All template variables are of the form `{CODEX_ACTION_...}` and the supported values are defined below.

### `CODEX_ACTION_ISSUE_TITLE`

If the action was triggered on a GitHub issue, this is the issue title.

Specifically it is read as the `.issue.title` from the `$GITHUB_EVENT_PATH`.

### `CODEX_ACTION_ISSUE_BODY`

If the action was triggered on a GitHub issue, this is the issue body.

Specifically it is read as the `.issue.body` from the `$GITHUB_EVENT_PATH`.

### `CODEX_ACTION_GITHUB_EVENT_PATH`

The value of the `$GITHUB_EVENT_PATH` environment variable, which is the path to the file that contains the JSON payload for the event that triggered the workflow. Codex can use `jq` to read only the fields of interest from this file.

### `CODEX_ACTION_PR_DIFF`

If the action was triggered on a pull request, this is the diff between the base and head commits of the PR. It is the output from `git diff`.

Note that the content of the diff could be quite large, so is generally safer to point Codex at `CODEX_ACTION_GITHUB_EVENT_PATH` and let it decide how it wants to explore the change.
