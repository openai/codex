# @openai/codex-shell-tool-mcp

**Note: This MCP server is still experimental. When using it with Codex CLI, ensure the CLI version matches the MCP server version.**

`@openai/codex-shell-tool-mcp` is an MCP server that provides a tool named `shell` that runs a shell command inside a sandboxed instance of Bash. This special instance of Bash intercepts requests to spawn new processes (specifically, [`execve(2)`](https://man7.org/linux/man-pages/man2/execve.2.html) calls). For each call, it makes a request back to the MCP server to determine whether to allow the proposed command to execute. It also has the option of _escalating_ the command to run unprivileged outside of the sandbox governing the Bash process.

The user can use [Codex `.rules`](https://developers.openai.com/codex/local-config#rules-preview) files to define how a command should be handled. The action to take is determined by the `decision` parameter of a matching rule as follows:

- `allow`: the command will be _escalated_ and run outside the sandbox
- `prompt`: the command will be subject to human approval via an [MCP elicitation](https://modelcontextprotocol.io/specification/draft/client/elicitation) (it will run _escalated_ if approved)
- `forbidden`: the command will fail with exit code `1` and an error message will be written to `stderr`

Commands that do not match an explicit rule in `.rules` will be allowed to run as-is, though they will still be subject to the sandbox applied to the parent Bash process.

## Motivation

When a software agent asks if it is safe to run a command like `ls`, without more context, it is unclear whether it will result in executing `/bin/ls`. Consider:

- There could be another executable named `ls` that appears before `/bin/ls` on the `$PATH`.
- `ls` could be mapped to a shell alias or function.

Because `@openai/codex-shell-tool-mcp` intercepts `execve(2)` calls directly, it _always_ knows the full path to the program being executed. In turn, this makes it possible to provide stronger guarantees on how [Codex `.rules`](https://developers.openai.com/codex/local-config#rules-preview) are enforced.

## Usage

First, verify that you can download and run the MCP executable:

```bash
npx -y @openai/codex-shell-tool-mcp --version
```

To test out the MCP with a one-off invocation of Codex CLI, it is important to _disable_ the default shell tool in addition to enabling the MCP so Codex has exactly one shell-like tool available to it:

```bash
codex --disable shell_tool \
  --config 'mcp_servers.bash={command = "npx", args = ["-y", "@openai/codex-shell-tool-mcp"]}'
```

To configure this permanently so you can use the MCP while running `codex` without additional command-line flags, add the following to your `~/.codex/config.toml`:

```toml
[features]
shell_tool = false

[mcp_servers.shell-tool]
command = "npx"
args = ["-y", "@openai/codex-shell-tool-mcp"]
```

Note when the `@openai/codex-shell-tool-mcp` launcher runs, it selects the appropriate native binary to run based on the host OS/architecture. For the Bash wrapper, it inspects `/etc/os-release` on Linux or the Darwin major version on macOS to try to find the best match it has available. See [`bashSelection.ts`](https://github.com/openai/codex/blob/main/shell-tool-mcp/src/bashSelection.ts) for details.

## MCP Client Requirements

This MCP server is designed to be used with [Codex](https://developers.openai.com/codex/cli), as it declares the following `capability` that Codex supports when acting as an MCP client:

```json
{
  "capabilities": {
    "experimental": {
      "codex/sandbox-state": {
        "version": "1.0.0"
      }
    }
  }
}
```

This capability means the MCP server honors requests like the following to update the sandbox policy the MCP server uses when spawning Bash:

```json
{
  "id": "req-42",
  "method": "codex/sandbox-state/update",
  "params": {
    "sandboxPolicy": {
      "type": "workspace-write",
      "writable_roots": ["/home/user/code/codex"],
      "network_access": false,
      "exclude_tmpdir_env_var": false,
      "exclude_slash_tmp": false
    }
  }
}
```

Once the server has processed the update, it sends an empty response to acknowledge the request:

```json
{
  "id": "req-42",
  "result": {}
}
```

The Codex harness (used by the CLI and the VS Code extension) sends such requests to MCP servers that declare the `codex/sandbox-state` capability.

## Package Contents

This package wraps the `codex-exec-mcp-server` binary and its helpers so that the shell MCP can be invoked via `npx -y @openai/codex-shell-tool-mcp`. It bundles:

- `codex-exec-mcp-server` and `codex-execve-wrapper` built for macOS (arm64, x64) and Linux (musl arm64, musl x64).
- A patched Bash that honors `BASH_EXEC_WRAPPER`, built for multiple glibc baselines (Ubuntu 24.04/22.04/20.04, Debian 12/11, CentOS-like 9) and macOS (15/14/13).
- A launcher (`bin/mcp-server.js`) that picks the correct binaries for the current `process.platform` / `process.arch`, specifying `--execve` and `--bash` for the MCP, as appropriate.

See [the README in the Codex repo](https://github.com/openai/codex/blob/main/codex-rs/exec-server/README.md) for details.
