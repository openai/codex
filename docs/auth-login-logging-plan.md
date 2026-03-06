# Auth Login Logging

## Problem

Customer-side auth failures are hard to diagnose because the most important browser-login step,
the final `POST https://auth.openai.com/oauth/token` after the localhost callback, historically
does not show up as a first-class application event.

In the failing HARs and Slack thread, browser auth succeeds, workspace selection succeeds, and the
browser reaches `http://localhost:1455/auth/callback`. Support can usually confirm that:

- the user reached the browser sign-in flow
- the browser returned to the localhost callback
- Codex showed a generic sign-in failure

What support cannot reliably determine from Codex-owned logs is why the final token exchange
failed. That leaves the most important diagnostic question unanswered:

- was this a backend non-2xx response
- a transport failure talking to `auth.openai.com`
- a proxy, TLS, DNS, or connectivity issue
- some other local client-side failure after browser auth completed

This documentation explains how the current instrumentation closes that gap without broadening the
normal logging surface in unsafe ways.

## Mental Model

The browser-login flow has three separate outputs, and they do not serve the same audience:

- the browser-facing error page
- the caller-visible returned `io::Error`
- the normal structured application log

Those outputs now intentionally diverge.

The browser-facing page and returned error still preserve the backend detail needed by developers,
sysadmins, and support engineers to understand what happened. The structured log stays narrower:
it emits explicitly reviewed fields, redacted URLs, and redacted transport errors so the normal
log file is useful without becoming a credential sink.

## Non-goals

This does not add auth logging to every runtime request.

- The instrumentation is scoped to the initial browser-login callback flow.
- The refresh-token path in `codex-core` remains a separate concern.
- This does not attempt to classify every transport failure into a specific root cause from string
  matching.

## Tradeoffs

This implementation prefers fidelity for caller-visible errors and restraint for structured logs.

- Non-2xx token endpoint responses log parsed safe fields such as status, `error`, and
  `error_description` when available.
- Non-JSON token endpoint bodies are preserved in the returned error so CLI/browser flows still
  surface the backend detail that operators need.
- The callback-layer structured log does not log `%err` for token endpoint failures, because that
  would persist arbitrary backend response text into the normal log file.
- Transport failures keep the underlying `reqwest` error text, but attached URLs are redacted
  before they are logged or returned.
- Caller-supplied issuer URLs are sanitized before they are logged, including custom issuers with
  embedded credentials or sensitive query params.

The result is not maximally detailed in one place. It is intentionally split so each surface gets
the level of detail it can safely carry.

## Architecture

The browser-login callback flow lives in
[`codex-rs/login/src/server.rs`](../codex-rs/login/src/server.rs).

The key behavior is:

- the callback handler logs whether the callback was received and whether state validation passed
- the token exchange logs start, success, and non-2xx responses as structured events
- transport failures log the redacted `reqwest` error plus `is_timeout`, `is_connect`, and
  `is_request`
- the browser-facing `Codex Sign-in Error` page remains intact
- the returned `io::Error` continues to carry useful backend detail for CLI/browser callers

App-server consumers use the same login-server path rather than a separate auth implementation.

- `account/login/start` calls into `run_login_server(...)`
- app-server waits for `server.block_until_done()`
- app-server emits `account/login/completed` with wrapped success/error state

That means the login-crate instrumentation benefits:

- direct CLI / TUI login
- Electron app login
- VS Code extension login

Direct `codex login` also writes a small file-backed log through the CLI crate.

- the file is `codex-login.log` under the configured `log_dir`
- this uses a deliberately small tracing setup local to the CLI login commands
- it does not try to reuse the TUI logging stack wholesale, because the TUI path also installs
  feedback, OpenTelemetry, and other interactive-session layers that are not needed for a
  one-shot login command
- the duplication is intentional: it keeps the direct CLI behavior easy to reason about while
  still giving support a durable artifact from the same `codex_login::server` events

## Observability

The main new signals are emitted from the `login` crate target, for example
`codex_login::server`, so they stay aligned with the code that produces them.

The useful events are:

- callback received
- callback state mismatch
- OAuth callback returned error
- OAuth token exchange started
- OAuth token exchange transport failure
- OAuth token exchange returned non-success status
- OAuth token exchange succeeded

The structured log intentionally uses a narrower payload than the returned error:

- issuer URLs are sanitized before logging
- sensitive URL query keys such as `code`, `state`, `token`, `access_token`, `refresh_token`,
  `id_token`, `client_secret`, and `code_verifier` are redacted
- embedded credentials and fragments are stripped from logged URLs
- parsed token-endpoint fields are logged individually when available
- arbitrary non-JSON token endpoint bodies are not logged into the normal application log

This split is the main privacy boundary in the implementation.

## Failure Modes

The current instrumentation is most useful for these cases:

- browser auth succeeds but the final token exchange fails
- custom issuer deployments need confirmation that the callback reached the login server
- operators need to distinguish backend non-2xx responses from transport failures
- transport failures need the underlying `reqwest` signal without leaking sensitive URL parts

It is intentionally weaker for one class of diagnosis:

- it does not try to infer specific transport causes such as proxy, TLS, or DNS from message
  string matching, because that kind of over-classification can mislead operators

## Security and Sensitivity Notes

This implementation treats the normal application log as a persistent surface that must be safe to
collect and share.

That means:

- user-supplied issuer URLs are sanitized before logging
- transport errors redact attached URLs instead of dropping them entirely
- known secret-bearing query params are redacted surgically rather than removing all URL context
- non-JSON token endpoint bodies are preserved only for the returned error path, not the
  structured-log path

This behavior reflects two review-driven constraints that are already fixed in the code:

- custom issuers no longer leak embedded credentials or sensitive query params in the
  `starting oauth token exchange` log line
- non-JSON token endpoint bodies are once again preserved for caller-visible errors, but they no
  longer get duplicated into normal structured logs through callback-layer `%err` logging

## Debug Path

For a failed sign-in, read the evidence in this order:

1. Browser/HAR evidence:
   confirm the browser reached `http://localhost:1455/auth/callback`.
2. Login-crate structured logs:
   check whether the callback was received, whether state validation passed, and whether the token
   exchange failed as transport or non-2xx.
3. Caller-visible error:
   use the CLI/browser/app error text to recover backend detail that is intentionally not copied
   into the normal log file.
4. App-server wrapper:
   if the flow runs through app-server, use `account/login/completed` and its wrapped
   `Login server error: ...` result as the client-facing envelope around the same login-crate
   behavior.

The most important invariant is simple: browser success does not imply login success. The native
client still has to exchange the auth code successfully after the callback arrives.

## Tests

The `codex-login` test suite covers the new redaction and parsing boundaries:

- parsed token endpoint JSON fields still surface correctly
- plain-text token endpoint bodies still remain available to the caller-visible error path
- sensitive query values are redacted selectively
- URL shape is preserved while credentials, fragments, and known secret-bearing params are removed
- issuer sanitization redacts custom issuer credentials and sensitive params before logging
