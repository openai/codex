# THREAT_MODEL v1.0 (M.O.M.)

Status: Draft  
Version: 1.0.0  
Purpose: Define threats, mitigations, and explicit refusals. This document builds user trust by stating what the system will NOT do.

## 0. Assets to Protect

- User’s memory payloads (encrypted content)
- Relationship graph (who/what/when connections)
- Derived inferences (salience, sentiment, patterns)
- Keys (local user-held secrets)

## 1. Adversaries

- External attackers (data theft, account takeover)
- Malicious insiders at service providers
- Curious partners/family members without consent
- Feature creep (product decisions that erode privacy)
- Supply chain attacks (dependency compromise)

## 2. Attack Surfaces

- Integrations (email/calendar/contacts)
- Sync layer / backups
- Notifications (lock screen leakage)
- LLM prompt injection via emails/web content
- Dependency ecosystem

## 3. Non-Negotiable Refusals (MUST NOT)

- No training on user private memory data by default.
- No selling or sharing memory data.
- No silent sharing to family plan or collaborators.
- No shadow profiles created from contacts without user consent.
- No emotion exploitation (nudges engineered to increase engagement at user expense).

## 4. Mitigations (MVP)

- End-to-end encryption; user-held keys.
- Fail-closed policy when keys absent.
- Explicit consent flags per node for: recall/link/nudge/family/export.
- Prompt-injection filtering for inbound content:
  - treat external text as untrusted
  - strip/neutralize “instructions” embedded in emails/webpages
- Dependency pinning + lockfiles + signed releases where possible.
- Notification redaction modes:
  - safe summary only
  - no private node content on lock screen

## 5. Incident Posture

If something goes wrong, the system MUST:

- default to disabling integrations
- preserve audit logs locally (user-controlled)
- provide a clear “what happened” report

## 6. Future (v1.1+)

- Hardware-backed keys (Secure Enclave/TPM)
- Differential privacy for optional analytics
- On-device embedding and retrieval

END.
