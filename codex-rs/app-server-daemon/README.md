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

## Installation and update cases

Whether app-server becomes up to date automatically depends on both how Codex was
installed and which backend is available.

| Situation | What starts | Does this daemon fetch new binaries? | Does a running app-server eventually move to a newer binary on its own? |
| --- | --- | --- | --- |
| Codex installed with `brew` or `npm` only | `start` uses the currently running `codex` executable path | No | No. Package-manager updates are out of band, and the daemon does not restart a running app-server automatically. |
| Codex installed with `install.sh`, but only `start` is used | `start` prefers `CODEX_HOME/packages/standalone/current/codex` | No | No. The managed path is used when starting or restarting, but no updater is installed. |
| Codex installed with `install.sh`, then `bootstrap` on user-scoped `systemd` | The persistent systemd service uses `CODEX_HOME/packages/standalone/current/codex` | Yes. The generated timer runs `install.sh` hourly. | Yes, assuming the update succeeds and active work eventually drains. The timer fetches a new managed binary, then reloads app-server onto it. |
| Codex installed with `install.sh`, then `bootstrap` without user-scoped `systemd` | The pidfile backend uses `CODEX_HOME/packages/standalone/current/codex` | No | No. Bootstrap records daemon settings, but the fallback backend does not install periodic updates. |
| Some other tool updates the binary path currently used by the daemon | The next fresh start or restart uses the updated file at that path | No | Not automatically. The existing process keeps the old executable image until an explicit `restart` or, for a bootstrapped systemd service, an explicit `reload`. |

### Package-manager installs

For `brew`, `npm`, or any other install that does not place a standalone binary
at `CODEX_HOME/packages/standalone/current/codex`:

- `start`, `restart`, `stop`, and `version` work
- `bootstrap` does not work, because it requires the standalone managed install
- this daemon never invokes `brew`, `npm`, or any other package manager
- if external tooling replaces the package-manager binary on disk, a running
  app-server keeps using the old process image until it is restarted

### Standalone installs

For installs created by `install.sh`:

- lifecycle commands prefer the standalone managed binary path, even if the
  user invoked a different `codex` binary to issue the command
- `bootstrap` is supported
- on user-scoped `systemd`, bootstrap is the only flow that installs automatic
  updates; the generated timer fetches via `install.sh`, then requests a reload
- without user-scoped `systemd`, bootstrap still manages lifecycle but does not
  provide auto-update

### Out-of-band updates

This daemon does not watch arbitrary executable files for replacement. If some
other tool updates a binary that the daemon would use on its next launch:

- a currently running app-server remains on the old executable image
- `restart` will launch the updated binary
- for a bootstrapped systemd service, `reload` will also move app-server onto the
  updated binary after active work drains
- for pidfile-backed daemons, there is no periodic reload trigger

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
