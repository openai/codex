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

	client, err := codex.New(ctx, exampleOptions(logger))
	if err != nil {
		panic(err)
	}
	defer client.Close()

	rpcClient := client.Client()
	models, err := rpcClient.ModelList(ctx, protocol.ModelListParams{})
	if err != nil {
		panic(err)
	}

	fmt.Println(formatModels(models))
}

const exampleReplayEnv = "CODEX_EXAMPLE_REPLAY"

func exampleOptions(logger *slog.Logger) codex.Options {
	if os.Getenv(exampleReplayEnv) == "" {
		return codex.Options{Logger: logger}
	}

	info := exampleClientInfo()
	return codex.Options{
		Transport:  rpc.NewReplayTransport(exampleTranscript(info)),
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

func exampleTranscript(info protocol.ClientInfo) []rpc.TranscriptEntry {
	result := map[string]any{
		"models": []map[string]any{
			{"id": "model-1", "title": "Test Model"},
		},
	}
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
			Method: "model/list",
			Params: mustRaw(protocol.ModelListParams{}),
		}),
		readLine(rpc.JSONRPCResponse{
			ID:     rpc.NewIntRequestID(2),
			Result: mustRaw(result),
		}),
	}
}

func formatModels(models *protocol.ModelListResponse) string {
	if models == nil {
		return "models: <nil>"
	}
	data, err := json.MarshalIndent(*models, "", "  ")
	if err != nil {
		return fmt.Sprintf("models: %v", *models)
	}
	return fmt.Sprintf("models: %s", data)
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
