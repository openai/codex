# Standalone Installer Hardening Plan

This plan addresses the review feedback on the standalone installer rework.

The installer is moving Codex from a flat executable layout into a managed
release layout:

```text
$CODEX_HOME/
  packages/
    standalone/
      current -> releases/<version>-<target>
      releases/
        <version>-<target>/
          codex
          codex-resources/
            rg
```

That shape is still the right direction. The problems are in the transition
and activation mechanics. A standalone installer must make the new release
ready first, then switch users over in one safe step, and it must not destroy a
working install if anything fails before that switch.

## Goals

- A fresh install works on macOS, Linux, and Windows.
- A rerun of the same version is safe and idempotent.
- An upgrade switches from the old release to the new release atomically where
  the platform allows it.
- Existing users of the old standalone layout migrate without deleting files by
  hand.
- Concurrent installer runs cannot delete or half-activate each other's work.
- The visible `codex` command works before we offer to remove npm, bun, or
  brew installs.
- The TUI update path preserves the install choices that matter to future
  updates.

## Current Review Findings

### 1. Unix `current` symlink replacement is broken on macOS

`install.sh` creates a temporary symlink and then runs:

```sh
mv -f "$tmp_link" "$CURRENT_LINK"
```

On macOS/BSD `mv`, when `CURRENT_LINK` is a symlink to a directory, this can
move the temporary symlink into the existing target directory instead of
replacing `CURRENT_LINK`.

The bad result looks like this:

```text
standalone/current -> releases/old-version
standalone/releases/old-version/.current.<pid> -> releases/new-version
```

The user keeps running the old release.

Fix:

- Add `replace_symlink` in `install.sh`.
- Use `mv -T` when available.
- Use `mv -h` on macOS/BSD.
- If neither exists, remove the old symlink and rename the new symlink while
  holding the installer lock.

The fallback is not fully atomic, but the lock makes it safe from another
installer process. The normal Linux and macOS paths stay atomic.

### 2. Windows migration from the old standalone layout fails

The old Windows installer wrote real files into:

```text
%LOCALAPPDATA%\Programs\OpenAI\Codex\bin
  codex.exe
  rg.exe
  codex-command-runner.exe
  codex-windows-sandbox-setup.exe
```

The PR now tries to replace that `bin` directory with a junction. The helper
`Ensure-Junction` refuses to replace a non-empty directory, so existing users
can hit a hard failure.

Fix:

- Detect only the known old standalone `bin` layout at the default Windows
  install path.
- Ask the user before replacing that old layout.
- Move the old `bin` directory aside before creating the new junction.
- Delete the backup only after the new visible `codex.exe --version` check
  passes.
- Keep refusing unknown non-empty directories.

This gives existing standalone users a migration path without teaching the
installer to replace arbitrary user directories.

### 3. Concurrent installs can corrupt activation

Two installer processes can both decide a release is incomplete, both download
it, and both manipulate the same `release_dir`, `current`, and visible
command.

The risky Unix sequence is:

```sh
if [ -e "$release_dir" ] || [ -L "$release_dir" ]; then
  rm -rf "$release_dir"
fi
...
mv "$stage_release" "$release_dir"
```

Fix:

- Add one installer lock per standalone root.
- Hold the lock across:
  - release completeness check
  - staging
  - final release rename
  - `current` update
  - visible command update
  - install metadata write
- On Unix, prefer `flock` when available.
- On macOS without `flock`, use an atomic `mkdir "$lock_dir"` lock with a
  trap cleanup.
- On Windows, use a named mutex or an exclusive lock file opened with no share
  mode.

The lock should live under:

```text
$CODEX_HOME/packages/standalone/install.lock
```

### 4. Unix staging happens under `/tmp`

The Unix installer currently stages under `mktemp -d`, then moves the staged
release into `$RELEASES_DIR`.

If `/tmp` and `CODEX_HOME` are on different filesystems, the final move is a
copy plus delete. That is not the atomic activation model the installer claims.

Fix:

- Download and extract into a temp directory as today.
- Stage the final release directory under `$RELEASES_DIR`.
- Use a staging path like:

```text
$RELEASES_DIR/.staging.<release-name>.<pid>
```

- Rename the staging directory to the final release directory on the same
  filesystem.

The archive can still download into `/tmp`. The release directory that becomes
active must be staged beside the final destination.

### 5. Conflicting installs are removed too early

The installer currently offers to uninstall npm, bun, or brew Codex before the
standalone install has succeeded.

That ordering can remove a working `codex`, then fail during download,
extraction, activation, or PATH setup.

