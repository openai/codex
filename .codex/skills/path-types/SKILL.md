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

## Migration requirements

Keep these requirements in mind while migrating code to conform with the above guidelines:

* existing app-server clients keep sending and receiving legacy native-path strings
* app-server can retain and manipulate foreign-platform path URIs
* exec-server APIs use file:// URIs
* local-only operation must not change model-visible text
* model tool arguments may contain raw relative or absolute paths for any OS
* path reasoning must work before the related environment has come online
* URIs cannot explicitly encode the executor’s path convention or operating system
* users must not configure the environment’s OS/path convention explicitly
* URIs should not yet be stored in rollouts, databases, or other persistent storage
* path conversion errors: fail-closed for security-relevant paths, fail-open for UI/diagnostics
* prefer small focused methods on `PathUri` or `LegacyAppPathString` over local helpers

It is OK if the conversion between paths and URIs is somewhat lossy as long as it will do the right
thing for real users.

Migrating to URIs should not add significant new failure modes. We will need to surface errors in
some places that were previously infallible but it should be kept to a minimum.
