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
