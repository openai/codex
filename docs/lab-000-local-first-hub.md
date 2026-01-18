# LAB_000 Local-First Hub: Build + Wiring Plan

## Purpose

LAB_000 is the local-first anchor that turns high-bandwidth ideation into bounded, actionable output. It provides a single, authoritative folder that:

- Receives intake without friction.
- Protects boundaries and IP.
- Enables safe, optional automation.
- Keeps all work usable from an iPhone-first workflow.

This document scopes what to build first, how to route it, and how to wire optional services without compromising autonomy.

## Design Principles

- **Local-first authority:** The canonical truth lives in LAB_000, not in external tools.
- **Single-entry intake:** All raw material lands in one place before sorting.
- **Fail-safe routing:** When unsure, route to rest or archive instead of exposure.
- **Minimal interfaces:** Reduce places that demand performance or re-explanation.
- **Automate only after authority is fixed:** Tools follow the packet; they do not define it.

## Core Structure (Load-Bearing)

```
LAB_000/
├── README_START_HERE.md
├── AUTHORITY/
│   ├── MANIFEST.md
│   └── BOUNDARIES.md
├── INTAKE/
│   └── DROPBOX/
├── STATE/
│   └── CURRENT_FOCUS.md
├── OUTPUT/
│   └── EXPORTS/
└── ARCHIVE/
```

### File Roles (Short + Operational)

- **README_START_HERE.md** — One-page briefing used in every chat.
- **AUTHORITY/MANIFEST.md** — Anti-extraction and consent rules.
- **AUTHORITY/BOUNDARIES.md** — Allowed outputs and prohibited behaviors.
- **INTAKE/DROPBOX/** — Unsorted incoming materials.
- **STATE/CURRENT_FOCUS.md** — Weekly objective, do-not-work list, next output.
- **OUTPUT/EXPORTS/** — Client-safe or publish-ready artifacts only.
- **ARCHIVE/** — Completed sessions, old drafts, and historical backups.

## iPhone-First Workflow (Operational Loop)

1. **Capture** → Add materials to `INTAKE/DROPBOX/` (screenshots, notes, voice memos).
2. **Signal** → Update `STATE/CURRENT_FOCUS.md` once per week.
3. **Runback** → Move one item at a time into `OUTPUT/EXPORTS/` only when finalized.
4. **Archive** → Rotate completed sessions into `ARCHIVE/` without re-reading.

## Wiring Strategy (Safe + Minimal)

Automation is optional and always downstream of LAB_000. The safest wiring is hub-and-spoke:

**Hub (LAB_000)** → **Spokes (tools for delivery only)**

### Recommended Spokes

- **ChatGPT / Codex**: Only reads the authoritative packet; never the full archive.
- **GitHub**: Stores public-safe exports or templates only.
- **Drive / Notion**: Distribution and collaboration surfaces, not authoritative storage.

### Minimum Safe Permissions

- **Read-only for AI** where possible.
- **Write only to OUTPUT/EXPORTS/** when you explicitly approve.
- **No direct write access** to AUTHORITY/ or STATE/ via automation.

## Suggested Automation Phases

### Phase 1 — Manual (Now)

- Use LAB_000 as the sole context packet in every session.
- Export only finalized outputs to external tools.
- No automated syncing until authority docs are stable.

### Phase 2 — Assisted (After Authority Lock)

- Create a short intake checklist for routing.
- Use scripted prompts that only produce outputs for OUTPUT/EXPORTS/.
- Keep manual control of publish steps.

### Phase 3 — Automated (Optional)

- Add a single dispatcher rule:
  - If output is marked “client-safe,” export it to one external channel.
  - If not marked, it stays local.

## Risk Controls (Non-Negotiable)

- **One-way exports only**: never allow external tools to overwrite LAB_000.
- **No auto-publication**: require explicit approval per export.
- **Keyhole access**: share only the minimal room needed (never the entire hub).

## Immediate Next Actions (Stable + Finite)

1. Draft README_START_HERE.md using a studio handbook tone.
2. Draft AUTHORITY/MANIFEST.md (anti-extraction + consent boundaries).
3. Draft AUTHORITY/BOUNDARIES.md (allowed vs. prohibited outputs).
4. Define a single “client-safe” marker used for export permissioning.

## Done Definition

LAB_000 is “done” when:

- The README and authority docs are locked.
- You can use a single sentence to start any session.
- Every output has a clear landing spot.
- External tools only receive what you explicitly export.
