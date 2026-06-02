# ThreadManager Sample

Small binary that starts a Codex thread with `ThreadManager` from
`codex-core-api`, submits a user turn, and prints mapped notifications as
newline-delimited JSON.

```sh
cargo run -p codex-thread-manager-sample -- "Say hello"
```

Use `--model` to override the configured default model:

```sh
cargo run -p codex-thread-manager-sample -- --model gpt-5.2 "Say hello"
```

The prompt can also be piped through stdin:

```sh
printf 'Say hello\n' | cargo run -p codex-thread-manager-sample
```

To install the demo idle extension, pass `--keep-going-on-idle`. The extension
listens for the thread idle lifecycle event and starts another turn with a
`keep going` user message each time the thread becomes idle. The sample keeps
running until interrupted or until the thread reports an error:

```sh
cargo run -p codex-thread-manager-sample -- --keep-going-on-idle "Say hello"
```
