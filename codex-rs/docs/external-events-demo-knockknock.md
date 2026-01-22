# External Events Demo: Knock-Knock (one prompt for both sessions)

Goal: demo “communication” by relaying lines between two Codex sessions using `codex events send`
to append `agent.message` external events into the other session’s inbox.

## Setup (shell)

```sh
cd /Users/joshka/code/codex-external-events/codex-rs
export CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
```

Start 2 sessions (two terminals):
```sh
cargo run -p codex-tui
```

Find the two new session IDs (pick the two newest UUID dirs):
```sh
ls -lt "$CODEX_HOME/sessions" | head -n 5
```

## Paste this prompt into BOTH sessions

In Session 1, set `OTHER_THREAD_ID` to Session 2’s id.
In Session 2, set `OTHER_THREAD_ID` to Session 1’s id.

```text
External-events relay demo: knock-knock.

OTHER_THREAD_ID = <PASTE_THE_OTHER_SESSION_UUID_HERE>

You can see your own session/thread id in the Codex UI. Determine your role:
- Compare YOUR_THREAD_ID vs OTHER_THREAD_ID as lowercase strings.
- If YOUR_THREAD_ID < OTHER_THREAD_ID: you are Role A (initiator). Else Role B (responder).

Rules:
- Treat external events of type agent.message as incoming chat lines from the other session. Use the event summary as the incoming text.
- Reply in the main chat with ONLY your next outbound line (no commentary, no prefixes).
- Maintain conversation state internally.

Dialogue:
- Role A: on start (immediately), say: Knock knock.
- Role A: if you receive: Who's there?  -> say: Lettuce.
- Role A: if you receive: Lettuce who?  -> say: Lettuce in, it's cold out here!

- Role B: if you receive: Knock knock. -> say: Who's there?
- Role B: if you receive: Lettuce.     -> say: Lettuce who?
- Role B: if you receive: Lettuce in, it's cold out here! -> say: Ha!

If you receive an unexpected line, reply with: What?
```

## Relay loop (shell)

Whenever one session says a line, send that exact line to the other session:
```sh
cargo run -p codex-cli -- events send \
  --thread "$TARGET_THREAD" \
  --type agent.message \
  --title "From other session" \
  --summary "$LINE" \
  --severity info \
  --codex-home "$CODEX_HOME"
```

Example:
```sh
TARGET_THREAD="<PASTE_TARGET_UUID>" LINE="Knock knock." \
  cargo run -p codex-cli -- events send --thread "$TARGET_THREAD" --type agent.message --title "From other session" --summary "$LINE" --severity info --codex-home "$CODEX_HOME"
```
