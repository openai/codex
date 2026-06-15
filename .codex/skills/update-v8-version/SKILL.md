---
name: update-v8-version
description: Update Codex's pinned `v8` / `rusty_v8` versions, validate the release-candidate path, and investigate failed V8 canary or artifact builds. Use when asked to bump V8, update `rusty_v8` artifacts, prepare or validate a V8 release candidate, check `v8-canary`, or diagnose why a V8 version update no longer builds.
---

# Update V8 Version

## Core Workflow

1. Read `third_party/v8/README.md` and follow its version-bump sequence. Treat
   that document as the release-process source of truth.
2. Inspect and update the concrete repo surfaces that carry the pin:
   - `codex-rs/Cargo.toml`
   - `codex-rs/Cargo.lock`
   - `MODULE.bazel`
   - `third_party/v8/BUILD.bazel`
   - `third_party/v8/README.md`
   - the matching `third_party/v8/rusty_v8_<version>.sha256` manifest when the
     remaining prebuilt inputs change
3. Keep the existing checksum helpers in the loop:

   ```bash
   python3 .github/scripts/rusty_v8_bazel.py update-module-bazel
   python3 .github/scripts/rusty_v8_bazel.py check-module-bazel
   python3 -m unittest discover -s .github/scripts -p test_rusty_v8_bazel.py
   ```

4. Validate the release-candidate path before broadening the work:
   - Prefer checking the `v8-canary` CI result for the candidate branch or PR
     when one exists, using GitHub check tooling or `gh` as appropriate.
   - If CI is unavailable or the user asked for a local-only check, run the
     closest local validation that is practical for the changed surface and say
     explicitly that it is a local substitute, not the full hosted canary.
5. If the canary path passes, stop there. Summarize the result and encourage the
   user to commit the candidate changes or proceed with the release flow they
   requested. Do not publish tags, releases, or pushes unless the user asked.

## Failure Path

Enter this path only when the canary or local build path fails.

1. Capture the failing target, workflow job, and first actionable error.
2. Compare the currently pinned version with the target version at the relevant
   upstream tag or SHA. Inspect both:
   - `denoland/rusty_v8`
   - upstream V8 source at the target Bazel-pinned version
3. Track build-relevant deltas rather than broad source churn:
   - generated binding layout changes
   - archive or asset naming changes
   - GN/Bazel target changes
   - custom libc++ / libc++abi / llvm-libc inputs
   - sandbox or pointer-compression feature relationships
   - patch hunks in `patches/` that no longer apply or no longer match upstream
4. Trace each failing delta back into Codex's build graph:
   - `MODULE.bazel`
   - `third_party/v8/BUILD.bazel`
   - `.github/scripts/rusty_v8_bazel.py`
   - `.github/workflows/v8-canary.yml`
   - `.github/workflows/rusty-v8-release.yml`
5. Update only the pieces required to restore the target version's build and
   artifact contract. Keep patch explanations and doc changes close to the
   affected files.
6. Re-run the focused validation. If it becomes green, return to the normal
   workflow and stop with a concise summary plus the remaining release step.

## Reporting

- Say whether validation came from hosted `v8-canary` or from a local
  substitute.
- Distinguish "version bump complete" from "release published".
- When blocked, report the upstream delta that matters, the Codex file it hits,
  and the next concrete fix to try.
