## Overview
Represents the account information shown in the status pane. Distinguishes between ChatGPT-based accounts (with optional email and plan) and API-key flows without extra metadata.

## Detailed Behavior
- `StatusAccountDisplay` enum:
  - `ChatGpt { email, plan }` captures optional user email and subscription plan strings fetched from the login service.
  - `ApiKey` indicates authentication via API key where no extra UI metadata is available.

## Broader Context
- Used by the status widgets to render account summaries and show whether features like login/logout are available.
- Paired with other status module structs (`card.rs`, `format.rs`) to assemble the complete status view.

## Technical Debt
- Only models a subset of potential account types; future sources (e.g., enterprise SSO) would require extending the enum.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Expand variants when new authentication providers surface so the UI can differentiate them cleanly.
related_specs:
  - card.rs.spec.md
  - helpers.rs.spec.md
