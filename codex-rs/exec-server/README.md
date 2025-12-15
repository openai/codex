# codex-exec-server

This crate contains the code for two executables:

- `codex-exec-mcp-server` is an MCP server that provides a tool named `shell` that runs a shell command inside a sandboxed instance of Bash. Every resulting `execve(2)` call made within Bash is intercepted and run via the executable defined by the `BASH_EXEC_WRAPPER` environment variable within the Bash process. In practice, `BASH_EXEC_WRAPPER` is set to `codex-execve-wrapper`.
- `codex-execve-wrapper` is the executable that takes the arguments to the `execve(2)` call and "escalates" it to the MCP server via a shared file descriptor (specified by the `CODEX_ESCALATE_SOCKET` environment variable) for consideration. Based on the [Codex `.rules`](https://developers.openai.com/codex/local-config#rules-preview), the MCP server replies with one of:
  - `Run`: `codex-execve-wrapper` should invoke `execve(2)` on itself to run the original command within Bash
  - `Escalate`: forward the file descriptors of the current process to the MCP server so the command can be run faithfully outside the sandbox. Because the MCP server will have the original FDs for `stdout` and `stderr`, it can write those directly. When the process completes, the MCP server forwards the exit code to `codex-execve-wrapper` so that it exits in a consistent manner.
  - `Deny`: the MCP server has declared the proposed command to be "forbidden," so `codex-execve-wrapper` will print an error to `stderr` and exit with `1`.

## Patched Bash

We carry a small patch to `execute_cmd.c` (see `patches/bash-exec-wrapper.patch`) that adds support for `BASH_EXEC_WRAPPER`. The original commit message is “add support for BASH_EXEC_WRAPPER” and the patch applies cleanly to `a8a1c2fac029404d3f42cd39f5a20f24b6e4fe4b` from https://github.com/bminor/bash. To rebuild manually:

```bash
git clone https://github.com/bminor/bash
git checkout a8a1c2fac029404d3f42cd39f5a20f24b6e4fe4b
git apply /path/to/patches/bash-exec-wrapper.patch
./configure --without-bash-malloc
make -j"$(nproc)"
```

## Release workflow

`.github/workflows/shell-tool-mcp.yml` builds the Rust binaries, compiles the patched Bash variants, assembles the `vendor/` tree, and creates `codex-shell-tool-mcp-npm-<version>.tgz` for inclusion in the Rust GitHub Release. When the version is a stable or alpha tag, the workflow also publishes the tarball to npm using OIDC. The workflow is invoked from `rust-release.yml` so the package ships alongside other Codex artifacts.
