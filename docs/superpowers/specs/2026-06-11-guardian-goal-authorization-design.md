# Guardian Goal Authorization Evidence

## Problem

Guardian's approval policy already treats a necessary implementation of an explicitly requested operation as strong user-authorization evidence. However, Guardian's transcript builder drops all contextual user messages. Persisted goal continuations are contextual messages, so Guardian does not see a user-provided goal even when the goal explicitly names the action under review.

In CODEX-21G8, the active goal required rebuilding `Shadow-0.4.dmg` and updating GitHub release `v0.4.0` with the new binary. Guardian did not receive that objective and repeatedly classified the upload as `user_authorization = "unknown"`. After the user repeated authorization in an ordinary chat message, Guardian classified the same upload as `high` authorization and allowed it.

## Goals

- Preserve a persisted goal's user-provided objective as authorization evidence in Guardian review.
- Keep unrelated contextual scaffolding out of the Guardian transcript.
- Avoid weakening Guardian's existing risk or authorization thresholds.
- Avoid repeatedly sending an unchanged objective on every automatic continuation.

## Non-goals

- Automatically approve every action that advances a goal.
- Treat model-authored plans, tool output, or assistant statements as user authorization.
- Change tenant policy for public release publication or other high-risk actions.
- Include full goal-continuation instructions in Guardian context.

## Design

Extend contextual-fragment parsing with a narrow parser for canonical internal context envelopes. In Guardian transcript collection, recognize only internal context whose trusted source label is `goal`, then extract only the contents of `<objective>` or `<untrusted_objective>`.

Represent the extracted text as a distinct `UserGoal` transcript entry rendered as `user-provided goal`. Treat this entry like a user entry for transcript-retention priority, but keep its distinct label so the reviewer can distinguish a persisted objective from a fresh chat message.

Track the most recently retained goal objective while walking history. Skip consecutive repetitions of the same objective and append a new entry when the objective changes. This preserves initial creation and later user edits without filling the transcript with automatic-continuation duplicates.

Do not change `policy_template.md` initially. Its existing definition of `high` authorization already covers necessary implementations of explicitly requested operations. The defect is missing evidence, not an insufficient threshold rule.

## Data Flow

1. The goal extension injects `<codex_internal_context source="goal">` into parent history.
2. Guardian transcript collection recognizes that canonical envelope before applying the generic contextual-message exclusion.
3. The collector extracts the objective and emits a `UserGoal` entry.
4. Guardian receives the labeled objective alongside the exact proposed action.
5. Guardian applies the existing risk and authorization policy.

## Security and Failure Handling

- Accept only a well-formed canonical internal-context envelope with source exactly `goal`.
- Extract only the objective element; discard continuation rules, budgets, and other runtime instructions.
- Treat the objective as untrusted evidence, consistent with the rest of the Guardian transcript.
- If parsing fails or the objective is empty, omit it rather than forwarding the full contextual message.
- Preserve current behavior for environment, skill, extension, and other contextual messages.
- A goal is evidence of requested outcome, not blanket authorization. Guardian must still compare the proposed target and side effects with the objective under its existing policy.

## Testing

Add focused unit coverage for transcript collection and prompt construction:

- Ordinary contextual messages remain excluded.
- A canonical goal continuation contributes only the objective.
- An edited goal using `<untrusted_objective>` contributes the updated objective.
- Repeated identical continuations produce one retained objective entry.
- A changed objective produces a later distinct entry.
- Malformed, empty, non-goal, and unrelated contextual messages remain excluded.
- A full Guardian prompt for `gh release upload v0.4.0 Shadow-0.4.dmg` includes the matching user-provided release objective and the planned action.
- An unrelated action fixture demonstrates that the objective and action remain separately labeled; no deterministic code path pre-classifies it as authorized.

Run the targeted `codex-core` Guardian tests, then formatting and the repository's normal focused checks for the touched crates. Add or run a model-level replay/eval separately to verify that the matching release-upload case produces high authorization while an unrelated upload remains low or unknown.

## Files Expected to Change

- `codex-rs/core/src/context/internal_model_context.rs`: expose narrow parsing/access for canonical internal context.
- `codex-rs/core/src/context/mod.rs`: export the parser or parsed fragment access needed by Guardian.
- `codex-rs/core/src/guardian/prompt.rs`: extract, deduplicate, label, and retain goal objectives.
- `codex-rs/core/src/guardian/tests.rs`: unit and prompt-level regression coverage.

## Success Criteria

- CODEX-21G8's goal objective would be visible in the approval-review prompt before the release upload decision.
- Existing contextual-message filtering remains intact.
- No risk threshold or automatic allow path is added.
- Targeted tests pass and snapshots, if affected, are intentionally updated.
