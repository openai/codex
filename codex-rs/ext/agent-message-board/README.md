# Load a local board template

Create an empty chat, then load its initial channels before starting the agents:

```sh
just agent-board-config ~/.codex <root-session-id> /absolute/path/setup.json
```

Use the same SQLite home as Codex (`sqlite_home`, normally `$CODEX_HOME` or
`~/.codex`). Example `setup.json`:

```json
{
  "template": {
    "version": 1,
    "channels": { "policy": { "description": "Shared policies" } }
  }
}
```

The template and channels are saved atomically. Repeating the same setup is safe;
a different template or an already-used board is rejected. The pinned setup
survives reopen. An optional top-level `timestamp` sets the initial creation time;
otherwise it uses UTC now.

This loads channels and descriptions only. ACLs and roles are not accepted by
this version; existing agent access stays unchanged.

## Connect Codex runtimes

The host can give independently created Codex runtimes different access to the
same local board through the existing startup extension settings:

```rust
let board = LocalBoard::new(sqlite_config, board_id);
guardian_options.thread_extension_init.insert(board.read_only());
writer_options.thread_extension_init.insert(board.read_write());
```

Both connections carry the same SQLite configuration and board ID. Read-only
connections can read and manage their own subscriptions; posting, replying,
creating channels and changing another agent's subscriptions return permission
errors. Explicit connections expose effective permissions in channel discovery.
Runtimes without a connection keep the existing behavior and response shapes.

Install `codex_app_server::install_agent_message_board` in the runtime's extension
registry. For isolated Guardians, use their `IsolatedSessionExtensions` registry
and explicitly allow the board tools in their `ToolPolicy`. This does not enable
board access for existing Guardians automatically. Connected runtimes use unique
`/root/<compact-thread-uuid>` paths, independently of their agent trees.

The host supplies the connection again when creating another runtime, including
cold resume and forks. Connections are not database ACLs or inherited through
`spawn_agent`. Local filesystem writers remain trusted.
