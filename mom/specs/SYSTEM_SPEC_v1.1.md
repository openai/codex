# SYSTEM_SPEC_v1.1 — Data Model + Invariants (M.O.M.)

Status: Draft  
Version: 1.1.0  
Applies to: M.O.M. (My Organic Memory) — MemNode™, Corridors™, Coda™, StagePort™

## 0. Purpose

This document defines the minimum required data structures and invariants for the M.O.M. memory system.
It is designed to:

- prevent silent retcons of memory
- enforce privacy boundaries by default
- ensure that recall, linking, and proactive nudges operate under explicit governance

## 1. Definitions (Normative)

**MUST / MUST NOT / SHOULD / MAY** are normative per RFC-style language.

### 1.1 MemNode™

A discrete governed memory unit, stored as encrypted payload + governed metadata, addressable by stable ID, and versioned via Coda™.

### 1.2 Corridors™

Typed relationships between MemNodes that constrain traversal, activation, and synthesis. Corridors are policy-bearing edges.

### 1.3 Coda™

Append-only continuity + provenance log for MemNodes and Corridors. Coda provides non-overwrite guarantees and auditability.

### 1.4 StagePort™

A fail-closed policy enforcement boundary that gates all operations: capture, read, link, traverse, synthesize, nudge, export, share, delete/redact.

---

## 2. Data Model (Minimum Required Fields)

### 2.1 MemNode Object (MVP)

A MemNode record MUST include:

- node_id: UUID (stable)
- created_at: ISO 8601 UTC timestamp
- updated_at: ISO 8601 UTC timestamp
- source_type: enum { voice, text, email, calendar, meeting, manual, import }
- payload_encrypted: bytes (opaque ciphertext)
- gist_encrypted: bytes OR gist_local: string (see StagePort policy)
- entities_index: array of entity_ref (hashed tokens permitted)
- time_range: { start?: timestamp, end?: timestamp, fuzzy?: boolean, confidence?: 0..1 }
- salience: float 0..1
- privacy_level: enum { private, shared, masked }
- consent_flags: object {
  recall: boolean,
  link: boolean,
  nudge: boolean,
  family_graph: boolean,
  export: boolean
  }
- coda_ref: { current_version_id: UUID }

Optional (recommended):

- tags: array<string>
- values: array<string> (e.g., "autonomy", "career", "health")
- location_hint: { geohash?: string, label?: string, confidence?: 0..1 }
- retention_policy: enum { keep, review_later, auto_expire }
- suppression: { do_not_surface_until?: timestamp }

### 2.2 Corridor Object (MVP)

A Corridor record MUST include:

- corridor_id: UUID
- from_node: UUID
- to_node: UUID
- type: enum {
  mentions,
  about,
  related_to,
  depends_on,
  conflicts_with,
  echoes,
  originates_from,
  belongs_to_project
  }
- weight: float 0..1
- created_at: ISO 8601 UTC timestamp
- policy: object {
  traversal_contexts: array<enum { recall, weaver, nudge, audit }>,
  requires_consent: boolean,
  min_privacy_level: enum { private, shared, masked }
  }
- justification_encrypted: bytes OR justification_local: string (see StagePort policy)
- coda_ref: { current_version_id: UUID }

Optional (recommended):

- decay: { half_life_days?: number } # link weight may decay unless reinforced
- provenance: { created_by: enum { user, agent }, model_id?: string }

### 2.3 Coda Version Record (MVP)

Coda MUST be append-only and MUST include:

- version_id: UUID
- object_type: enum { memnode, corridor }
- object_id: UUID
- prev_version_id: UUID | null
- change_type: enum { create, edit, redact, merge, split, delete_tombstone, restore }
- hash: string (cryptographic hash of canonicalized metadata + ciphertext pointers)
- author: enum { user, agent, system }
- timestamp: ISO 8601 UTC timestamp
- reason: string (short; MAY be encrypted)
- signature: optional string (for user-held signing keys)

### 2.4 StagePort Policy State (MVP)

StagePort MUST maintain:

- policy_version: semver string
- key_state: { local_key_present: boolean, hardware_backing?: boolean }
- allowed_integrations: array<string>
- nudge_mode: enum { off, conservative, standard }
- export_mode: enum { off, user_confirmed_only }
- family_mode: enum { off, enabled_with_consent }

---

## 3. Invariants (Non-negotiable Rules)

### 3.1 Privacy & Keys

- Payload MUST be encrypted at rest.
- Decryption keys MUST be user-controlled (“user-held key”) or user-authorized device keys.
- System MUST fail closed if keys are unavailable.

### 3.2 Non-Overwrite Guarantee (Coda)

- MemNode content MUST NOT be overwritten in place.
- Any change MUST create a new Coda version record.
- Prior versions MUST remain addressable (unless cryptographic deletion is invoked by user).

### 3.3 No Silent Sharing

- No MemNode MAY become shared without explicit user action.
- Shared and masked modes MUST have separate consent flags.
- Family graph ingestion MUST be opt-in per node.

### 3.4 Link Governance

- Agent MAY propose Corridors.
- Agent MUST NOT auto-activate Corridors across privacy boundaries.
- Corridors across privacy_level {private ↔ shared} MUST require user consent.

### 3.5 Recall Citations

- Any synthesized answer MUST cite its supporting MemNodes (node_ids or user-readable card references).
- StagePort MAY redact citations in “masked output” mode, but MUST preserve internal traceability.

### 3.6 Nudge Safety

- Nudges MUST follow RULESET_NUDGE_CONSTITUTION_v1.0.
- Nudges MUST NOT reveal masked/private content on lock screen or in shared contexts.

### 3.7 Deletion & Redaction

- “Delete” MUST be either:
  (a) tombstone (remove pointers + prevent surfacing), OR
  (b) cryptographic delete (destroy keys for payload)
- Redaction MUST preserve Coda continuity; redactions are versioned edits, not erasure.

### 3.8 Integrity & Drift Control (TacoBell clause)

- Linking heuristics MUST be bounded: traversal budgets, max corridor fanout, confidence thresholds.
- Regex / rules / transforms MUST be tested against worst-case performance (catastrophic backtracking prevention).
- Dependency updates MUST be pinned and reproducible (lockfiles required).

---

## 4. Core Operations (StagePort-Gated)

### 4.1 Capture(input)

StagePort MUST:

- encrypt payload
- extract gist (local preferred)
- extract entity tokens (hashed)
- assign initial salience + privacy defaults
- write MemNode + Coda(create)

### 4.2 ProposeLinks(node_id)

Agent MAY:

- scan for candidate links
- propose Corridors with justification + confidence
  StagePort MUST:
- reject links crossing privacy boundaries without consent
- write Corridors + Coda(create) only if permitted

### 4.3 Recall(query, context)

StagePort MUST:

- retrieve only permitted nodes
- provide citations
- log access event (optional, user-visible)

### 4.4 Nudge()

StagePort MUST:

- check nudge constitution
- check exposure context
- deliver minimal safe text, never raw private payload

### 4.5 Export()

StagePort MUST:

- require user confirmation
- allow export scopes (single node / project / date range)
- optionally produce redacted export

---

## 5. Versioning

- 1.1.x adds enforceable schemas and invariants.
- Any breaking schema change increments MAJOR.
- Any new optional field increments MINOR.

END.
