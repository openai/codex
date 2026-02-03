# Craft Agents Codex Fork

> **This is a fork of [openai/codex](https://github.com/openai/codex) maintained by [Craft Docs](https://www.craft.do) for [Craft Agents](https://github.com/lukilabs/craft-agents-oss) integration.**

## Why This Fork Exists

Craft Agents is a multi-backend AI assistant that supports both Anthropic Claude and OpenAI Codex. To provide consistent permission handling across backends, we needed hooks that don't exist in upstream Codex.

## Fork Additions

| Hook | Purpose |
|------|---------|
| `item/toolCall/preExecute` | **Pre-tool approval** - Intercept ALL tool calls before execution, allowing the host app to approve/block/modify |

This enables:
- **Unified permission system** - Same permission behavior as Claude Agent SDK's `PreToolUse` hook
- **Explore mode** - Block write operations while allowing reads
- **Source management** - Block tools from inactive MCP sources with auto-activation support
- **Path expansion** - Modify tool inputs (e.g., expand `~` to home directory)

## Building from Source

```bash
# Clone this repo
git clone https://github.com/lukilabs/craft-agents-codex.git
cd craft-agents-codex

# Build (requires Rust)
cargo build --release

# Binary location
./target/release/codex
```

## Usage with Craft Agents

```bash
# Set CODEX_PATH to use this fork
export CODEX_PATH=/path/to/craft-agents-codex/target/release/codex
```

Or configure `codexPath` in your LLM connection settings.

## Staying Up-to-Date

We regularly sync with upstream `openai/codex` to incorporate new features and fixes.

```bash
# Sync from upstream (maintainers)
git fetch upstream
git merge upstream/main
```

## Upstream Documentation

For general Codex CLI usage, see the upstream repo:
- [openai/codex](https://github.com/openai/codex)
- [Codex Documentation](https://developers.openai.com/codex)

## License

[Apache-2.0 License](LICENSE)
