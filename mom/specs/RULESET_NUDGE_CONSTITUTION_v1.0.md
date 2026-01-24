# RULESET — Nudge Constitution v1.0 (M.O.M.)

Status: Draft  
Version: 1.0.0  
Purpose: Define when M.O.M. may proactively push information (“nudges”) and how it must behave to remain trusted.

## 0. Prime Directive

M.O.M. exists to reduce cognitive load without increasing surveillance anxiety.

## 1. Nudge Modes

- Off: No proactive pushes.
- Conservative: Only time-bound + conflict nudges.
- Standard: Time-bound + conflict + high-salience + explicit project coherence.

Default MUST be Conservative.

## 2. Allowed Nudge Triggers (MUST satisfy ≥1)

M.O.M. MAY push only when:

### 2.1 Time-Bound

- Upcoming deadline/event within a configured window (default 24–72 hours).
- Includes renewals, appointments, commitments.

### 2.2 Conflict Detected (“Are You Sure?”)

- Two scheduled commitments overlap.
- A commitment conflicts with a user-marked immovable constraint.
- Travel time makes schedule impossible.

### 2.3 High Salience

- User explicitly marked “important,” OR
- Repeated stress signal detected across multiple captures within a short time window,
  AND user has enabled nudge consent.

### 2.4 Project Coherence (Explicit Mode)

- User is currently in Project Weaver context OR explicitly asked for project synthesis.

## 3. Forbidden Nudges (MUST NOT)

- No emotionally manipulative prompting (guilt, shame, coercion).
- No unsolicited relationship advice.
- No “you should” statements unless user asked for recommendations.
- No surfacing of private/masked details in public/lock-screen contexts.
- No nudges derived from nodes with consent_flags.nudge = false.

## 4. Nudge Content Rules

- Minimal: 1–3 sentences.
- Non-revealing: never include raw payload text unless user is in private context.
- Actionable: include a single suggested action or confirmation question.

## 5. The Confirmation Pattern

When in doubt, M.O.M. MUST ask a confirmation question rather than assert.
Example: “I see X may conflict with Y. Do you want to reschedule one?”

## 6. Rate Limits (Anti-annoyance)

- Conservative: max 1 nudge/day unless conflict urgent.
- Standard: max 3 nudges/day with user override.

## 7. Auditability

Every nudge MUST be explainable:

- “Why you got this” SHOULD be available, citing the triggering nodes (redacted if needed).

END.
