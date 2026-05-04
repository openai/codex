---
name: codex-issue-digest
description: Run a GitHub issue digest for openai/codex by feature-area labels, all areas, and configurable time windows. Use when asked to summarize recent Codex bug reports or enhancement requests, especially for owner-specific labels such as tui, exec, app, or similar areas.
---

# Codex Issue Digest

## Objective

Produce a headline-first, insight-oriented digest of `openai/codex` issues for the requested feature-area labels over the previous 24 hours by default. Honor a different duration when the user asks for one, for example "past week" or "48 hours". Default to a summary-only response; include details only when requested.

Include only issues that currently have `bug` or `enhancement` plus at least one requested owner label. If the user asks for all areas or all labels, collect `bug`/`enhancement` issues across all labels.

## Inputs

- Feature-area labels, for example `tui exec`
- `all areas` / `all labels` to scan all current feature labels
- Optional repo override, default `openai/codex`
- Optional time window, default previous 24 hours; examples: `48h`, `7d`, `1w`, `past week`

## Workflow

1. Run the collector from a current Codex repo checkout:

```bash
python3 .codex/skills/codex-issue-digest/scripts/collect_issue_digest.py --labels tui exec --window-hours 24
```

Use `--window "past week"` or `--window-hours 168` when the user asks for a non-default duration. Use `--all-labels` when the user says all areas or all labels.

2. Use the JSON as the source of truth. It includes new issues, new issue comments, new reactions/upvotes, current labels, current reaction counts, model-ready `summary_inputs`, and detailed `digest_rows`.
3. Choose the output mode from the user's request:
   - Default mode: start the report with `## Summary` and do not emit `## Details`.
   - Details-upfront mode: if the user asks for details, a table, a full digest, "include details", or similar, start with `## Summary`, then include `## Details`.
   - Follow-up details mode: if the user asks for more detail after a summary-only digest, produce `## Details` from the existing collector JSON when it is still available; otherwise rerun the collector.
4. In `## Summary`, write a headline-first executive summary:
   - The first nonblank line under `## Summary` must be a single-line headline or judgment, not a bullet. It should be useful even if the reader stops there.
   - On quiet days, prefer exactly: `No major issues reported by users.` Use this when there are no elevated rows, no newly repeated theme, and nothing that needs owner action.
   - When users are surfacing notable issues, make the headline name the count or theme, for example `Two issues are being surfaced by users:`.
   - Immediately under an active headline, list only the issues or themes driving attention, ordered by importance. Start each line with the row's `attention_marker` when present, then a concise owner-readable description and inline issue refs.
   - Treat `🔥🔥` as headline-worthy and `🔥` as elevated. Do not add fire emoji yourself; only copy the row's `attention_marker`.
   - Keep any extra summary detail after the headline to 1-3 terse lines, only when it adds a decision-relevant caveat, repeated theme, or owner action.
   - Do not include routine counts, broad stats, or low-signal table summaries in `## Summary` unless they change the headline. Put metadata and optional counts in `## Details` or the footer.
   - In default mode, end the report with a concise prompt such as `Want details? I can expand this into the issue table.` Keep this separate from the summary headline so the headline stays clean.
   - Cluster and name themes yourself from `summary_inputs`; the collector intentionally does not hard-code issue categories.
   - Use a cluster only when the issues genuinely share the same product problem. If several issues merely share a broad platform or label, describe them individually.
   - Do not omit a repeated theme just because its individual issues fall below the details table cutoff. Several similar reports should be called out as a repeated customer concern.
   - For single-issue rows, summarize the concern directly instead of calling it a cluster.
   - Use inline numbered issue links from each relevant row's `ref_markdown`.
   - Example quiet summary:

```markdown
## Summary
No major issues reported by users.

Source: collector v4, git `abc123def456`, window `2026-04-27T00:00:00Z` to `2026-04-28T00:00:00Z`.
Want details? I can expand this into the issue table.
```

   - Example active summary:

