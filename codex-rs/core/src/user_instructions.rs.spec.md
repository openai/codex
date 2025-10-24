## Overview
`core::user_instructions` wraps raw instruction text in the tagged block consumed by Codex models. It mirrors the environment-context serialization so the model can distinguish system guidance from conversational content.

## Detailed Behavior
- `UserInstructions` stores the raw instruction string. `new` accepts any `Into<String>` to accommodate borrowed or owned data.
- `serialize_to_xml` produces:
  ```
  <user_instructions>

  …

  </user_instructions>
  ```
  using protocol constants; blank lines around the body keep formatting consistent with environment context tags.
- `From<UserInstructions> for ResponseItem` creates a user-role message containing the serialized block, matching the format expected by prompt assembly.

## Broader Context
- `codex.rs` merges project docs (`project_doc.rs`) and user-provided overrides into a single instruction string before converting it with this module. Client specs describe how the resulting `ResponseItem` feeds into the model payload.
- Maintaining consistent tagging between environment context and user instructions helps downstream parsers (model-side tools, analytics) identify instruction blocks without manual parsing.
- Context can't yet be determined for instruction metadata (e.g., priority), but the XML-like wrapper provides a natural place to add sub-elements if needed.

## Technical Debt
- Tag wrapping mirrors logic in `environment_context`; extracting a shared helper for tagged blocks would reduce duplication and ensure formatting stays aligned.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Deduplicate the tag-serialization pattern shared with `environment_context` to reduce drift when tags change.
related_specs:
  - ./environment_context.rs.spec.md
  - ./project_doc.rs.spec.md
  - ./client_common.rs.spec.md
