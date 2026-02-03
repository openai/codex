# Role Nucleus Brief: A Reversible, Measurable Office OS

This document translates the idea into a precise, buildable system: **roles are portable capability shells**, **people are optional carriers**, and the **workspace is a central nucleus that measures impact while keeping revocation clean**. It is written as an architectural brief for a minimum-viable prototype that feels playful in mechanics yet reads enterprise-grade. The guiding constraint is **no damage to the core modules**: every enhancement must preserve base DNA while enabling crispr-like improvements at the edges.

---

## 1) Core primitives (the non-negotiables)

**Role (Hat / Costume / Persona Shell)**

- A capability container with permissions, runbooks, and quality metrics.
- Owned by the workspace, not by the person.
- Has a baseline configuration plus attachable “patch” layers.

**Person (Human / Device / Private Diary)**

- A contributor who can wear a role.
- Has a private “closet/briefcase” for drafts, PII, and personal notes.
- Can revoke the hat at any time without destroying the role.

**Workspace (Nucleus / Grid / Office OS)**

- The shared environment that executes actions, keeps receipts, and measures outcomes.
- Stores only what is explicitly contributed or licensed.
- Provides risk rails the way a physical office provides fire codes.

**Contribution (Patch / Improvement / Upgrade)**

- A reversible layer that can be attached to a role.
- Has a provenance trail: who, when, what, impact.
- Can be set to “active only while worn,” “licensed to remain,” or “promoted to base.”

---

## 2) Memory as protocol (score → cues → execution)

**Memory is choreography**: it must be **scored**, **cued**, and **performed**.  
Every role action is a score (notation), every prompt is a cue (targeted retrieval), and every run is execution (runtime performance). This creates a stable two-pipeline translation:

**Studio pipeline**  
Term → Cue → Movement (sequence)

**System pipeline**  
Instruction → Code + Retrieval → Performance (runtime)

This ensures the **core role DNA** is preserved while the **RNA layer** (cues, runbooks, patches) can be iterated without erasing the base structure.

## 3) The invitation flow (one-click, mobile-native)

**Goal:** Anyone can join and contribute without friction while preserving attribution.

**Flow (minimum viable):**

1. Email/SMS invite → **magic link**
2. Open on phone → **choose a hat**
3. “Add to Home Screen” (PWA)
4. First action → **receipt appears**

**Why it works:**

- No account creation required for first action.
- Every action is tagged with: `person_id`, `hat_id`, `action_id`, `delta`.
- Impact is measured by action receipts, not by vibes or politics.

---

## 4) The revocation mechanism (“NO CAP”)

**Principle:** Removing the hat restores the person immediately, while the role persists.

**Three revocation policies:**

1. **Strict revert** – remove all personal patches, return to base role.
2. **Licensed imprint** – patches remain but inactive unless licensed.
3. **Promoted to base** – org adopts the patch into baseline, with credit preserved.

This is the **understudy mechanic**: when the hat is removed, an understudy role appears by default—operational continuity without emotional drama.

---

## 5) The grid office (digital “real estate” that holds risk)

Physical offices externalize risk via building codes. Remote work erased this.

**Digital equivalents that must exist in the nucleus:**

- **Access rails:** who can view/edit/export
- **Continuity rails:** who covers if a role is removed
- **Audit rails:** every action produces a receipt
- **Incident rails:** breach, export, or failure protocols
- **Compliance rails:** clear policy confirmations

This is the **Risk Closet**: boring, visible, always-on.  
It reframes liability as a shared, measurable system.

---

## 6) Fit scoring (the “costume try-on” math)

**You want to measure fit without dehumanizing.**  
Use four dimensions that executives respect:

1. **Uplift** — reduced cost/time/errors
2. **Reliability** — improvements that persist
3. **Chemistry** — smoother handoffs, fewer conflicts
4. **Risk Discipline** — adherence to safety rails

**Fit Index (per person, per role):**

```
Fit = w1*Uplift + w2*Reliability + w3*Chemistry + w4*RiskDiscipline
```

This is the “dress fits or doesn’t fit” outcome, without personal judgment.

---

## 7) The “Glinda effect” (role upgrades as measurable patches)

When a person elevates a role:

- The workspace tracks the delta in throughput, quality, and risk.
- If the person leaves, the role reverts unless explicitly licensed.
- The delta is visible, like a **playbill insert**: “Understudy on tonight.”

This produces the “Chenoweth week” effect **inside the system**:

- “When X wore Finance Hat, errors dropped 38%.”
- “After revocation, errors returned to baseline.”

No Broadway references are required; the math is internal and defensible.

---

## 8) Minimum viable roles (single-device business ops)

Start with six desks. Each has 3 actions max:

1. **Executive Assistant** – calendar sweep, follow-ups, daily brief
2. **Operations Manager** – bottleneck scan, handoff checks, queue health
3. **Finance Lead** – variance scan, receipts reconcile, export gate
4. **Sales Desk** – pipeline import, next-step prompts, renewal alerts
5. **Client Success** – account pulse, churn risk, recap capture
6. **Risk Closet** – access audit, device hygiene, emergency runbook

Each action produces a receipt and updates role readiness.

---

## 9) The personal closet (PII safety + dignity)

Every person has a **private closet**:

- drafts and private reasoning
- personal data not shared by default
- staging area for optional contribution

Only what is explicitly **donated** becomes a role patch.  
This prevents silent exploitation while still enabling measurable impact.

---

## 10) Ledger first (how everything stays clean)

Every action writes a ledger entry:

- **Who:** person + hat
- **What:** action label
- **Delta:** metric change
- **Risk weight:** low/medium/high
- **Receipt:** human-readable summary

This makes attribution, billing, and reversibility **structural**, not emotional.

---

## 11) MVP architecture (local-first, minimal dependencies)

**Frontend:** PWA (installable, offline)  
**Storage:** local DB + append-only ledger  
**Sync:** optional nucleus server for shared workspace  
**Invites:** signed scoped tokens (revocable)  
**Revocation:** patch deactivation + understudy fallback

This can start as **pure local-first** and grow into a distributed nucleus.

---

## 12) What this enables (the clean, necessary future)

- **Role excellence becomes licensable.**
- **People can revoke without destroying the org.**
- **Risk gets mapped and shared, not misattributed.**
- **Fit is measured by impact receipts, not social friction.**

This is not play-acting; it is **a new corporate grammar**:

- Roles are stable.
- Contributions are reversible.
- People are safe.
- Impact is measurable.

---

## 13) Next spec artifact (recommended)

**Role Attribution & Licensing Standard (RALS v0.1)**  
Defines:

- hat tokens
- patch ownership + revocation
- understudy rules
- ledger schema
- fit scoring weights
- risk rail definitions

This becomes the constitution of the system and is the next buildable deliverable.
