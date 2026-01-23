# Codex Go SDK

Embed the Codex app-server in Go workflows.

This SDK speaks JSON-RPC to the `codex app-server` process. By default it spawns the CLI and communicates over stdio.

## Requirements

- Go 1.25+
- `codex` available on your `PATH`

## Install

```bash
go get github.com/openai/codex/sdk/go
```

## Quickstart

```go
package main

import (
    "context"
    "fmt"
    "log/slog"
    "os"

    "github.com/openai/codex/sdk/go"
)

func main() {
    ctx := context.Background()
    logger := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelInfo}))

    client, err := codex.New(ctx, codex.Options{Logger: logger})
    if err != nil {
        panic(err)
    }
    defer client.Close()

    thread, err := client.StartThread(ctx, codex.ThreadStartOptions{})
    if err != nil {
        panic(err)
    }

    result, err := thread.Run(ctx, "Diagnose the test failure and propose a fix", nil)
    if err != nil {
        panic(err)
    }

    fmt.Println(result.FinalResponse)
}
```

## Streaming

Use `RunStreamed` to receive notifications as the turn progresses.

```go
stream, err := thread.RunStreamed(ctx, []codex.Input{codex.TextInput("Inspect the repo")}, nil)
if err != nil {
    panic(err)
}

defer stream.Close()

for {
    note, err := stream.Next(ctx)
    if err != nil {
        break
    }
    fmt.Printf("%s\n", note.Method)
    if note.Method == "turn/completed" {
        break
    }
}
```

## Approvals

Configure approval handling by supplying a handler when constructing the client.

```go
logger := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelInfo}))
client, err := codex.New(ctx, codex.Options{
    Logger:          logger,
    ApprovalHandler: codex.AutoApproveHandler{Logger: logger},
})
```

For custom approval logic, implement `rpc.ServerRequestHandler` (from `sdk/go/rpc`).

## Structured Output

Provide a JSON Schema to constrain the final assistant message.

```go
schema := map[string]any{
    "type": "object",
    "properties": map[string]any{
        "summary": map[string]any{"type": "string"},
        "status": map[string]any{"type": "string", "enum": []string{"ok", "action_required"}},
    },
    "required": []string{"summary", "status"},
    "additionalProperties": false,
}

_, err := thread.RunInputs(ctx, []codex.Input{codex.TextInput("Summarize repo status")}, &codex.TurnOptions{
    OutputSchema: schema,
})
```

## Low-level RPC

Use the RPC client directly for full control.

```go
rpcClient := client.Client()
models, err := rpcClient.ModelList(ctx, protocol.ModelListParams{})
```

## Code generation

Regenerate protocol types and RPC stubs:

```bash
cd sdk/go

go generate ./...
```

This runs:

- `cargo run -p codex-app-server-protocol --bin export`
- `go-jsonschema` (via `internal/codegen`)

Generated files are checked in under `sdk/go/protocol` and `sdk/go/rpc`.
