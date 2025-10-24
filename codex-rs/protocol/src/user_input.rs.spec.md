## Overview
`protocol::user_input` models the different kinds of content a user can send to Codex. It provides a tagged enum so downstream components can serialize text, remote images, or local file attachments uniformly.

## Detailed Behavior
- `UserInput` is marked `#[non_exhaustive]` to allow future variants without breaking binary compatibility. Current variants include:
  - `Text` with a UTF-8 string payload.
  - `Image` containing a pre-encoded data URI or external URL (`image_url`).
  - `LocalImage` capturing a `PathBuf` to a local file that later stages convert into a base64 data URI before sending to model APIs.
- Derives `Serialize`, `Deserialize`, `PartialEq`, `JsonSchema`, and `TS`, ensuring the same shape is available for TypeScript bindings and JSON contracts.

## Broader Context
- These inputs feed directly into request construction for model APIs and transcripts. The `LocalImage` variant ties into `models.rs`, where the actual encoding occurs; keeping the enum in this module prevents higher layers from needing to understand encoding rules.
- Because the enum is non-exhaustive, clients must handle unknown variants gracefully, which is important when rolling out new input types.
- Context can't yet be determined for streaming or structured inputs (audio, cursor positions); new variants will expand the enum as features mature.

## Technical Debt
- None observed; the enum cleanly captures current input types and is future-proofed via the non-exhaustive attribute.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./models.rs.spec.md
