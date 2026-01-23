package codex

import (
	"context"
	"io"
	"log/slog"
	"reflect"
	"testing"

	"github.com/openai/codex/sdk/go/protocol"
	"github.com/openai/codex/sdk/go/rpc"
)

func TestThreadStartOptionsToParams(t *testing.T) {
	opts := ThreadStartOptions{
		Model:                 "gpt-test",
		Cwd:                   "/tmp/project",
		ApprovalPolicy:        "never",
		SandboxPolicy:         map[string]any{"type": "readOnly"},
		Config:                map[string]any{"foo": "bar"},
		BaseInstructions:      "base",
		DeveloperInstructions: "dev",
		ExperimentalRawEvents: true,
	}

	params := opts.toParams()

	assertEqual(t, "model", params["model"], "gpt-test")
	assertEqual(t, "cwd", params["cwd"], "/tmp/project")
	assertEqual(t, "approvalPolicy", params["approvalPolicy"], "never")
	assertEqual(t, "sandbox", params["sandbox"], map[string]any{"type": "readOnly"})
	assertEqual(t, "config", params["config"], map[string]any{"foo": "bar"})
	assertEqual(t, "baseInstructions", params["baseInstructions"], "base")
	assertEqual(t, "developerInstructions", params["developerInstructions"], "dev")
	assertEqual(t, "experimentalRawEvents", params["experimentalRawEvents"], true)
}

func TestThreadResumeOptionsToParams(t *testing.T) {
	opts := ThreadResumeOptions{
		ThreadID:              "thr_123",
		History:               []any{"h1"},
		Path:                  "/tmp/rollout",
		Model:                 "gpt-test",
		ModelProvider:         "openai",
		Cwd:                   "/tmp/project",
		ApprovalPolicy:        "never",
		Sandbox:               map[string]any{"type": "readOnly"},
		Config:                map[string]any{"foo": "bar"},
		BaseInstructions:      "base",
		DeveloperInstructions: "dev",
	}

	params := opts.toParams()

	assertEqual(t, "threadId", params["threadId"], "thr_123")
	assertEqual(t, "history", params["history"], []any{"h1"})
	assertEqual(t, "path", params["path"], "/tmp/rollout")
	assertEqual(t, "model", params["model"], "gpt-test")
	assertEqual(t, "modelProvider", params["modelProvider"], "openai")
	assertEqual(t, "cwd", params["cwd"], "/tmp/project")
	assertEqual(t, "approvalPolicy", params["approvalPolicy"], "never")
	assertEqual(t, "sandbox", params["sandbox"], map[string]any{"type": "readOnly"})
	assertEqual(t, "config", params["config"], map[string]any{"foo": "bar"})
	assertEqual(t, "baseInstructions", params["baseInstructions"], "base")
	assertEqual(t, "developerInstructions", params["developerInstructions"], "dev")
}

func TestBuildTurnParams(t *testing.T) {
	opts := &TurnOptions{
		Cwd:               "/tmp",
		ApprovalPolicy:    "never",
		SandboxPolicy:     map[string]any{"type": "readOnly"},
		Model:             "gpt-test",
		Effort:            "medium",
		Summary:           "short",
		OutputSchema:      map[string]any{"type": "object"},
		CollaborationMode: "default",
	}

	params := buildTurnParams("thr_123", []Input{TextInput("hello")}, opts)

	assertEqual(t, "threadId", params["threadId"], "thr_123")
	assertEqual(t, "input", params["input"], []Input{TextInput("hello")})
	assertEqual(t, "cwd", params["cwd"], "/tmp")
	assertEqual(t, "approvalPolicy", params["approvalPolicy"], "never")
	assertEqual(t, "sandboxPolicy", params["sandboxPolicy"], map[string]any{"type": "readOnly"})
	assertEqual(t, "model", params["model"], "gpt-test")
	assertEqual(t, "effort", params["effort"], "medium")
	assertEqual(t, "summary", params["summary"], "short")
	assertEqual(t, "outputSchema", params["outputSchema"], map[string]any{"type": "object"})
	assertEqual(t, "collaborationMode", params["collaborationMode"], "default")
}

func TestExtractThreadIDFromResponse(t *testing.T) {
	id, err := extractThreadIDFromResponse(map[string]any{"thread": map[string]any{"id": "thr_1"}})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if id != "thr_1" {
		t.Fatalf("expected thread id thr_1, got %q", id)
	}

	if _, err := extractThreadIDFromResponse(map[string]any{}); err == nil {
		t.Fatalf("expected error for missing thread id")
	}
}

func TestExtractThreadIDHelpers(t *testing.T) {
	note := map[string]any{"threadId": "thr_1"}
	if id := extractThreadID(note); id != "thr_1" {
		t.Fatalf("expected thread id thr_1, got %q", id)
	}

	note = map[string]any{"thread": map[string]any{"id": "thr_2"}}
	if id := extractThreadID(note); id != "thr_2" {
		t.Fatalf("expected thread id thr_2, got %q", id)
	}
}

func TestExtractTurnID(t *testing.T) {
	note := map[string]any{"turn": map[string]any{"id": "turn_1"}}
	if id := extractTurnID(note); id != "turn_1" {
		t.Fatalf("expected turn id turn_1, got %q", id)
	}
}

