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

## Patched Bash

We carry a small patch to `execute_cmd.c` (see `patches/bash-exec-wrapper.patch`) that adds support for `EXEC_WRAPPER`. The original commit message is “add support for BASH_EXEC_WRAPPER” and the patch applies cleanly to `a8a1c2fac029404d3f42cd39f5a20f24b6e4fe4b` from https://github.com/bminor/bash. To rebuild manually:

```bash
git clone https://github.com/bminor/bash
git checkout a8a1c2fac029404d3f42cd39f5a20f24b6e4fe4b
git apply /path/to/patches/bash-exec-wrapper.patch
./configure --without-bash-malloc
make -j"$(nproc)"
```
