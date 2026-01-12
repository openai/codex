# SYVAQ × AVC — Design Rationale

Document ID: TDC-01-ARCH-DR  
Audience: Principal Engineers, Technical VCs, Diligence Review  
Status: Canonical (v1)

## Purpose

This document explains why the Syvaq MVP architecture is shaped as it is, by contrasting it against plausible alternatives that were explicitly rejected.

The goal is not to claim optimality, but to demonstrate constraint awareness, trade-off literacy, and reversibility.

## Architecture Considered: Predictive Safety Intelligence Platform

**Description**  
A system using historical data, behavioral signals, and third-party datasets to generate probabilistic safety risk scores and proactive alerts.

**Why It Was Rejected**

- Requires large, representative datasets unavailable at MVP stage.
- Introduces false authority under uncertainty.
- Creates implicit duty of care through predictive language.
- High regulatory and legal exposure relative to early value.

**Conclusion**  
Rejected due to authority risk exceeding informational benefit.

## Architecture Considered: Always-On Background Monitoring

**Description**  
Continuous GPS and sensor tracking with automatic escalation on anomaly detection.

**Why It Was Rejected**

- Persistent surveillance increases privacy and consent risk.
- Battery and reliability constraints degrade user trust.
- “Always-on” framing shifts responsibility from user to system.

**Conclusion**  
Rejected to preserve event-scoped consent and agency.

## Architecture Considered: Emergency Services Integration

**Description**  
Direct integration with 911 / emergency response systems triggered by system conditions.

**Why It Was Rejected**

- Jurisdictional fragmentation and compliance burden.
- High liability for false positives.
- Inflexible escalation paths inappropriate for ambiguous situations.

**Conclusion**  
Rejected in favor of user-defined human interpretation.

## Selected Architecture: Session-Bound Signal & Escalation Layer

**Description**  
A user-initiated, session-bounded system that:

- Emits time-scoped signals.
- Routes missed check-ins to trusted contacts.
- Logs minimal, auditable events.

**Why This Was Chosen**

- Aligns authority with user intent.
- Keeps humans in the loop.
- Scales governance before features.
- Allows future augmentation without re-architecting responsibility.

**Key Property**  
The system can be expanded without being contradicted by its MVP claims.

## Reversibility as a First-Class Constraint

The chosen architecture allows:

- Feature addition without revoking prior assurances.
- Removal or rollback without orphaned obligations.
- Governance updates without runtime dependency.

This was treated as a design requirement, not an afterthought.

## Summary

This architecture was chosen not because it is maximal, but because it is:

- Legible under scrutiny.
- Honest about uncertainty.
- Defensible under failure.
- Expandable without ethical debt.

Systems that survive diligence are rarely the loudest ones.
