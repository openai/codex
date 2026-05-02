# codex-app-server-daemon

`codex-app-server-daemon` backs the machine-readable `codex app-server`
lifecycle commands used by remote clients such as the desktop and mobile apps.
It is intended for Codex instances launched over SSH, including fresh developer
machines that should expose app-server with `remote_control` enabled.

## Commands

```sh
codex app-server start
codex app-server restart
codex app-server stop
codex app-server version
codex app-server bootstrap --remote-control
```

Every command writes exactly one JSON object to stdout. Consumers should parse
that JSON rather than relying on human-readable text. Lifecycle responses report
the resolved backend, socket path, local CLI version, and running app-server
version when applicable.

## Bootstrap flow

For a new remote machine:

```sh
curl -fsSL https://chatgpt.com/codex/install.sh | sh
$HOME/.codex/packages/standalone/current/codex app-server bootstrap --remote-control
```

`bootstrap` requires the standalone managed install. It records the daemon
settings under `CODEX_HOME/app-server-daemon/`, starts app-server, and chooses
the best available backend:

- user-scoped `systemd`, when available
- pidfile-backed detached daemonization as the fallback

On user-scoped `systemd`, bootstrap installs home-scoped service and timer units
so multiple `CODEX_HOME` roots can coexist. The hourly timer refreshes the
standalone install and then issues `systemctl --user reload` for the app-server
service.

## Lifecycle semantics

`start` is idempotent and returns after app-server is ready to answer the normal
JSON-RPC initialize handshake on the Unix control socket.

`restart` stops any managed daemon and starts it again. With `systemd`, restart
waits up to one minute for active work to drain before the service manager may
force completion.

`reload` is only used by the generated update service. App-server treats reload
as a graceful-only drain: it waits indefinitely for active work to finish and
then restarts on the new binary.

The pidfile backend mirrors the same operator intent as closely as it can:
`stop` sends a graceful termination request first, then sends a second
termination signal after the grace window if the process is still alive.

All mutating lifecycle commands are serialized per `CODEX_HOME`, so a concurrent
`start`, `restart`, `stop`, or `bootstrap` does not race another in-flight
lifecycle operation.

## State

The daemon stores its local state under `CODEX_HOME/app-server-daemon/`:

- `settings.json` for persisted launch settings
- `app-server.pid` for the pidfile backend's process record
- `daemon.lock` for daemon-wide lifecycle serialization

When user-scoped `systemd` is available, generated units are written under the
user unit directory with names derived from `CODEX_HOME`, for example
`codex-app-server-<hash>.service`.
