# codex-shell-escalation

This crate contains the Unix shell-escalation protocol implementation and the
`codex-execve-wrapper` executable.

`codex-execve-wrapper` receives the arguments to an intercepted `execve(2)` call and delegates the
decision to the shell-escalation protocol over a shared file descriptor (specified by the
`CODEX_ESCALATE_SOCKET` environment variable). The server on the other side replies with one of:

- `Run`: `codex-execve-wrapper` should invoke `execve(2)` on itself to run the original command
  within the sandboxed shell.
- `Escalate`: forward the file descriptors of the current process so the command can be run
  faithfully outside the sandbox. When the process completes, the server forwards the exit code
  back to `codex-execve-wrapper`.
- `Deny`: the server has declared the proposed command to be forbidden, so
  `codex-execve-wrapper` prints an error to `stderr` and exits with `1`.

Both patched shells use `EXEC_WRAPPER` to locate `codex-execve-wrapper` and
preserve `CODEX_ESCALATE_SOCKET` as the inherited escalation socket file
descriptor. This shared environment contract lets intercepted commands pass
through trees containing either shell without losing the escalation session.

## Patched bash

We carry a small patch to `execute_cmd.c` (see
`patches/bash-exec-wrapper.patch`) that adds support for `EXEC_WRAPPER`. The
patch applies to `a8a1c2fac029404d3f42cd39f5a20f24b6e4fe4b` from
https://git.savannah.gnu.org/git/bash. To rebuild manually:

```bash
git clone https://git.savannah.gnu.org/git/bash
git checkout a8a1c2fac029404d3f42cd39f5a20f24b6e4fe4b
git apply /path/to/patches/bash-exec-wrapper.patch
./configure --without-bash-malloc
make -j"$(nproc)"
```

Release artifacts are built by `.github/workflows/rust-release-bash.yml` when
a `codex-bash-vX.Y.Z` tag is pushed. When the bash commit or patch changes,
publish the next version tag.

## Patched zsh

We carry a small patch to `Src/exec.c` (see `patches/zsh-exec-wrapper.patch`) that adds support for `EXEC_WRAPPER`. The patch applies to `77045ef899e53b9598bebc5a41db93a548a40ca6` from https://git.code.sf.net/p/zsh/code. To rebuild manually:

```bash
git clone https://git.code.sf.net/p/zsh/code
git checkout 77045ef899e53b9598bebc5a41db93a548a40ca6
git apply /path/to/patches/zsh-exec-wrapper.patch
./Util/preconfig
./configure
make -j"$(nproc)"
```

Release artifacts are built by `.github/workflows/rust-release-zsh.yml` when a
`codex-zsh-vX.Y.Z` tag is pushed. When the zsh commit or patch changes, publish
the next version tag and update the checked-in DotSlash manifests to use the new
release.
