Review this PR and respond with a very concise final message, formatted in Markdown.

There should be a summary of the changes (1-2 sentences) and a few bullet points if necessary.

Then provide the **review** (1-2 sentences plus bullet points, friendly tone).

Things to look out for when doing the review:

## General Principles

- **Make sure the pull request body explains the motivation behind the change.** If the author has failed to do this, call it out, and if you think you can deduce the motivation behind the change, propose copy.
- Ideally, the PR body also contains a small summary of the change. For small changes, the PR title may be sufficient.
- Each PR should ideally do one conceptual thing. For example, if a PR does a refactoring as well as introducing a new feature, push back and suggest the refactoring be done in a separate PR. This makes things easier for the reviewer, as refactoring changes can often be far-reaching, yet quick to review.
- When introducing new code, be on the lookout for code that duplicates existing code. When found, propose a way to refactor the existing code such that it should be reused.

## Code Organization

- Each crate in the Cargo workspace in `codex-rs` has a specific purpose: make a note if you believe new code is not introduced in the correct crate.
- When possible, try to keep the `core` crate as small as possible. Non-core but shared logic is often a good candidate for `codex-rs/common`.
- Be wary of large files and offer suggestions for how to break things into more reasonably-sized files.
- Rust files should generally be organized such that the public parts of the API appear near the top of the file and helper functions go below. This is analogous to the "inverted pyramid" structure that is favored in journalism.

## Assertions in Tests

Assert the equality of the entire objects instead of doing "piecemeal comparisons," performing `assert_eq!()` on individual fields.

Note that unit tests also function as "executable documentation." As shown in the following example, "piecemeal comparisons" are often more verbose, provide less coverage, and are not as useful as executable documentation.

For example, suppose you have the following enum:

```rust
#[derive(Debug, PartialEq)]
enum Message {
    Request {
        id: String,
        method: String,
        params: Option<serde_json::Value>,
    },
    Notification {
        method: String,
        params: Option<serde_json::Value>,
    },
}
```

This is an example of a _piecemeal_ comparison:

```rust
// BAD: Piecemeal Comparison

#[test]
fn test_get_latest_messages() {
    let messages = get_latest_messages();
    assert_eq!(messages.len(), 2);

    let m0 = &messages[0];
    match m0 {
        Message::Request { id, method, params } => {
            assert_eq!(id, "123");
            assert_eq!(method, "subscribe");
            assert_eq!(
                *params,
                Some(json!({
                    "conversation_id": "x42z86"
                }))
            )
        }
        Message::Notification { .. } => {
            panic!("expected Request");
        }
    }

    let m1 = &messages[1];
    match m1 {
        Message::Request { .. } => {
            panic!("expected Notification");
        }
        Message::Notification { method, params } => {
            assert_eq!(method, "log");
            assert_eq!(
                *params,
                Some(json!({
                    "level": "info",
                    "message": "subscribed"
                }))
            )
        }
    }
}
```

This is a _deep_ comparison:

```rust
// GOOD: Verify the entire structure with a single assert_eq!().

use pretty_assertions::assert_eq;

#[test]
fn test_get_latest_messages() {
    let messages = get_latest_messages();

    assert_eq!(
        vec![
            Message::Request {
                id: "123".to_string(),
                method: "subscribe".to_string(),
                params: Some(json!({
                    "conversation_id": "x42z86"
                })),
            },
            Message::Notification {
                method: "log".to_string(),
                params: Some(json!({
                    "level": "info",
                    "message": "subscribed"
                })),
            },
        ],
        messages,
    );
}
```

## More Tactical Rust Things To Look Out For

- Do not use `unsafe` (unless you have a really, really good reason like using an operating system API directly and no safe wrapper exists). For example, there are cases where it is tempting to use `unsafe` in order to use `std::env::set_var()`, but this indeed `unsafe` and has led to race conditions on multiple occasions. (When this happens, find a mechanism other than environment variables to use for configuration.)
- Encourage the use of small enums or the newtype pattern in Rust if it helps readability without adding significant cognitive load or lines of code.
- If you see opportunities for the changes in a diff to use more idiomatic Rust, please make specific recommendations. For example, favor the use of expressions over `return`.
- When modifying a `Cargo.toml` file, make sure that dependency lists stay alphabetically sorted. Also consider whether a new dependency is added to the appropriate place (e.g., `[dependencies]` versus `[dev-dependencies]`)

## Pull Request Body

- If the nature of the change seems to have a visual component (which is often the case for changes to `codex-rs/tui`), recommend including a screenshot or video to demonstrate the change, if appropriate.
- References to existing GitHub issues and PRs are encouraged, where appropriate, though you likely do not have network access, so may not be able to help here.

# PR Information

{CODEX_ACTION_GITHUB_EVENT_PATH} contains the JSON that triggered this GitHub workflow. It contains the `base` and `head` refs that define this PR. Both refs are available locally.
