# Codex Guardrails Demo Plan (Reduced Scope)

## Goal
Demonstrate a minimal first‑run guardrail flow with only two actions:
1) Ask the user their Codex sophistication level.
2) If they answer **low**, auto‑create `AGENTS.md` and `PLANS.md` (only for first‑time users).

## Demo Setup
- Start from a clean branch off master.
- Use a repo that does **not** already have `AGENTS.md` or `PLANS.md` at the root.
- Ensure the demo uses a fresh Codex home to mimic a first‑run user.

## Demo Script (Narrated Flow)

### 1) First‑Run Question: Sophistication Level
Prompt (first‑time users only):
> What’s your level of Codex sophistication? (low / medium / high)

Narration:
“This question only appears for first‑time users. It should not appear on subsequent runs.”

### 2) Auto‑scaffold Guardrails (only if low + first‑time)
If the user answers **low** and they are a first‑time user, Codex automatically creates:
- `AGENTS.md`
- `PLANS.md`

Narration:
“With a low sophistication choice on first run, Codex creates the guardrail files automatically.”

## Demo Success Criteria
- The sophistication question is asked **only** for first‑time users (unless forced by flag).
- If the user answers **low** on first run, `AGENTS.md` and `PLANS.md` are created automatically.
- No other onboarding steps, plan gates, or test execution are part of this demo.

## Notes for a Clean Demo
- To force the sophistication question on every run, use:
  ```
  codex --force-onboarding-question
  ```
- For a true first‑run experience:
  ```
  CODEX_HOME=$(mktemp -d) codex
  ```
