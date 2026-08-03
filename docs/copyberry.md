# Copyberry

Every commit on `main` in this repository is sanitized and then exported to `main` on https://github.com/openai/codex (assuming the commit is non-empty after the sanitization process).

The specifics of the transformation are defined in [`api/copyberry`](https://github.com/openai/openai/tree/master/api/copyberry) in the monorepo, but at the time of this writing, sanitization works as follows:

- The following files in `openai/codex-internal` are copied into the tree for the commit as-is:
  - `LICENSE`
  - `MODULE.bazel`
  - `MODULE.bazel.lock`
  - `bazel/**`
  - `codex-cli/**`
  - `codex-rs/**` except for anything that matches `codex-rs/internal*/**`
  - `defs.bzl`
  - `sdk/**`
  - `third_party/**`
- Within `codex-rs/Cargo.toml`, `codex-rs/**/Cargo.toml`, and `codex-rs/**/*.rs`, special comments (with examples linked below) are used to rewrite the source files (e.g., lines that end with `// copybara:strip-for-public` are removed). These comments are not transformed in any other files, and the final export fails if a `copybara:` marker remains anywhere in the public projection.
- The contents of the `public/` folder are copied verbatim into the root of the commit tree, overwriting files from the list above when paths collide. Anything matching `public/codex-rs/internal*/**` is excluded.
- The `MODULE.bazel.lock` and `codex-rs/Cargo.lock` files are minimally reconciled from existing baselines so third-party versions are preserved while internal-only first-party crates are excised.
- This tree is used to create an isolated two-commit branch containing a sanitized baseline and candidate, with no internal or public ancestry. Codex receives that branch in a blob-filtered clone where `origin/main` provides the `openai/codex` history, and is asked to create an appropriate message for the candidate commit. This ensures that internal, proprietary information is not in context when drafting the commit message.
- The final version of the commit is published as a pull request on https://github.com/openai/codex.
- Copyberry merges the PR according to GitHub's [indirect merge](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/about-pull-request-merges#indirect-merges) rules so the PR is labeled "merged" rather than "closed." Note the pull request number will be different than that of the original PR in `openai/codex-internal` that was used to create the public PR.

Important practical implications:

- `.github/workflows` defines GitHub workflows for `openai/codex-internal` while `public/.github/workflows` defines GitHub workflows for `openai/codex`. Note this parallelism also exists for `AGENTS.md` and the `.codex/` folder.
- External users will NOT see the original PR body or PR conversation for the commits merged to `main`, so you should feel free to include internal-only information there.
- Once a PR is merged, you can still modify the PR body through the GitHub UI (though the message of the associated commit is frozen), so if the sanitation pass left the commit message in a state that is too "watered down," feel free to manually update the PR body on `openai/codex`, particularly if it is a PR that is cited often externally.
- Public releases are still made from `openai/codex` so there are no questions externally around build provenance.

## How it Works

[**Copybara**](https://github.com/google/copybara) is an open source tool from Google that is used to transform and move code between repositories.

[**Copyberry**](https://github.com/openai/openai/blob/master/api/copyberry/README.md) is our internal, cloud-hosted version of Copybara.

**NOTE:** At the time of this writing, there is also work on a Copyberry2. Some of the clunky things we do in Copyberry today should be able to be done cleanly in Copyberry2.

The pipeline to export a commit is slightly complicated due to various constraints:

- The [`codex-internal-no-internal-code`](https://github.com/openai/openai/blob/master/api/copyberry/copyberry/configs/codex-internal-no-internal-code/push/copy.bara.sky) Copyberry job copies commits from `main` on `codex-internal` to the [`copybara-no-internal-code`](https://github.com/openai/codex-internal/tree/copybara-no-internal-code) branch on `codex-internal`. This transformation rewrites `Cargo.toml` and `.rs` files in `codex-rs` according to the special `copybara` comments defined in that Copybara workflow.
- The [`codex-internal-to-public-staging.yml`](../.github/workflows/codex-internal-to-public-staging.yml) GitHub workflow processes each commit on the `copybara-no-internal-code` branch and:
  - Runs `.github/codex-internal-to-public/pipeline.py prepare` to remove internal staging files and
    reconcile the public Cargo and Bazel lockfiles.
  - Runs Codex to generate the sanitized commit message in a temporary `public-commit-message.md` file.
  - Runs `.github/codex-internal-to-public/pipeline.py validate-message` to perform some static checks on the proposed message.
  - Runs `.github/codex-internal-to-public/pipeline.py publish` to copy the final commit to the [`copybara-no-internal-references`](https://github.com/openai/codex-internal/tree/copybara-no-internal-references) branch.
- The [`codex-internal-to-codex-oss`](https://github.com/openai/openai/blob/master/api/copyberry/copyberry/configs/codex-internal-to-codex-oss/push/copy.bara.sky) Copyberry job takes each commit from `copybara-no-internal-references` and uses it to create a pull request on `openai/codex`. Only after creating the pull request does Copyberry know the number it was assigned, so it then force-pushes the pull request to rewrite the commit message so the title includes the pull request number (this way, it is hyperlinked in the GitHub UI when looking at commit history).
- Finally, the PR is [_indirectly merged_](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/about-pull-request-merges#indirect-merges) to `openai/codex`. Note this does not wait for CI jobs, so it is possible that the new commit breaks CI. (The [`copybara-internal-to-public-check.yml`](.github/workflows/copybara-internal-to-public-check.yml) workflow is designed to prevent breakages to `openai/codex` CI pre-merge, but it is not a bulletproof solution.)

Note there are multiple stages because:

- Copyberry transforms cannot run Codex in order to rewrite the commit message.
- A GitHub workflow in `openai/codex-internal` cannot merge a commit/PR on `openai/codex`.
- The staging GitHub workflow cannot push changes to the root `.github/` folder because doing so would require manual approval under the organization's push policy. It therefore strips those changes; `public/.github/` changes are introduced later by Copyberry.

Assuming Copyberry2 gains the ability to run Codex as part of a transformation, we should be able to go directly from `main` on `openai/codex-internal` to `main` on `openai/codex`: we would no longer need the intermediate `copybara-no-internal-code` and `copybara-no-internal-references` branches.

## Copybara Comments

TODO(mbolin): to finish writing this section. For now, look at sample tests in <https://github.com/openai/openai/blob/master/api/copyberry/copyberry/configs/test_config_data.py>.

## When things Break

If the sync logic gets out of whack, point Codex at this file as well as the code for Copyberry and ask it to diagnose the situation and offer fixes. Sometimes it will have to create a one-off "sync commit" to get things back in order.
