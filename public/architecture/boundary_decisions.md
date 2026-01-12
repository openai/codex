# SYVAQ × AVC — Boundary Decisions

Document ID: TDC-01-ARCH-BD  
Audience: Senior Engineering, Technical Diligence, Risk Review  
Status: Canonical (v1)

## Purpose

This document enumerates deliberate system boundary decisions made during the design of the Syvaq MVP. These decisions are not omissions; they are intentional exclusions chosen to reduce false authority, legal exposure, and irreversible coupling at an early stage.

The system is designed to be useful under uncertainty, not authoritative under incomplete data.

## Boundary 1: No Predictive Risk Scoring

**Decision**  
The MVP does not include probabilistic risk scoring, crime prediction, or behavioral inference models.

**Rationale**

- Available data at MVP scale is sparse, noisy, and highly context-dependent.
- Probabilistic outputs risk being interpreted as authoritative guidance by users or third parties.
- False precision in safety contexts introduces legal and ethical exposure disproportionate to value delivered.

**Consequence**

- The system operates as a signal and escalation layer, not a decision engine.
- Human judgment remains primary.

## Boundary 2: No Autonomous Action

**Decision**  
The system does not autonomously contact authorities, emergency services, or third-party responders.

**Rationale**

- Automated emergency escalation carries jurisdictional, liability, and consent complexity.
- User-defined trusted contacts provide contextual interpretation unavailable to automated responders.

**Consequence**

- All escalations are routed through explicitly configured human endpoints.
- Responsibility transfer is explicit, not implicit.

## Boundary 3: No Continuous Surveillance

**Decision**  
The system does not perform continuous background tracking outside of active user-initiated sessions.

**Rationale**

- Persistent surveillance increases privacy risk without proportional safety benefit.
- Session-bounded operation allows clearer consent, auditability, and user expectation alignment.

**Consequence**

- The system is event-scoped, not ambient.
- Data retention is minimal and purpose-limited.

## Boundary 4: No Social Graph or Network Effects

**Decision**  
The MVP does not include social feeds, discovery mechanisms, or user-to-user networking.

**Rationale**

- Social features introduce moderation, harassment, and amplification risks unrelated to core safety signaling.
- Network effects incentivize growth behaviors misaligned with safety-first objectives.

**Consequence**

- The system remains instrumental, not performative.
- Trust is local and user-defined.

## Boundary 5: No Monetization at the Safety Layer

**Decision**  
The MVP does not monetize user safety interactions or signals.

**Rationale**

- Monetization at the safety layer risks incentive distortion.
- Early revenue pressure encourages scope expansion before reliability is established.

**Consequence**

- Economic models are deferred to higher layers or adjacent services.
- Safety signaling remains non-transactional.

## Boundary 6: Externalized Governance

**Decision**  
Core governance, risk containment, and audit logic is externalized to AVC Systems Studio rather than embedded directly in application logic.

**Rationale**

- Separating governance from application code allows independent review, versioning, and revocation.
- Prevents silent scope creep under delivery pressure.

**Consequence**

- Governance can evolve without destabilizing application behavior.
- Liability boundaries remain legible.

## Summary

These boundaries are not constraints on ambition. They are load-bearing decisions that allow the system to exist safely before scale.

Future expansion is possible only where:

- Authority can be justified
- Failure can be contained
- Reversal remains feasible
