# SYVAQ × AVC — Deployment Modes

Document ID: TDC-01-OPS-DM  
Audience: Engineering, Risk, Operations  
Status: Canonical (v1)

## Purpose

This document defines the operational deployment modes for the Syvaq MVP. Each mode limits scope, exposure, and implied responsibility, while maintaining a clear upgrade path.

## Mode 1: Demo

**Intent**  
Demonstrate execution competence without implying operational coverage.

**Characteristics**

- Proof surfaces only (static demos, sandboxed flows).
- No production data.
- No reliability guarantees.

**Boundary**  
Demo mode is non-operational by definition.

## Mode 2: Pilot

**Intent**  
Validate core signaling behavior under controlled, opt-in usage.

**Characteristics**

- Small, known user cohort.
- Defined escalation paths to trusted contacts.
- Minimal, auditable logging.
- Governance posture active and reviewable.

**Boundary**  
Pilot mode introduces responsibility only within explicitly licensed scope.

## Mode 3: Scale (Deferred)

**Intent**  
Expand usage only after reliability and governance gates are satisfied.

**Characteristics**

- Hardened operational posture.
- Auditable retention policies.
- Formalized support and incident response.

**Boundary**  
Scale is deferred until evidence supports higher duty of care.

## Summary

Deployment mode is a risk gate. Every transition requires proof, not optimism.
