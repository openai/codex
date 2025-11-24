# How to Create a Release

_Instruction for Maintainers only._

- Prepare the release by updating [CHANGELOG.md](CHANGELOG.md), see for example
[this PR](https://github.com/open-telemetry/opentelemetry-proto/pull/537).
Merge the PR. From this point on no new PRs can be merged until the release is complete.

- Go to Github [release page](https://github.com/open-telemetry/opentelemetry-proto/releases),
click `Draft a new release`.

- Click "Choose a tag" and specify the next version number. The Target branch should be "main".

- Click "Generate release notes" to get a draft release note. Remove editorial
changes from the notes and any other changes that you don't want in the release notes.
In addition, you can refer to [CHANGELOG.md](CHANGELOG.md) for a list of major changes since last release.

- Click "Publish Release".

Our tags follow the naming convention of `v1.<minor>.<patch>`. Increment `minor` by 1
and use `patch` value of 0 for new minor version releases. For patch releases keep `minor`
unchanged and increment the `patch`.
