## Overview
`protocol::account` defines the account metadata that Codex surfaces when authenticating against hosted services. It captures the plan tier and distinguishes between API-key based accounts and ChatGPT-linked sessions.

## Detailed Behavior
- `PlanType` enumerates account tiers (Free through Enterprise/Edu) and includes an `Unknown` catch-all to remain forward compatible. Values are serialized in lowercase to match external APIs and TypeScript bindings.
- `Account` is a tagged enum with two variants:
  - `ApiKey` stores the raw key string for integrations that authenticate directly.
  - `ChatGpt` includes optional email and the associated `PlanType`, aligning with the ChatGPT account model.
- Both enums derive `Serialize`, `Deserialize`, `JsonSchema`, and `TS`, ensuring the same shapes are available to Rust and TypeScript clients.

## Broader Context
- These types are consumed by onboarding flows and telemetry that need to display or log the user’s subscription tier. Downstream specs (e.g., app-server endpoints) should reference these enums to avoid duplicating string constants.
- The `Unknown` tier ensures the protocol tolerates new plan names without immediate binary updates; clients should treat it as an opaque tier until documentation is updated.
- Context can't yet be determined for multi-tenant or organization membership extensions; future variants would extend this enum.

## Technical Debt
- None observed; the enums are minimal and aligned with current external contracts.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
