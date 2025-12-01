name: CLA Assistant
on:
  issue_comment:
    types: [created]
  pull_request_target:
    types: [opened, closed, synchronize]

permissions:
  actions: write
  contents: write
  pull-requests: write
  statuses: write

jobs:
  cla:
    # Only run the CLA assistant for the canonical openai repo so forks are not blocked
    # and contributors who signed previously do not receive duplicate CLA notifications.
    if: ${{ github.repository_owner == 'openai' }}
    runs-on: ubuntu-latest
    steps:
      - uses: contributor-assistant/github-action@v2.6.1
        # Run on close only if the PR was merged. This will lock the PR to preserve
        # the CLA agreement. We don't want to lock PRs that have been closed without
        # merging because the contributor may want to respond with additional comments.
        # This action has a "lock-pullrequest-aftermerge" option that can be set to false,
        # but that would unconditionally skip locking even in cases where the PR was merged.
        if: |
          (
            github.event_name == 'pull_request_target' &&
            (
              github.event.action == 'opened' ||
              github.event.action == 'synchronize' ||
              (github.event.action == 'closed' && github.event.pull_request.merged == true)
            )
          ) ||
          (
            github.event_name == 'issue_comment' &&
            (
              github.event.comment.body == 'recheck' ||
              github.event.comment.body == 'I have read the CLA Document and I hereby sign the CLA'
            )
          )
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          path-to-document: https://github.com/openai/codex/blob/main/docs/CLA.md
          path-to-signatures: signatures/cla.json
          branch: cla-signatures
          allowlist: codex,dependabot,dependabot[bot],github-actions[bot]
