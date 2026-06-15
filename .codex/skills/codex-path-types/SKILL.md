---
name: codex-path-types
description: Choose Rust path types that meet Codex's compatibility requirements. Use when defining new path-bearing Rust types or when explicitly asked to migrate existing types in app-server, exec-server, or dependencies shared by them.
---

# Codex Path Types

Apply this guidance when defining new types. Change existing code only when explicitly requested,
and keep edits minimal and proportional. Treat these rules as the target state of an ongoing
migration; if compliance is difficult, ask the user how to proceed.

- In app-server protocol types, use `ApiPathString` for backwards compatibility during the URI
  migration. At the protocol boundary, convert it to `PathUri` and use `PathUri` internally. For
  host-local logic, such as some config values, use `AbsolutePathBuf` or `PathBuf` instead.
- In exec-server protocol types, use `PathUri`. Internally, use `PathUri` or `AbsolutePathBuf` as
  appropriate.
- In dependencies shared by both servers, use `PathUri` or separate APIs that decouple their use
  cases.
