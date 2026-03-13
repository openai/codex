# codex-utils-pty

Lightweight helpers for spawning interactive processes either under a PTY (pseudo terminal) or regular pipes. The public API is minimal and mirrors both backends so callers can switch based on their needs (e.g., enabling or disabling TTY).

## API surface

- `spawn_pty_process(program, args, cwd, env, arg0, size)` → `SpawnedProcess`
- `spawn_pipe_process(program, args, cwd, env, arg0)` → `SpawnedProcess`
- `spawn_pipe_process_no_stdin(program, args, cwd, env, arg0)` → `SpawnedProcess`
- `spawn_pipe_process_split(program, args, cwd, env, arg0)` → `SpawnedProcessSplit`
- `conpty_supported()` → `bool` (Windows only; always true elsewhere)
- `TerminalSize { rows, cols }` selects PTY dimensions in character cells.
- `ProcessHandle` exposes:
  - `writer_sender()` → `mpsc::Sender<Vec<u8>>` (stdin)
  - `resize(TerminalSize)`
  - `close_stdin()`
  - `has_exited()`, `exit_code()`, `terminate()`
- `SpawnedProcess` bundles `session`, `output_rx`, and `exit_rx` (oneshot exit code).
- `SpawnedProcessSplit` bundles `session`, merged `output_rx`, split `stdout_rx` / `stderr_rx`, and `exit_rx`.

## Usage examples

```rust
use std::collections::HashMap;
use std::path::Path;
use codex_utils_pty::spawn_pty_process;
use codex_utils_pty::TerminalSize;

# tokio_test::block_on(async {
let env_map: HashMap<String, String> = std::env::vars().collect();
let spawned = spawn_pty_process(
    "bash",
    &["-lc".into(), "echo hello".into()],
    Path::new("."),
    &env_map,
    &None,
    TerminalSize::default(),
).await?;

let writer = spawned.session.writer_sender();
writer.send(b"exit\n".to_vec()).await?;

// Collect output until the process exits.
let mut output_rx = spawned.output_rx;
let mut collected = Vec::new();
while let Ok(chunk) = output_rx.try_recv() {
    collected.extend_from_slice(&chunk);
}
let exit_code = spawned.exit_rx.await.unwrap_or(-1);
# let _ = (collected, exit_code);
# anyhow::Ok(())
# });
```

Swap in `spawn_pipe_process` for a non-TTY subprocess; the rest of the API stays the same.
Use `spawn_pipe_process_no_stdin` to force stdin closed (commands that read stdin will see EOF immediately).

## Tests

Unit tests live in `src/lib.rs` and cover both backends (PTY Python REPL and pipe-based stdin roundtrip). Run with:

```
cargo test -p codex-utils-pty -- --nocapture
```
