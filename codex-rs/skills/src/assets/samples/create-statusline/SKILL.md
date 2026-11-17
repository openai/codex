---
name: create-statusline
description: Build and enable a custom MCP statusline for the Codex CLI. Use when someone wants a provider that displays information in the terminal footer.
---

Help the user choose what to show. Reuse an existing stdio MCP server when it already owns the data and is configured globally or supplied by an installed plugin. For a new provider, copy [the Python starter](assets/statusline.py) and edit `render`. The [TypeScript reference](references/statusline.d.ts) bundled with this skill describes the snapshot and UI tree. If those files are on a different machine from your workspace, write the provider in the workspace using the protocol below.

### Advertise and render the footer

Advertise exactly one valid statusline entrypoint per tool in `tools/list`. Use the stable server and tool names for the saved selection. Set `_meta["openai/ui"].entrypoints` to include `{"type":"statusline","resourceUri":"ui://my-statusline/footer"}`. The tool's title is the label in the picker.

Codex requests the advertised URI with `resources/read`. Read the displayed thread's snapshot from `params._meta["com.openai/codex-statusline-state"]`. Return exactly one text resource at that URI, with MIME type `application/json` and a JSON-encoded UI tree. Optional account and usage fields may be absent. A string renders literal text and `null` clears the footer.

Use a `row` with an explicit whole-number `gap` to group items. Use named theme or status colors to follow the user's theme. Codex accepts up to 64 KiB of UI tree JSON, 16 KiB of UTF-8 leaf text, 256 non-null nodes, and a depth of 16 (root at 0). Each read has one second to finish. After a successful read, Codex waits one second before reading again. An error or timeout stops polling. Reselect the provider in `/statusline` to restart it.

### Enable and verify

Install the provider through an installed plugin or register the stdio command on the connected app-server host. For example, run `codex mcp add my-statusline -- python3 -u /absolute/path/to/statusline.py` there, using a path and interpreter the host can access. If the workspace is on a separate executor, give the user the provider file to copy to the host and the command to run there. Set `tui_extensions_statusline = true` under `[features]` in the TUI client's configuration and restart the TUI if you changed the flag.

Exercise `tools/list` and `resources/read` with a representative snapshot. After adding the server, start a new conversation in its workspace to connect to it. Open `/statusline`, choose **Choose a statusline plugin**, and select the provider. Verify that changing threads changes the displayed inputs. F2 shows the failed attempt if the footer falls back. After editing the Python file, restart its stdio process to load the changes.
