# MCP client conformance

This directory tests the actual Codex executable against the official Model
Context Protocol client conformance suite. It exercises the shipping legacy,
intermediate `2025-11-25`, and modern `2026-07-28` protocols, localhost HTTP,
stdio, OAuth, and additional transport and security regression fixtures.

The official upstream suite is pinned to
`modelcontextprotocol/conformance@49103de6ed70804e940637bf3e9e29e4a3f54e64`.
Use Node.js 22 and Python 3.10 or later.

## Run the conformance gate

First install the frozen workspace dependencies and build Codex:

```bash
pnpm install --frozen-lockfile
cargo build --locked --manifest-path codex-rs/Cargo.toml -p codex-cli --bin codex
```

From a published Codex checkout, run:

```bash
python3 scripts/mcp_conformance/run_codex_compliance.py \
  codex-rs/target/debug/codex \
  --conformance-cli node_modules/@modelcontextprotocol/conformance/dist/index.js \
  --baseline-report scripts/mcp_conformance/regression-baseline-v1.json \
  --report /tmp/codex-mcp-conformance.json
```

The positional executable can also point to an already built Codex binary.
`--conformance-cli` selects the exact, lockfile-installed upstream JavaScript
runner instead of downloading a moving version during a test.

## What the baseline means

`regression-baseline-v1.json` is a compact, reviewed snapshot of the upstream
revision, required protocol versions, HTTP and stdio transports, enabled modern
feature, OAuth coverage, and individual passing and failing check identities.

The gate exits successfully only when:

- The upstream suite and modern feature match the committed baseline.
- The shipping legacy, intermediate, and modern protocols are actually tested.
- The required HTTP, stdio, and authentication scenarios are actually run.
- Every previously passing check still passes.
- No additional check fails.

Existing known failures remain visible in the complete JSON report. In
particular, `success` describes complete upstream conformance and
`regressionGate.success` describes the no-new-regressions merge gate; the gate
does not relabel an existing failure as a pass.

Create a compact baseline from a reviewed complete report without contacting
the upstream suite again:

```bash
python3 scripts/mcp_conformance/run_codex_compliance.py \
  /absolute/path/to/codex \
  --baseline-report /absolute/path/to/full-conformance-report.json \
  --extract-baseline /tmp/mcp-conformance-regression-baseline-v1.json
```

Alternatively, add `--write-baseline /tmp/mcp-conformance-regression-baseline-v1.json`
to a complete conformance run. Review every baseline change; do not regenerate
it to conceal a regression.

## Run the production reviewer regression gate

The separate reviewer gate tests the real Codex app-server across all three
shipping, legacy, and modern protocol modes. It covers stdio and localhost
HTTP, exact-integer tool and elicitation schemas, bounded multi-round requests,
malformed discovery response IDs, repeated pagination cursors, SSE framing and
keepalives, and catalog boundaries. In a published Codex checkout, run:

```bash
python3 scripts/mcp_conformance/review_regressions.py \
  /absolute/path/to/codex \
  --mode all \
  --baseline-report scripts/mcp_conformance/review-regression-baseline-v1.json \
  --report /tmp/codex-mcp-review-regressions.json
```

A complete, main-derived baseline records all 186 real check identities and all
21 required cases. Existing failures remain explicitly visible in the complete
report; `regressionGate.success: true` means there are no newly failing or
missing checks. Improvements are recorded under `fixedChecks`. The gate never
classifies an existing failure as a passing check.

Extract a compact deterministic reviewer baseline from a reviewed complete
production report without rerunning the client:

```bash
python3 scripts/mcp_conformance/review_regressions.py \
  /absolute/path/to/codex \
  --baseline-report /absolute/path/to/full-review-regressions.json \
  --extract-baseline /tmp/review-regression-baseline-v1.json
```

Review every baseline update. Do not regenerate a baseline to hide a regression.

## Run the fixture self-tests

```bash
env PYTEST_DISABLE_PLUGIN_AUTOLOAD=1 \
  python3 -m pytest -q scripts/mcp_conformance
```

## Run the required SDK integration

The existing required SDK workflow runs the complete Python fixture self-tests.
Its TypeScript job builds the actual Codex executable, sets `CODEX_EXEC_PATH`,
installs the pinned upstream conformance runner, and runs both the official
authenticated suite and the separate production reviewer regression matrix.
Neither gate can be skipped. To reproduce the focused integration locally:

```bash
CODEX_EXEC_PATH=/absolute/path/to/codex \
  pnpm --filter @openai/codex-sdk test -- \
  --runInBand tests/mcpConformance.test.ts
```
