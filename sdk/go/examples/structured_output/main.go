package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"os"

	"github.com/openai/codex/sdk/go"
	"github.com/openai/codex/sdk/go/protocol"
	"github.com/openai/codex/sdk/go/rpc"
)

func main() {
	ctx := context.Background()
	logger := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelInfo}))
	prompt := "Summarize repo status"

	client, err := codex.New(ctx, exampleOptions(prompt, logger))
	if err != nil {
		panic(err)
	}
	defer client.Close()

	thread, err := client.StartThread(ctx, codex.ThreadStartOptions{})
	if err != nil {
		panic(err)
	}

	schema := exampleSchema()

	result, err := thread.RunInputs(ctx, []codex.Input{codex.TextInput(prompt)}, &codex.TurnOptions{
		OutputSchema: schema,
	})
	if err != nil {
		panic(err)
	}

	fmt.Println(result.FinalResponse)
}

func exampleSchema() map[string]any {
	return map[string]any{
		"type": "object",
		"properties": map[string]any{
			"summary": map[string]any{"type": "string"},
			"status": map[string]any{
				"type": "string",
				"enum": []string{"ok", "action_required"},
			},
		},
		"required":             []string{"summary", "status"},
		"additionalProperties": false,
	}
}

const exampleReplayEnv = "CODEX_EXAMPLE_REPLAY"

func exampleOptions(prompt string, logger *slog.Logger) codex.Options {
	if os.Getenv(exampleReplayEnv) == "" {
		return codex.Options{Logger: logger}
	}

	info := exampleClientInfo()
	return codex.Options{
		Transport:  rpc.NewReplayTransport(exampleTranscript(info, prompt, exampleSchema(), "Structured summary")),
		ClientInfo: info,
	}
}

func exampleClientInfo() protocol.ClientInfo {
	return protocol.ClientInfo{
		Name:    "codex-go-example",
		Title:   stringPtr("Codex Go SDK Example"),
		Version: "test",
	}
}

func exampleTranscript(info protocol.ClientInfo, prompt string, schema map[string]any, finalResponse string) []rpc.TranscriptEntry {
	return []rpc.TranscriptEntry{
		writeLine(rpc.JSONRPCRequest{
			ID:     rpc.NewIntRequestID(1),
			Method: "initialize",
			Params: mustRaw(protocol.InitializeParams{ClientInfo: info}),
		}),
		readLine(rpc.JSONRPCResponse{
			ID:     rpc.NewIntRequestID(1),
			Result: mustRaw(map[string]any{}),
		}),
		writeLine(rpc.JSONRPCNotification{Method: "initialized"}),
		writeLine(rpc.JSONRPCRequest{
			ID:     rpc.NewIntRequestID(2),
			Method: "thread/start",
			Params: mustRaw(map[string]any{}),
		}),
		readLine(rpc.JSONRPCResponse{
			ID: rpc.NewIntRequestID(2),
			Result: mustRaw(map[string]any{
				"thread": map[string]any{"id": "thr_123"},
			}),
		}),
		writeLine(rpc.JSONRPCRequest{
			ID:     rpc.NewIntRequestID(3),
			Method: "turn/start",
			Params: mustRaw(turnStartParams(prompt, schema)),
		}),
		readLine(rpc.JSONRPCResponse{
			ID:     rpc.NewIntRequestID(3),
			Result: mustRaw(map[string]any{"turn": turnPayload("turn_1", "inProgress")}),
		}),
		readLine(rpc.JSONRPCNotification{
			Method: "turn/started",
			Params: mustRaw(map[string]any{"threadId": "thr_123", "turn": turnPayload("turn_1", "inProgress")}),
		}),
		readLine(rpc.JSONRPCNotification{
			Method: "item/completed",
			Params: mustRaw(map[string]any{"threadId": "thr_123", "item": map[string]any{"text": finalResponse}}),
		}),
		readLine(rpc.JSONRPCNotification{
			Method: "turn/completed",
			Params: mustRaw(map[string]any{"threadId": "thr_123", "turn": turnPayload("turn_1", "completed")}),
		}),
	}
}

func turnStartParams(prompt string, schema map[string]any) map[string]any {
	return map[string]any{
		"threadId":     "thr_123",
		"input":        []codex.Input{codex.TextInput(prompt)},
		"outputSchema": schema,
	}
}

func turnPayload(turnID, status string) map[string]any {
	return map[string]any{
		"id":     turnID,
		"status": status,
		"items":  []any{},
		"error":  nil,
	}
}

func writeLine(payload any) rpc.TranscriptEntry {
	return rpc.TranscriptEntry{Direction: rpc.TranscriptWrite, Line: mustJSON(payload)}
}

func readLine(payload any) rpc.TranscriptEntry {
	return rpc.TranscriptEntry{Direction: rpc.TranscriptRead, Line: mustJSON(payload)}
}

func mustJSON(payload any) string {
	data, err := json.Marshal(payload)
	if err != nil {
		panic(err)
	}
	return string(data)
}

func mustRaw(payload any) json.RawMessage {
	if payload == nil {
		return nil
	}
	data, err := json.Marshal(payload)
	if err != nil {
		panic(err)
	}
	return data
}

func stringPtr(value string) *string {
	return &value
}
