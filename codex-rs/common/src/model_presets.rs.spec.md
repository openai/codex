## Overview
`common::model_presets` centralizes metadata about Codex-supported language models. It introduces `ReasoningEffortPreset` and `ModelPreset` structs and exposes `builtin_model_presets` to deliver the curated list to front-end surfaces such as the TUI model picker and onboarding flows.

## Detailed Behavior
- `ReasoningEffortPreset` pairs a `ReasoningEffort` enum value with a short human description so interfaces can explain the trade-offs between effort levels.
- `ModelPreset` captures identifiers, display names, descriptions, default reasoning effort, the supported effort presets, and whether the preset is the workspace default.
- The static `PRESETS` slice defines two presets (`gpt-5-codex` and `gpt-5`) with tailored reasoning effort menus. Each entry references `ReasoningEffortPreset` literals stored in the same slice for zero-cost sharing across calls.
- `builtin_model_presets` currently ignores its `AuthMode` argument and returns `PRESETS.to_vec()`, cloning the slice into an owned vector for consumers that need mutability or ownership transfer.
- A unit test enforces the invariant that exactly one preset is flagged as the default, guarding against regressions when updating the array.

## Broader Context
- The list feeds multiple user interfaces and API clients; additions or removals should be coordinated with backend availability and pricing disclosures.
- The unused `AuthMode` parameter hints at future tailoring of available models based on authentication context. Consumers should expect the output set to vary once that logic is implemented.
- Context can't yet be determined for how regional deployments or enterprise entitlements will alter the preset catalog; revisit when those systems gain documentation.

## Technical Debt
- `builtin_model_presets` accepts `AuthMode` but does not currently filter presets. This placeholder should either be implemented or removed to avoid confusion when reading call sites.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Implement or remove `AuthMode` filtering in `builtin_model_presets` to reflect actual behavior.
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./config_summary.rs.spec.md
