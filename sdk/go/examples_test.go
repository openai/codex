package codex_test

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/openai/codex/sdk/go"
	"github.com/openai/codex/sdk/go/protocol"
	"github.com/openai/codex/sdk/go/rpc"
)

func ExampleQuickstart() {
	ctx := context.Background()
	prompt := "Diagnose the test failure and propose a fix"
	clientInfo := protocol.ClientInfo{
		Name:    "codex-go-example",
		Title:   stringPtr("Codex Go SDK Example"),
		Version: "test",
	}

	client, err := codex.New(ctx, codex.Options{
		Transport:  rpc.NewReplayTransport(exampleTranscript(clientInfo, prompt, "Hello from replay")),
		ClientInfo: clientInfo,
	})
	if err != nil {
		panic(err)
	}
	defer client.Close()

	thread, err := client.StartThread(ctx, codex.ThreadStartOptions{})
	if err != nil {
		panic(err)
	}

	result, err := thread.Run(ctx, prompt, nil)
	if err != nil {
		panic(err)
	}

	fmt.Println(result.FinalResponse)

	// Output:
	// Hello from replay
}

func ExampleStreaming() {
	ctx := context.Background()
	prompt := "Inspect the repo"
	clientInfo := protocol.ClientInfo{
		Name:    "codex-go-example",
		Title:   stringPtr("Codex Go SDK Example"),
		Version: "test",
	}

	client, err := codex.New(ctx, codex.Options{
		Transport:  rpc.NewReplayTransport(exampleTranscript(clientInfo, prompt, "Streaming hello")),
		ClientInfo: clientInfo,
	})
	if err != nil {
		panic(err)
	}
	defer client.Close()

	thread, err := client.StartThread(ctx, codex.ThreadStartOptions{})
	if err != nil {
		panic(err)
	}

	stream, err := thread.RunStreamed(ctx, []codex.Input{codex.TextInput(prompt)}, nil)
	if err != nil {
		panic(err)
	}
	defer stream.Close()

	for {
		note, err := stream.Next(ctx)
		if err != nil {
			break
		}
		fmt.Println(note.Method)
		if note.Method == "turn/completed" {
			break
		}
	}

	// Output:
	// turn/started
	// item/completed
	// turn/completed
}

func exampleTranscript(clientInfo protocol.ClientInfo, prompt, finalResponse string) []rpc.TranscriptEntry {
	return []rpc.TranscriptEntry{
		writeLine(rpc.JSONRPCRequest{
			ID:     rpc.NewIntRequestID(1),
			Method: "initialize",
			Params: mustRaw(protocol.InitializeParams{ClientInfo: clientInfo}),
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
			Params: mustRaw(turnStartParams(prompt)),
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

func turnStartParams(prompt string) map[string]any {
	return map[string]any{
		"threadId": "thr_123",
		"input":    []codex.Input{codex.TextInput(prompt)},
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

// Quickstart exists to associate ExampleQuickstart with a named identifier.
type Quickstart struct{}

// Streaming exists to associate ExampleStreaming with a named identifier.
type Streaming struct{}
