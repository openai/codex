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
settings under `CODEX_HOME/app-server-daemon/`, starts app-server as a
pidfile-backed detached process, and launches a detached updater loop.

## Installation and update cases

Whether app-server becomes up to date automatically depends on how Codex was
installed and whether `bootstrap` was used.

| Situation | What starts | Does this daemon fetch new binaries? | Does a running app-server eventually move to a newer binary on its own? |
| --- | --- | --- | --- |
| Codex installed with `brew` or `npm` only | `start` uses the currently running `codex` executable path | No | No. Package-manager updates are out of band, and the daemon does not restart a running app-server automatically. |
| Codex installed with `install.sh`, but only `start` is used | `start` prefers `CODEX_HOME/packages/standalone/current/codex` | No | No. The managed path is used when starting or restarting, but no updater is installed. |
| Codex installed with `install.sh`, then `bootstrap` | The pidfile backend uses `CODEX_HOME/packages/standalone/current/codex` | Yes. Bootstrap launches a detached updater loop that runs `install.sh` hourly. | Yes, while that updater process is alive. After a successful fetch, it restarts a currently running app-server onto the managed binary. |
| Some other tool updates the binary path currently used by the daemon | The next fresh start or restart uses the updated file at that path | No | Not automatically. The existing process keeps the old executable image until an explicit `restart`. |

### Package-manager installs

For `brew`, `npm`, or any other install that does not place a standalone binary
at `CODEX_HOME/packages/standalone/current/codex`:

- `start`, `restart`, `stop`, and `version` work
- `bootstrap` does not work, because it requires the standalone managed install
- this daemon never invokes `brew`, `npm`, or any other package manager
- if external tooling replaces the package-manager binary on disk, a running
  app-server keeps using the old process image until it is restarted
- if a caller needs Codex-managed freshness on that host, it should install the
  standalone build and bootstrap that managed copy; the package-manager install
  can coexist with it

### Standalone installs

For installs created by `install.sh`:

- lifecycle commands prefer the standalone managed binary path, even if the
  user invoked a different `codex` binary to issue the command
- `bootstrap` is supported
- `bootstrap` starts a detached pid-backed updater loop that fetches via
  `install.sh`, then restarts app-server if it is running
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

`stop` sends a graceful termination request first, then sends a second
termination signal after the grace window if the process is still alive.

All mutating lifecycle commands are serialized per `CODEX_HOME`, so a concurrent
`start`, `restart`, `stop`, or `bootstrap` does not race another in-flight
lifecycle operation.

## State

The daemon stores its local state under `CODEX_HOME/app-server-daemon/`:

- `settings.json` for persisted launch settings
- `app-server.pid` for the app-server process record
- `app-server-updater.pid` for the pid-backed standalone updater loop
- `daemon.lock` for daemon-wide lifecycle serialization