func TestExtractText(t *testing.T) {
	if text, ok := extractText(map[string]any{"text": "hello"}); !ok || text != "hello" {
		t.Fatalf("expected text from map")
	}

	if text, ok := extractText(map[string]any{"wrapped": map[string]any{"text": "inner"}}); !ok || text != "inner" {
		t.Fatalf("expected text from nested map")
	}

	type message struct {
		Text string
	}
	if text, ok := extractText(message{Text: "struct"}); !ok || text != "struct" {
		t.Fatalf("expected text from struct")
	}
}

func TestNotificationError(t *testing.T) {
	note := rpc.Notification{Method: "error", Params: map[string]any{"willRetry": true}}
	if err := notificationError(note); err != nil {
		t.Fatalf("expected nil error for willRetry")
	}

	note = rpc.Notification{Method: "error", Params: map[string]any{"error": map[string]any{"message": "boom"}}}
	if err := notificationError(note); err == nil || err.Error() != "boom" {
		t.Fatalf("expected error boom, got %v", err)
	}

	note = rpc.Notification{Method: "turn/completed", Params: map[string]any{"turn": map[string]any{"status": "failed", "error": map[string]any{"message": "fail"}}}}
	if err := notificationError(note); err == nil || err.Error() != "fail" {
		t.Fatalf("expected error fail, got %v", err)
	}
}

func TestResolveLogger(t *testing.T) {
	logger := resolveLogger(nil)
	if logger == nil {
		t.Fatalf("expected non-nil logger")
	}
	logger.Info("silenced")
}

func TestAttachApprovalLogger(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	handler := AutoApproveHandler{}
	attached := attachApprovalLogger(handler, logger)
	typed, ok := attached.(AutoApproveHandler)
	if !ok {
		t.Fatalf("expected AutoApproveHandler")
	}
	if typed.Logger == nil {
		t.Fatalf("expected logger to be attached")
	}
}

func TestAutoApproveResponses(t *testing.T) {
	handler := AutoApproveHandler{}
	resp, err := handler.ItemCommandExecutionRequestApproval(context.Background(), protocol.CommandExecutionRequestApprovalParams{ItemID: "item", ThreadID: "thr", TurnID: "turn"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resp == nil {
		t.Fatalf("expected response")
	}
}

func TestAutoApproveLegacyResponses(t *testing.T) {
	handler := AutoApproveHandler{}
	if _, err := handler.ItemFileChangeRequestApproval(context.Background(), protocol.FileChangeRequestApprovalParams{ItemID: "item", ThreadID: "thr", TurnID: "turn"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if _, err := handler.ApplyPatchApproval(context.Background(), protocol.ApplyPatchApprovalParams{CallID: "call", ConversationID: "thr", FileChanges: map[string]any{}}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if _, err := handler.ExecCommandApproval(context.Background(), protocol.ExecCommandApprovalParams{CallID: "call", ConversationID: "thr", Command: []string{"echo"}}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if _, err := handler.ItemToolRequestUserInput(context.Background(), protocol.ToolRequestUserInputParams{ItemID: "item", ThreadID: "thr", TurnID: "turn"}); err == nil {
		t.Fatalf("expected error for tool user input")
	}
}

func TestNewUsesDefaultClientInfo(t *testing.T) {
	ctx := context.Background()
	client, err := New(ctx, Options{
		Transport: rpc.NewReplayTransport(initializeTranscript()),
	})
	if err != nil {
		t.Fatalf("new client error: %v", err)
	}
	if client.Client() == nil {
		t.Fatalf("expected rpc client")
	}
	_ = client.Close()
}

func TestNewSpawnError(t *testing.T) {
	ctx := context.Background()
	_, err := New(ctx, Options{
		Spawn: SpawnOptions{CodexPath: "codex-missing-binary"},
	})
	if err == nil {
		t.Fatalf("expected spawn error")
	}
}

func initializeTranscript() []rpc.TranscriptEntry {
	info := defaultClientInfo()
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
	}
}

func TestInputHelpers(t *testing.T) {
	if input := TextInput("hi"); input.Type != "text" || input.Text != "hi" {
		t.Fatalf("unexpected text input: %#v", input)
	}
	if input := ImageInput("https://example.com"); input.Type != "image" || input.URL != "https://example.com" {
		t.Fatalf("unexpected image input: %#v", input)
	}
	if input := LocalImageInput("/tmp/img.png"); input.Type != "localImage" || input.Path != "/tmp/img.png" {
		t.Fatalf("unexpected local image input: %#v", input)
	}
	if input := SkillInput("skill", "/tmp/skill"); input.Type != "skill" || input.Name != "skill" || input.Path != "/tmp/skill" {
		t.Fatalf("unexpected skill input: %#v", input)
	}
}

func TestMatchThreadID(t *testing.T) {
	note := rpc.Notification{Params: map[string]any{"threadId": "thr_1"}}
	if !matchesThreadID(note, "thr_1") {
		t.Fatalf("expected matching thread id")
	}
	if matchesThreadID(note, "thr_2") {
		t.Fatalf("expected non-matching thread id")
	}

	empty := rpc.Notification{Params: map[string]any{}}
	if !matchesThreadID(empty, "thr_1") {
		t.Fatalf("expected match when thread id missing")
	}
}

func TestExtractErrorMessage(t *testing.T) {
	if msg := extractErrorMessage(map[string]any{"message": "oops"}); msg != "oops" {
		t.Fatalf("unexpected message: %s", msg)
	}
	if msg := extractErrorMessage(nil); msg != "" {
		t.Fatalf("expected empty message")
	}
}

func assertEqual(t *testing.T, name string, got, want any) {
	t.Helper()
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("unexpected %s: %#v (want %#v)", name, got, want)
	}
}
