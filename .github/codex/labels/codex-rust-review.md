Review this PR and respond with a very concise final message, formatted in Markdown.

There should be a summary of the changes (1-2 sentences) and a few bullet points if necessary.

Then provide the **review** (1-2 sentences plus bullet points, friendly tone).

Things to look out for when doing the review:

- **Make sure the pull request body explains the motivation behind the change.** If the author has failed to do this, call it out, and if you think you can deduce the motivation behind the change, propose copy.
- Ideally, the PR body also contains a small summary of the change. For small changes, the PR title may be sufficient.
- Each PR should ideally do one conceptual thing. For example, if a PR does a refactoring as well as introducing a new feature, push back and suggest the refactoring be done in a separate PR. This makes things easier for the reviewer, as refactoring changes can often be far-reaching, yet quick to review.
- If the nature of the change seems to have a visual component (which is often the case for changes to `codex-rs/tui`), recommend including a screenshot or video to demonstrate the change, if appropriate.
- Rust files should generally be organized such that the public parts of the API appear near the top of the file and helper functions go below. This is analagous to the "inverted pyramid" structure that is favored in journalism.
- Encourage the use of small enums or the newtype pattern in Rust if it helps readability without adding significant cognitive load or lines of code.
- Be wary of large files and offer suggestions for how to break things into more reasonably-sized files.
- When modifying a `Cargo.toml` file, make sure that dependency lists stay alphabetically sorted. Also consider whether a new dependency is added to the appropriate place (e.g., `[dependencies]` versus `[dev-dependencies]`)
- If you see opportunities for the changes in a diff to use more idiomatic Rust, please make specific recommendations. For example, favor the use of expressions over `return`.
- When introducing new code, be on the lookout for code that duplicates existing code. When found, propose a way to refactor the existing code such that it should be reused.
- Each create in the Cargo workspace in `codex-rs` has a specific purpose: make a note if you believe new code is not introduced in the correct crate.
- When possible, try to keep the `core` crate as small as possible. Non-core but shared logic is often a good candidate for `codex-rs/common`.
- References to existing GitHub issues and PRs are encouraged, where appropriate, though you likely do not have network access, so may not be able to help here.

{CODEX_ACTION_GITHUB_EVENT_PATH} contains the JSON that triggered this GitHub workflow. It contains the `base` and `head` refs that define this PR. Both refs are available locally.
