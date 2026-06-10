# App Server File Streaming API

## Status

Implemented in app-server and exec-server.

The existing buffered `fs/readFile` and `fs/writeFile` methods remain unchanged.

## Goals

- Read and write files without buffering the entire file in memory.
- Provide backpressure through bounded request/response chunks.
- Allow positional reads.
- Support cancellation and deterministic cleanup.
- Atomically replace destination files after successful streamed writes.

## Read API

### `fs/readFile/open`

Opens a file for positional reads.

```json
{
  "handleId": "read-1",
  "path": "/absolute/path/to/file"
}
```

Response:

```json
{
  "maxChunkBytes": 262144
}
```

### `fs/readFile/read`

Reads at most `maxBytes` starting at the absolute byte offset. If `maxBytes`
is omitted or exceeds `maxChunkBytes`, the server uses `maxChunkBytes`.
Clients may use `maxChunkBytes` to size a reusable read buffer.

```json
{
  "handleId": "read-1",
  "offset": 0,
  "maxBytes": 65536
}
```

Response:

```json
{
  "dataBase64": "SGVsbG8=",
  "eof": false
}
```

`eof` means no more bytes were available at the time of the read. A later read
at the same offset may return data if the file grows.

### `fs/readFile/stat`

Returns metadata for the open file.

```json
{
  "handleId": "read-1"
}
```

Response:

```json
{
  "sizeBytes": 1234,
  "createdAtMs": 1730910000000,
  "modifiedAtMs": 1730910000000
}
```

The metadata applies to the opened file handle, so the operation remains valid
if the path is renamed or replaced after it is opened.

### `fs/readFile/close`

Closes the read handle.

```json
{
  "handleId": "read-1"
}
```

Response:

```json
{}
```

## Write API

### `fs/writeFile/open`

Creates an empty temporary file in the destination directory. Its name starts
with `.codex-tmp-` so abandoned files are attributable and hidden on platforms
where dot-prefixed files are hidden.

```json
{
  "handleId": "write-1",
  "path": "/absolute/path/to/file"
}
```

Response:

```json
{
  "maxChunkBytes": 262144
}
```

### `fs/writeFile/write`

Decodes and appends the complete chunk to the temporary file. A successful
response acknowledges the entire decoded chunk; partial success is not exposed.

```json
{
  "handleId": "write-1",
  "dataBase64": "SGVsbG8="
}
```

Response:

```json
{}
```

### `fs/writeFile/commit`

Flushes the completed file, atomically replaces the destination, and closes the
write handle.

```json
{
  "handleId": "write-1"
}
```

Response:

```json
{
  "sizeBytes": 5,
  "modifiedAtMs": 1730910000000
}
```

### `fs/writeFile/close`

Closes the write handle and deletes the uncommitted temporary file.

```json
{
  "handleId": "write-1"
}
```

Response:

```json
{}
```

## Shared Semantics

- Handle IDs are client-supplied strings scoped to one connection.
- Opening a duplicate active handle ID returns `INVALID_REQUEST`.
- Operations for one handle are serialized. Different handles may run
  concurrently.
- Reads are positional and do not maintain a server-side cursor.
- Writes are sequential appends.
- Each read or write transfers at most the `maxChunkBytes` returned by its
  corresponding open operation.
- `maxChunkBytes` is currently 262144 bytes.
- Backpressure comes from awaiting bounded read and write responses. Clients
  should use a bounded pipeline of up to two read requests to hide transport
  round-trip latency without accumulating unbounded response data.
- Close operations are idempotent. App-server close requests bypass queued
  operations and cancel an active chunk operation. Once commit starts, it runs
  to completion so the server never reports cancellation while an atomic
  replacement may still publish. Exec-server handles bounded file RPCs in
  request order, so close takes effect before the next file operation.
- Closing a connection closes all of its handles.
- Any filesystem or I/O error closes the affected handle. Protocol errors such
  as an unknown handle do not affect other handles.
- Failed or cancelled writes delete their temporary files.
- Write commit replaces the destination, matching existing `fs/writeFile`
  overwrite behavior.
- Errors use normal JSON-RPC error responses.
- App-server operations target the app-server host filesystem. Exec-server
  exposes the same pull-based handle operations for remote filesystem clients.
- Streaming through the platform sandbox helper is not supported because the
  helper is one-shot and cannot retain open-file identity across requests.
