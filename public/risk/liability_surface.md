# SYVAQ × AVC — Liability Surface & Responsibility Boundaries

Document ID: TDC-01-RISK-LS  
Audience: Senior Engineering, Risk, Legal, Technical Diligence  
Status: Canonical (v1)

## Purpose

This document defines the explicit responsibility boundaries of the Syvaq MVP and the AVC Systems Studio governance layer.

Its goal is not to eliminate risk, but to ensure that:

- Responsibility is deliberate, not implied
- Authority is bounded, not inferred
- Liability does not expand silently through technical design choices

This document should be read alongside boundary and failure documentation.

## System Roles (Explicit)

**End User**

- Initiates sessions
- Selects trusted contacts
- Retains situational judgment and agency

**Trusted Contacts**

- Receive escalation notifications
- Interpret signals contextually
- Decide whether and how to act

**Syvaq (Operating System Layer)**

- Transmits user-initiated signals
- Routes escalation based on configuration
- Maintains minimal event logs

**AVC Systems Studio (Governance Layer)**

- Defines licensing boundaries
- Establishes activation thresholds
- Enforces scope discipline and audit posture

No role subsumes another.

## Liability Boundary 1: No Duty of Protection

**Position**  
Syvaq does not assume a duty to protect, rescue, or intervene.

**Implication**

- The system is not a security service, emergency responder, or monitoring authority.
- Signals are informational, not guarantees of safety.

**Engineering Consequence**

- No “safety score,” “safe route,” or assurance language is used.
- UI avoids implied promises.

## Liability Boundary 2: No Delegated Authority

**Position**  
The system does not act on behalf of users in contacting authorities, employers, institutions, or third parties.

**Implication**

- No delegation of legal authority is created.
- No jurisdictional obligations are triggered.

**Engineering Consequence**

- All escalations terminate at user-selected human endpoints.
- No automated authority integrations exist.

## Liability Boundary 3: No Predictive or Advisory Claims

**Position**  
Syvaq does not provide advice, prediction, or recommendations.

**Implication**

- Outputs cannot be interpreted as professional judgment (security, legal, medical, or otherwise).

**Engineering Consequence**

- System language is descriptive (“check-in missed”) rather than evaluative (“danger detected”).
- Logs record events, not assessments.

## Liability Boundary 4: Data Custody & Interpretation

**Position**  
Syvaq retains limited custody of event metadata solely for operational and audit purposes.

**Implication**

- Data is not intended for behavioral inference or third-party analysis.
- Misinterpretation risk is mitigated through minimization and labeling.

**Engineering Consequence**

- Short retention windows.
- Clear schema separation between “event” and “interpretation.”

## Liability Boundary 5: Governance vs. Operations

**Position**  
AVC Systems Studio provides governance infrastructure, not operational command.

**Implication**

- AVC does not direct system behavior in real time.
- AVC does not control user interactions or data flows.

**Engineering Consequence**

- Governance logic is external, versioned, and revocable.
- No runtime dependency on AVC systems for core operation.

## Liability Boundary 6: Activation Threshold as Risk Gate

**Position**  
The $2,500 activation threshold marks the point at which professional liability may be assumed for licensed components.

**Implication**

- Below threshold: no representation, no deployment, no implied responsibility.
- Above threshold: responsibility is limited to explicitly licensed artifacts.

**Engineering Consequence**

- Feature exposure and deployment rights are gated.
- No partial or informal activation paths exist.

## Known Non-Responsibilities (Explicit)

The system does not assume responsibility for:

- User behavior before or after sessions
- Actions taken (or not taken) by trusted contacts
- External service availability (telecom, GPS, cloud)
- Interpretation of logs by third parties
- Outcomes following escalation notifications

These are structural, not contractual, exclusions.

## Why This Matters (Technical Perspective)

Liability expands most often through:

- Ambiguous automation
- Implicit authority
- Overconfident interfaces
- Silent scope drift

This system avoids those patterns by:

- Keeping humans in the loop
- Naming responsibility edges
- Designing for reversibility
- Treating governance as infrastructure, not policy

## Summary

The Syvaq MVP is intentionally narrow in responsibility and precise in authority.

This is not a limitation of ambition. It is the prerequisite for safe evolution.

Any future expansion of responsibility must:

- Be explicit
- Be reviewable
- Be revocable

Until then, the system remains legible, bounded, and inspectable.

## Final Line

A system that does not name its liability surface will eventually inherit one it did not intend.
