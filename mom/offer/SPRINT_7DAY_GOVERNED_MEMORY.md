# 7-Day Sprint — Governed Memory + Safe Nudges (StagePort Install)

**Offer:** I install governed memory + nudge safety into your AI product in 7 days.  
**Outcome:** Your “memory” becomes traceable, consent-aware, and non-creepy — with a ruleset that prevents TacoBell failures (tiny mistakes → global damage).

## Who this is for

- AI apps that store memories, notes, chats, or user context
- teams shipping proactive features (nudges, reminders, suggestions)
- founders who need trust + auditability for investors and users

## What you get (deliverables)

### 1) Memory Governance Layer (StagePort)

- policy gate for capture/read/link/nudge/export
- consent flags per memory object
- privacy-level handling (private / shared / masked)
- fail-closed behavior when keys or consent aren’t present

### 2) Continuity + Provenance (Coda)

- append-only version records for memory updates (no silent overwrites)
- edit/redact/merge/split workflows with traceability
- “why did it say that?” explainability hooks

### 3) Nudge Constitution (Safe Proactive Behavior)

- strict trigger rules (time-bound / conflict / high-salience / project mode)
- forbidden behaviors codified (no manipulation, no lock-screen leakage, etc.)
- rate limits + escalation rules

### 4) Evaluation Harness (so it doesn’t drift)

- creep tests (does it surface private stuff accidentally?)
- prompt-injection tests for inbound content (emails/webpages)
- regression checks for linking/traversal budgets
- basic performance guardrails (regex / traversal blowups)

### 5) Implementation artifacts (hand-off ready)

- schemas (MemNode/Corridor/Coda)
- API surfaces or service interfaces (language-agnostic)
- integration plan + checklist
- short runbook

## What I need from you (Day 0)

- access to repo or a sandbox branch
- current memory storage approach (db tables / vector store / files)
- what nudges you want to ship (exact examples)
- what you refuse to ship (privacy red lines)
- optional: a staging environment

## 7-Day Timeline

### Day 1 — Audit + Threat Model

- map current memory flows + integrations
- enumerate risks + non-negotiables
- confirm push policy mode (conservative by default)

### Day 2 — Data Model Drop-In

- MemNode + Corridor schema (minimum viable)
- Coda version records (append-only)
- define invariants and failure modes

### Day 3 — StagePort Gate

- implement policy checks for read/write/link/export
- consent flags enforced
- masked output behavior for risky contexts

### Day 4 — Nudge Constitution

- implement trigger logic + rate limits
- notification redaction safeguards
- Are you sure? conflict checks

### Day 5 — Evals + TacoBell Tests

- prompt injection suite
- privacy leakage tests
- traversal budgets + link explosion prevention
- reproducible dependency pinning review

### Day 6 — Integrations Hardening

- email/calendar ingestion sanitization
- logging/audit traces (user-safe)
- runbook draft

### Day 7 — Ship + Handoff

- PRs merged or staging deployed
- walkthrough + docs
- next-step roadmap (v1 → v1.5)

## Acceptance criteria (definition of done)

- memory updates are versioned (no overwrite)
- nudges occur only when allowed by rules
- outputs cite sources (node references)
- masked/private data cannot leak via notifications
- basic injection attempts are neutralized
- rate limits prevent spammy behavior

## Pricing (choose your tier)

- **Solo / Early-stage**: $3,500
- **Team / Growth**: $7,500
- **Enterprise / High-stakes**: $15,000+ (includes on-prem considerations)

## Optional add-ons

- family plan permissions model (shared vaults)
- redaction masks & safe summaries
- cross-device sync design with user-held keys
- full on-device pipeline recommendations

END.
