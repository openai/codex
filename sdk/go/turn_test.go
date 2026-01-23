package codex

import (
	"context"
	"encoding/json"
	"testing"

	"github.com/openai/codex/sdk/go/protocol"
	"github.com/openai/codex/sdk/go/rpc"
)

func TestThreadRunWithReplay(t *testing.T) {
	ctx := context.Background()
	info := protocol.ClientInfo{
		Name:    "codex-go-test",
		Title:   stringPtr("Codex Go SDK Test"),
		Version: "test",
	}

	client, err := New(ctx, Options{
		Transport:  rpc.NewReplayTransport(runTranscript(info, "hello", "final")),
		ClientInfo: info,
	})
	if err != nil {
		t.Fatalf("new client error: %v", err)
	}
	defer client.Close()

	thread, err := client.StartThread(ctx, ThreadStartOptions{})
	if err != nil {
		t.Fatalf("start thread error: %v", err)
	}

	result, err := thread.Run(ctx, "hello", nil)
	if err != nil {
		t.Fatalf("run error: %v", err)
	}
	if result.FinalResponse != "final" {
		t.Fatalf("unexpected final response: %s", result.FinalResponse)
	}
}

func TestResumeThreadWithReplay(t *testing.T) {
	ctx := context.Background()
	info := protocol.ClientInfo{
		Name:    "codex-go-test",
		Title:   stringPtr("Codex Go SDK Test"),
		Version: "test",
	}

	client, err := New(ctx, Options{
		Transport:  rpc.NewReplayTransport(resumeTranscript(info)),
		ClientInfo: info,
	})
	if err != nil {
		t.Fatalf("new client error: %v", err)
	}
	defer client.Close()

	thread, err := client.ResumeThread(ctx, ThreadResumeOptions{ThreadID: "thr_123"})
	if err != nil {
		t.Fatalf("resume thread error: %v", err)
	}
	if thread.ID() != "thr_123" {
		t.Fatalf("unexpected thread id: %s", thread.ID())
	}
}

func TestCloseNilClient(t *testing.T) {
	c := &Codex{}
	if err := c.Close(); err == nil {
		t.Fatalf("expected error for nil client")
	}
}

func runTranscript(info protocol.ClientInfo, prompt, finalResponse string) []rpc.TranscriptEntry {
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
			ID:     rpc.NewIntRequestID(2),
			Result: mustRaw(map[string]any{"thread": map[string]any{"id": "thr_123"}}),
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

func resumeTranscript(info protocol.ClientInfo) []rpc.TranscriptEntry {
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
			Method: "thread/resume",
			Params: mustRaw(map[string]any{"threadId": "thr_123"}),
		}),
		readLine(rpc.JSONRPCResponse{
			ID:     rpc.NewIntRequestID(2),
			Result: mustRaw(map[string]any{"thread": map[string]any{"id": "thr_123"}}),
		}),
	}
}

func turnStartParams(prompt string) map[string]any {
	return map[string]any{
		"threadId": "thr_123",
		"input":    []Input{TextInput(prompt)},
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
