# @openai/codex-shell-tool-mcp

This package wraps the `codex-exec-mcp-server` binary and its helpers so that the shell MCP can be invoked via `npx @openai/codex-shell-tool-mcp`. It bundles:

- `codex-exec-mcp-server` and `codex-execve-wrapper` built for macOS (arm64, x64) and Linux (musl arm64, musl x64).
- A patched Bash that honors `BASH_EXEC_WRAPPER`, built for multiple glibc baselines (Ubuntu 24.04/22.04/20.04, Debian 12/11, CentOS-like 9) and macOS (15/14/13).
- A launcher (`bin/mcp-server.js`) that picks the correct binaries for the current `process.platform` / `process.arch`, specifying `--execve` and `--bash` for the MCP, as appropriate.

## Usage

```bash
npx @openai/codex-shell-tool-mcp --help
```

The launcher selects a Rust target triple based on the host and chooses the closest Bash variant by inspecting `/etc/os-release` on Linux or the Darwin major version on macOS.

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