Fix:

1. Detect the conflicting manager-owned install early, but do not uninstall it.
2. Download, stage, and activate standalone.
3. Verify the visible `codex` command works:

```sh
"$BIN_PATH" --version
```

4. Only then ask whether to uninstall the old npm, bun, or brew install.

If the uninstall fails, keep the standalone install and print a warning about
PATH order.

### 6. No checksum verification

The installer downloads GitHub release tarballs over TLS and extracts them
directly. TLS protects transport, but the installer does not verify that the
archive matches an expected release digest.

Fix:

- Use GitHub's release asset `digest` field for the exact installer tarball.
- Verify the archive digest before extraction.
- Fail closed if the digest is missing or does not match.

GitHub already exposes SHA-256 digests for release assets, including
`codex-npm-<platform>-<version>.tgz`. A separate OpenAI-authored manifest can
still be a later supply-chain hardening step, but it is not required for basic
archive verification.

### 7. TUI update keeps a simple latest-install path

Standalone TUI updates rerun the generic installer command:

```sh
curl -fsSL https://chatgpt.com/codex/install.sh | sh
```

The installer accepts `CODEX_HOME` and `CODEX_INSTALL_DIR` as environment
overrides, but those are escape hatches rather than durable product
preferences. Recording them as installer state would make the updater replay
values that may only have been set for one shell session.

Fix:

- Keep runtime install detection based on `current_exe()`.
- Keep the TUI update command as a latest standalone installer rerun.
- Do not add an installer state file until there are real user-facing install
  preferences, such as a channel or update policy.

## Implementation Order

1. Add installer locks on Unix and Windows.
2. Move Unix release staging under `$RELEASES_DIR`.
3. Fix Unix symlink replacement.
4. Fix Windows visible `bin` migration by keeping `bin` as a real directory.
5. Move conflicting-install uninstall after standalone activation and
   verification.
6. Add install metadata and use it from the TUI update action.
7. Add checksum verification using GitHub's release asset digest.

The first five should land before this PR is considered safe to merge. The last
two can land in the same PR if the patch stays manageable; otherwise they should
be tracked as immediate follow-ups.

## Test Plan

### Unix tests

- Fresh install into isolated `HOME`, `CODEX_HOME`, and `CODEX_INSTALL_DIR`.
- Same-version rerun.
- Upgrade from fake old release to fake new release.
- macOS symlink replacement fixture:
  - create `current -> old-release`
  - update to `new-release`
  - assert `current` points to `new-release`
  - assert no `.current.*` file appears inside `old-release`
- Staging filesystem check:
  - assert final staging dir is created under `$RELEASES_DIR`
  - assert final activation uses rename from sibling staging dir
- Concurrent install smoke:
  - start two same-version installs against the same `CODEX_HOME`
  - assert both exit successfully
  - assert `current/codex --version` works
  - assert the release dir is complete

### Windows tests

- Fresh install into isolated `CODEX_HOME` and `CODEX_INSTALL_DIR`.
- Same-version rerun against the same `CODEX_HOME` and `CODEX_INSTALL_DIR`.
- Migration from old standalone layout:
  - create a real non-empty visible `bin` directory
  - include `codex.exe`, `rg.exe`, and helper binary names
  - run installer in non-interactive mode
  - assert installer asks for confirmation and fails closed
  - assert old files are preserved
- Unknown non-empty visible directory:
  - create a real directory with an extra user file
  - assert installer refuses to replace it
  - assert user file is preserved
- Same-version rerun.
- Concurrent install smoke with two PowerShell installer processes.
- TUI standalone update command on Windows:
  - assert PowerShell is called directly
  - assert `|iex` is passed to PowerShell, not `cmd /C`

### Failure-ordering tests

- Simulate a conflicting npm/bun/brew install.
- Force standalone download or extraction failure.
- Assert the old manager-owned install is not removed.
- Force standalone success and visible command verification success.
- Assert the uninstall prompt happens after verification.

### Manual smoke tests

- macOS fresh install and rerun.
- macOS upgrade between two locally served test releases.
- Linux fresh install and rerun.
- Windows fresh install.
- Windows migration from the old visible `bin` layout.

## Acceptance Criteria

- The installer never removes a working manager-owned Codex before standalone
  has been activated and verified.
- The Unix `current` link is replaced correctly on macOS and Linux.
- Release activation uses same-filesystem staging.
- Concurrent same-version installs leave a complete release and working visible
  command.
- Existing Windows standalone users do not need to delete their old install
  directory manually.
- TUI standalone update preserves the install directory choices from the
  original standalone install.