```markdown
## Summary
Two issues are being surfaced by users:
🔥🔥 Terminal launch hangs on startup [1](https://github.com/openai/codex/issues/123)
🔥 Resume switches model providers unexpectedly [2](https://github.com/openai/codex/issues/456)

Source: collector v4, git `abc123def456`, window `2026-04-27T00:00:00Z` to `2026-04-28T00:00:00Z`.
Want details? I can expand this into the issue table.
```
5. In `## Details`, when details are requested, include a compact table only when useful:
   - Prefer rows from `digest_rows`; include a `Refs` column using each row's `ref_markdown`.
   - Keep the table short; omit low-signal rows when the summary already covers them.
   - Use compact columns such as marker, area, type, description, interactions, and refs.
   - The `Description` cell should be a short owner-readable phrase. Use row `description`, title, body excerpts, and recent comments, but do not mechanically copy the raw GitHub issue title when it contains incidental details.
   - A clear quiet/no-concern sentence when there is no meaningful signal.
6. Use the JSON `attention_marker` exactly. It is empty for normal rows, `🔥` for elevated rows, and `🔥🔥` for very high-attention rows. The actual cutoffs are in `attention_thresholds`.
7. Use inline numbered references where a row or bullet points to issues, for example `Compaction bugs [1](https://github.com/openai/codex/issues/123), [2](https://github.com/openai/codex/issues/456)`. Do not add a separate footnotes section.
8. Label `interactions` as `Interactions`; it counts posts/comments/reactions during the requested window, not unique people.
9. Mention the collector `script_version`, repo checkout `git_head`, and time window in one compact source line. In default mode, put this before the details prompt so the final line still asks whether the user wants details. In details-upfront mode, it can be the footer.

## Reaction Handling

The collector uses GitHub reactions endpoints, which include `created_at`, to count reactions created during the digest window for hydrated issues. It reports both in-window reaction counts and current reaction totals. Treat current reaction totals as standing engagement, and treat `new_reactions` / `new_upvotes` as windowed activity.

By default, the collector fetches issue comments with `since=<window start>` and caps the number of comment pages per issue. This keeps very long historical threads from dominating a digest run and focuses the report on recent posts. Use `--fetch-all-comments` only when exhaustive comment history is more important than runtime.

GitHub issue search is still seeded by issue `updated_at`, so a purely reaction-only issue may be missed if reactions do not bump `updated_at`. Covering every reaction-only case would require either a persisted snapshot store or a broader scan of labeled issues.

## Attention Markers

The collector scales attention markers by the requested time window. The baseline is 5 human user interactions for `🔥` and 10 for `🔥🔥` over 24 hours; longer or shorter windows scale those cutoffs linearly and round up. For example, a one-week report uses 35 and 70 interactions. Human user interactions are human-authored new issue posts, human-authored new comments, and human reactions created during the window, including upvotes. Bot posts and bot reactions are excluded. In prose, explain this as high user interaction rather than naming the emoji.

## Freshness

The automation should run from a repo checkout that contains this skill. For shared daily use, prefer one of these patterns:

- Run the automation in a checkout that is refreshed before the automation starts, for example with `git pull --ff-only`.
- If the automation cannot safely mutate the checkout, have it report the current `git_head` from the collector output so readers know which skill/script version produced the digest.

## Sample Owner Prompt

```text
Use $codex-issue-digest to run the Codex issue digest for labels tui and exec over the previous 24 hours.
```

```text
Use $codex-issue-digest to run the Codex issue digest for all areas over the past week.
```

## Validation

Dry run the collector against recent issues:

```bash
python3 .codex/skills/codex-issue-digest/scripts/collect_issue_digest.py --labels tui exec --window-hours 24
```

```bash
python3 .codex/skills/codex-issue-digest/scripts/collect_issue_digest.py --all-labels --window "past week" --limit-issues 10
```

Run the focused script tests:

```bash
pytest .codex/skills/codex-issue-digest/scripts/test_collect_issue_digest.py
```
