---
name: path-types
description: Choose Rust types for operating system paths across the Codex repository. Use when defining new path-bearing types or explicitly migrating existing ones.
---

# Path Types

Apply this guidance when defining new types. Change existing code only when explicitly requested,
and keep edits minimal and proportional. Treat these rules as the target state of an ongoing
migration; if compliance is difficult, ask the user how to proceed.

- In app-server protocol types, use `LegacyAppPathString` for backwards compatibility during the URI
  migration. At the protocol boundary, convert it to `PathUri` and use `PathUri` internally. For
  host-local logic, such as some config values, use `AbsolutePathBuf` or `PathBuf` instead.
- In exec-server protocol types, use `PathUri`. Internally, use `PathUri` or `AbsolutePathBuf` as
  appropriate.
- In dependencies shared by both servers, use `PathUri` or separate APIs that decouple their use
  cases.
- Tool call arguments that the model is expected to generate should be deserialized as regular
  `String`s with feature-specific path handling code.
