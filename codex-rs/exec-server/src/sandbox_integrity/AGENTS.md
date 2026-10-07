# Sandbox integrity: scope and review contract

These checks currently emit telemetry only. Findings and preparation errors must
not change sandbox construction, permissions, approvals, or command execution.
Supported backends are Seatbelt, bubblewrap, and MXC; legacy Windows is out of scope.

## Responsibilities

- Backends supply complete dependency inventories and interpret the request's
  original filesystem policy: resolve command temp directories and MXC volumes,
  and reuse the backend's deny-glob expansion where enforcement expands globs.
- The runner prepares dependency facts and invokes each checker per dependency.
  Checkers remain independent of inventories and telemetry.
  Blocking work runs on the blocking pool with a five-second caller timeout.
  Timeouts use the existing check metric (`dependency=preparation`, `outcome=error`,
  `error_kind=timeout`). Late results are discarded; cancelling the blocking task
  or its scanner subprocess is deferred.
- Telemetry derives bounded tags from the policy, dependency facts, and checker
  results. Never emit paths, environment values, or error text.
  A missing metrics client disables emission, never the checks themselves.
  Use the process-global metrics client; separately injected clients in programs
  embedding Codex are outside this phase's scope.

## Deliberate approximation

Honor the basic read/write/deny rules. Do not reconstruct native enforcement or
audit OS ACLs. Additional backend protections can produce false positives;
additional write grants (such as Seatbelt scratch access) can produce false
negatives. Backend glob/symlink nuances and writable hard-link aliases are not
fully modeled. The separate glob scan can differ from enforcement's snapshot.
Metrics describe observed checker outcomes, not accuracy or missed violations.

MXC scanner discovery currently uses `which`, while enforcement uses
`Command`. Their search-order and environment differences can select different
executables or cause only one scan to use the internal walker. This is an accepted
telemetry limitation for this phase; changing production scanner selection is deferred.

These are accepted limitations of this phase, not requests to duplicate backend
implementations. Review against this scope. Regressions in explicit policy
handling, executor context, or reporting remain actionable; this is not a blanket
exemption for false positives or false negatives.

## Eligibility

Skip `SandboxOverride::EscalatedSandboxWithRestrictions` before preparation or
telemetry: these approved escalations retain only deny-read restrictions.
An ordinary configured `:root = write` policy with denials is still checked.
Do not infer approval from policy shape.
