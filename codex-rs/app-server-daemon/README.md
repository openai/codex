# codex-app-server-daemon

> `codex-app-server-daemon` is experimental and its lifecycle contract may
> change while the remote-management flow is still being developed.

`codex-app-server-daemon` backs the machine-readable `codex app-server`
lifecycle commands used by remote clients such as the desktop and mobile apps.
It is intended for Codex instances launched over SSH, including fresh developer
machines that should expose app-server with `remote_control` enabled.

## Platform support

The current daemon implementation is Unix-only. It uses pidfile-backed
daemonization plus Unix process and file-locking primitives, and does not yet
support Windows lifecycle management.

## Commands

```sh
codex app-server daemon start
codex app-server daemon restart
codex app-server daemon enable-remote-control
codex app-server daemon disable-remote-control
codex app-server daemon stop
codex app-server daemon version
codex app-server daemon bootstrap --remote-control
```

On success, every command writes exactly one JSON object to stdout. Consumers
should parse that JSON rather than relying on human-readable text. Lifecycle
responses report the resolved backend, socket path, local CLI version, and
running app-server version when applicable.

## Bootstrap flow

For a new remote machine:

```sh
curl -fsSL https://chatgpt.com/codex/install.sh | sh
$HOME/.codex/packages/standalone/current/codex app-server daemon bootstrap --remote-control
```

`bootstrap` requires the standalone managed install. It records the daemon
settings under `CODEX_HOME/app-server-daemon/`, starts app-server as a
pidfile-backed detached process, and launches a detached updater loop.

## Installation and update cases

The daemon assumes Codex is installed through `install.sh` and always launches
the standalone managed binary under `CODEX_HOME`.

| Situation | What starts | Does this daemon fetch new binaries? | Does a running app-server eventually move to a newer binary on its own? |
| --- | --- | --- | --- |
| `install.sh` has run, but only `start` is used | `start` uses `CODEX_HOME/packages/standalone/current/codex` | No | No. The managed path is used when starting or restarting, but no updater is installed. |
| `install.sh` has run, then `bootstrap` is used | The pidfile backend uses `CODEX_HOME/packages/standalone/current/codex` | Yes. Bootstrap launches a detached updater loop that runs `install.sh` hourly. | Yes, while that updater process is alive. After a successful fetch, it restarts a currently running app-server only when the managed binary reports a different version. |
| Some other tool updates the managed binary path | The next fresh start or restart uses the updated file at that path | No | Not automatically. The existing process keeps the old executable image until an explicit `restart`. |

### Standalone installs

For installs created by `install.sh`:

- lifecycle commands always use the standalone managed binary path
- `bootstrap` is supported
- `bootstrap` starts a detached pid-backed updater loop that fetches via
  `install.sh`, then restarts app-server if it is running on a different version
- the updater loop is not reboot-persistent; it must be started again by
  rerunning `bootstrap` after a reboot

### Out-of-band updates

This daemon does not watch arbitrary executable files for replacement. If some
other tool updates a binary that the daemon would use on its next launch:

- a currently running app-server remains on the old executable image
- `restart` will launch the updated binary
- for bootstrapped daemons, the detached updater loop only reacts to updates it
  fetched itself; it does not watch arbitrary file replacement

## Lifecycle semantics

`start` is idempotent and returns after app-server is ready to answer the normal
JSON-RPC initialize handshake on the Unix control socket.

`restart` stops any managed daemon and starts it again.

`enable-remote-control` and `disable-remote-control` persist the launch setting
for future starts. If a managed app-server is already running, they restart it
so the new setting takes effect immediately.

`stop` sends a graceful termination request first, then sends a second
termination signal after the grace window if the process is still alive.

All mutating lifecycle commands are serialized per `CODEX_HOME`, so a concurrent
`start`, `restart`, `enable-remote-control`, `disable-remote-control`, `stop`,
or `bootstrap` does not race another in-flight lifecycle operation.

## State

The daemon stores its local state under `CODEX_HOME/app-server-daemon/`:

- `settings.json` for persisted launch settings
- `app-server.pid` for the app-server process record
- `app-server-updater.pid` for the pid-backed standalone updater loop
- `daemon.lock` for daemon-wide lifecycle serialization
