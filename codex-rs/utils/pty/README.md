# codex-utils-pty parity notes

The tests in `utils/pty/src/lib.rs` compare PTY-backed sessions with the piped
fallback to ensure the shared behaviors stay aligned. Some behavior differences
are inherent to PTY semantics, so the tests normalize or explicitly allow them.

Known differences

- TTY line discipline: PTY-backed children run with a terminal line discipline
  (canonical input, echo, signal generation). Piped fallback uses plain pipes,
  so input is raw and no echo or line editing occurs.
- Line endings: PTY output may translate LF to CRLF (for example when `ONLCR` is
  enabled). Piped output preserves what the program writes.
- Stdout/stderr interleaving: PTY output is a single stream with ordering
  preserved by the terminal. Piped fallback merges stdout/stderr from separate
  readers, so relative ordering between the streams is not guaranteed.
- Terminal features: PTY sessions support terminal-only behaviors (window size,
  control sequences). Piped fallback does not emulate these features.
- Signals: PTY sessions can receive terminal-generated signals (such as Ctrl+C).
  Piped fallback does not provide that terminal-level signaling.
