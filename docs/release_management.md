# Release Management

Currently, we made Codex binaries available in three places:

- GitHub Releases https://github.com/openai/codex/releases/
- `@jojoyo/codex` on npm: https://www.npmjs.com/package/@jojoyo/codex
- `codex` on Homebrew: https://formulae.brew.sh/formula/codex

# Cutting a Release

Currently, choosing the version number for the next release is a manual process. In general, just go to https://github.com/openai/codex/releases/latest and see what the latest release is and increase the minor version by `1`, so if the current release is `0.20.0`, then the next release should be `0.21.0`.

Assuming you are trying to publish `0.21.0`, first you would run:

```shell
VERSION=0.21.0
./codex-rs/scripts/create_github_release.sh "$VERSION"
```

This will kick off a GitHub Action to build the release, so go to https://github.com/openai/codex/actions/workflows/rust-release.yml to find the corresponding workflow. (Note: we should automate finding the workflow URL with `gh`.)

When the workflow finishes, the GitHub Release is "done," but you still have to consider npm and Homebrew.

## Publishing to npm

After the GitHub Release is done, you can publish to npm. Note the GitHub Release includes the appropriate artifact for npm (which is the output of `npm pack`), which should be named `codex-npm-VERSION.tgz`. To publish to npm, run:

```
VERSION=0.21.0
./scripts/publish_to_npm.py "$VERSION"
```

Note that you must have permissions to publish to https://www.npmjs.com/package/@jojoyo/codex for this to succeed.

## Publishing to Homebrew

For Homebrew, we are properly set up with their automation system, so every few hours or so it will check our GitHub repo to see if there is a new release. When it finds one, it will put up a PR to create the equivalent Homebrew release, which entails building Codex CLI from source on various versions of macOS.

Inevitably, you just have to refresh this page periodically to see if the release has been picked up by their automation system:

https://github.com/Homebrew/homebrew-core/pulls?q=%3Apr+codex

Once everything builds, a Homebrew admin has to approve the PR. Again, the whole process takes several hours and we don't have total control over it, but it seems to work pretty well.

For reference, our Homebrew formula lives at:

https://github.com/Homebrew/homebrew-core/blob/main/Formula/c/codex.rb

## Release notes (auto-generated)

- Release notes are generated automatically from commits using `git-cliff` when the `rust-release` workflow runs on a `rust-v<version>` tag.
- Commit format: follow Conventional Commits (e.g., `feat(tui): ...`, `fix(core): ...`, `docs: ...`, `refactor: ...`, `ci: ...`).
- Groups in the notes are derived from the commit type; breaking changes can be marked with `!` or a `BREAKING CHANGE:` trailer.
- If you need a local preview:
- One‑command local preview：`scripts/gen_release_notes.sh rust-vX.Y.Z ./RELEASE_NOTES.md`
  - 逻辑：自动选取“上一个 rust-v* 标签（按创建时间的上一个）”，并用区间 `prev..current` 生成。
  - 若机器已安装 `git-cliff`，生成内容与 CI 一致；否则回退为简要的 `git log` 列表。
  - Or compare against the previous tag automatically by omitting `--tag` and running it on the release commit.
