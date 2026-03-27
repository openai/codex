# codex-exec-server

`codex-exec-server` is a small standalone JSON-RPC server for spawning
and controlling subprocesses through `codex-utils-pty`.

This PR intentionally lands only the standalone binary, client, wire protocol,
and docs. Exec and filesystem methods are stubbed server-side here and are
implemented in follow-up PRs.

It currently provides:

- a standalone binary: `codex-exec-server`
- a Rust client: `ExecServerClient`
- a small protocol module with shared request/response types

This crate is intentionally narrow. It is not wired into the main Codex CLI or
unified-exec in this PR; it is only the standalone transport layer.

## Transport

The server speaks the shared `codex-app-server-protocol` message envelope on
the wire.

The standalone binary supports:

- `ws://IP:PORT` (default)

Wire framing:

- websocket: one JSON-RPC message per websocket text frame

## Lifecycle

Each connection follows this sequence:

1. Send `initialize`.
2. Wait for the `initialize` response.
3. Send `initialized`.
4. Call exec or filesystem RPCs once the follow-up implementation PRs land.

If the server receives any notification other than `initialized`, it replies
with an error using request id `-1`.

If the websocket connection closes, the server terminates any remaining managed
processes for that client connection.

## API

### `initialize`

Initial handshake request.

Request params:

```json
{
  "clientName": "my-client"
}
```

Response:

```json
{
  "capabilities": {
    "direct": true,
    "zshFork": false
  }
}
```

### `initialized`

Handshake acknowledgement notification sent by the client after a successful
`initialize` response.

Params are currently ignored. Sending any other notification method is treated
as an invalid request.

### `command/exec`

Starts a new managed process.

Request params:

```json
{
  "processId": "proc-1",
  "argv": ["bash", "-lc", "printf 'hello\\n'"],
  "cwd": "/absolute/working/directory",
  "env": {
    "PATH": "/usr/bin:/bin"
  },
  "tty": true,
  "outputBytesCap": 16384,
  "arg0": null
}
```

Field definitions:

- `processId`: caller-chosen stable id for this process within the connection.
- `argv`: command vector. It must be non-empty.
- `cwd`: absolute working directory used for the child process.
- `env`: environment variables passed to the child process.
- `tty`: when `true`, spawn a PTY-backed interactive process; when `false`,
  spawn a pipe-backed process with closed stdin.
- `outputBytesCap`: maximum retained stdout/stderr bytes per stream for the
  in-memory buffer. Defaults to `codex_utils_pty::DEFAULT_OUTPUT_BYTES_CAP`.
- `arg0`: optional argv0 override forwarded to `codex-utils-pty`.

Response:

```json
{
  "processId": "proc-1",
  "running": true,
  "exitCode": null,
  "stdout": null,
  "stderr": null
}
```

Behavior notes:

- Reusing an existing `processId` is rejected.
- PTY-backed processes accept later writes through `command/exec/write`.
- Pipe-backed processes are launched with stdin closed and reject writes.
- Output is streamed asynchronously via `command/exec/outputDelta`.
- Exit is reported asynchronously via `command/exec/exited`.

### `command/exec/write`

Writes raw bytes to a running PTY-backed process stdin.

Request params:

```json
{
  "processId": "proc-1",
  "chunk": "aGVsbG8K"
}
```

`chunk` is base64-encoded raw bytes. In the example above it is `hello\n`.

Response:

```json
{
  "accepted": true
}
```

Behavior notes:

- Writes to an unknown `processId` are rejected.
- Writes to a non-PTY process are rejected because stdin is already closed.

### `command/exec/terminate`

Terminates a running managed process.

Request params:

```json
{
  "processId": "proc-1"
}
```

Response:

```json
{
  "running": true
}
```

If the process is already unknown or already removed, the server responds with:

```json
{
  "running": false
}
```

## Notifications

### `command/exec/outputDelta`

Streaming output chunk from a running process.

Params:

```json
{
  "processId": "proc-1",
  "stream": "stdout",
  "chunk": "aGVsbG8K"
}
```

Fields:

- `processId`: process identifier
- `stream`: `"stdout"` or `"stderr"`
- `chunk`: base64-encoded output bytes

### `command/exec/exited`

Final process exit notification.

Params:

```json
{
  "processId": "proc-1",
  "exitCode": 0
}
```

## Errors

The server returns JSON-RPC errors with these codes:

- `-32600`: invalid request
- `-32602`: invalid params
- `-32603`: internal error

Typical error cases:

- unknown method
- malformed params
- empty `argv`
- duplicate `processId`
- writes to unknown processes
- writes to non-PTY processes

## Rust surface

The crate exports:

- `ExecServerClient`
- `ExecServerError`
- `ExecServerClientConnectOptions`
- `RemoteExecServerConnectArgs`
- protocol structs `InitializeParams` and `InitializeResponse`
- `DEFAULT_LISTEN_URL` and `ExecServerListenUrlParseError`
- `run_main_with_listen_url()`
- `run_main()` for embedding the websocket server in a binary

## Example session

Initialize:

```json
{"id":1,"method":"initialize","params":{"clientName":"example-client"}}
{"id":1,"result":{}}
{"method":"initialized","params":{}}
```

Start a process:

```json
{"id":2,"method":"command/exec","params":{"processId":"proc-1","argv":["bash","-lc","printf 'ready\\n'; while IFS= read -r line; do printf 'echo:%s\\n' \"$line\"; done"],"cwd":"/tmp","env":{"PATH":"/usr/bin:/bin"},"tty":true,"outputBytesCap":4096,"arg0":null}}
{"id":2,"result":{"processId":"proc-1","running":true,"exitCode":null,"stdout":null,"stderr":null}}
{"method":"command/exec/outputDelta","params":{"processId":"proc-1","stream":"stdout","chunk":"cmVhZHkK"}}
```

Write to the process:

```json
{"id":3,"method":"command/exec/write","params":{"processId":"proc-1","chunk":"aGVsbG8K"}}
{"id":3,"result":{"accepted":true}}
{"method":"command/exec/outputDelta","params":{"processId":"proc-1","stream":"stdout","chunk":"ZWNobzpoZWxsbwo="}}
```

Terminate it:

```json
{"id":4,"method":"command/exec/terminate","params":{"processId":"proc-1"}}
{"id":4,"result":{"running":true}}
{"method":"command/exec/exited","params":{"processId":"proc-1","exitCode":0}}
```
