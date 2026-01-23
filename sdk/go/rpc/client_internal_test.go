package rpc

import (
	"context"
	"errors"
	"io"
	"log/slog"
	"strings"
	"testing"

	"github.com/openai/codex/sdk/go/protocol"
)

func TestClientInternals(t *testing.T) {
	transport := &captureTransport{}
	client := &Client{
		transport: transport,
		logger:    slog.New(slog.NewTextHandler(io.Discard, nil)),
		pending:   make(map[string]chan response),
		subs:      make(map[int]chan Notification),
		done:      make(chan struct{}),
	}

	handler := &recordingHandler{}
	client.SetRequestHandler(handler)
	if client.currentHandler() != handler {
		t.Fatalf("expected handler to be set")
	}

	id := NewIntRequestID(1)
	ch := make(chan response, 1)
	client.pending[id.Key()] = ch
	client.deletePending(id)
	if _, ok := client.pending[id.Key()]; ok {
		t.Fatalf("expected pending to be deleted")
	}

	client.pending[id.Key()] = ch
	client.errResult(id, errors.New("boom"))
	if resp := <-ch; resp.err == nil {
		t.Fatalf("expected error in response")
	}

	if err := client.replyError(NewIntRequestID(2), -1, "oops", nil); err != nil {
		t.Fatalf("replyError error: %v", err)
	}
	if !strings.Contains(transport.last, "\"error\"") {
		t.Fatalf("expected error response, got %q", transport.last)
	}

	if err := client.replyResult(NewIntRequestID(3), map[string]any{"bad": func() {}}); err == nil {
		t.Fatalf("expected replyResult error")
	}
	if err := client.send(map[string]any{"bad": func() {}}); err == nil {
		t.Fatalf("expected send error")
	}

	close(client.done)
	if err := client.ensureOpen(); err == nil {
		t.Fatalf("expected ensureOpen error when closed")
	}
	if err := client.errOrClosed(); err == nil {
		t.Fatalf("expected errOrClosed to return error")
	}
	client.err = errors.New("boom")
	if err := client.errOrClosed(); err == nil || err.Error() != "boom" {
		t.Fatalf("expected boom error")
	}
}

func TestHandleServerRequestErrors(t *testing.T) {
	transport := &captureTransport{}
	client := &Client{
		transport: transport,
		logger:    slog.New(slog.NewTextHandler(io.Discard, nil)),
		pending:   make(map[string]chan response),
		subs:      make(map[int]chan Notification),
		done:      make(chan struct{}),
	}

	req := JSONRPCRequest{ID: NewIntRequestID(1), Method: "applyPatchApproval"}
	client.handleServerRequest(req)
	if !strings.Contains(transport.last, "\"error\"") {
		t.Fatalf("expected error response without handler")
	}

	client.handler = &errorHandler{}
	client.handleServerRequest(req)
	if !strings.Contains(transport.last, "\"error\"") {
		t.Fatalf("expected error response for handler error")
	}
}

type errorHandler struct{}

func (h *errorHandler) ApplyPatchApproval(ctx context.Context, params protocol.ApplyPatchApprovalParams) (*protocol.ApplyPatchApprovalResponse, error) {
	return nil, errors.New("nope")
}

func (h *errorHandler) ExecCommandApproval(ctx context.Context, params protocol.ExecCommandApprovalParams) (*protocol.ExecCommandApprovalResponse, error) {
	return nil, errors.New("nope")
}

func (h *errorHandler) ItemCommandExecutionRequestApproval(ctx context.Context, params protocol.CommandExecutionRequestApprovalParams) (*protocol.CommandExecutionRequestApprovalResponse, error) {
	return nil, errors.New("nope")
}

func (h *errorHandler) ItemFileChangeRequestApproval(ctx context.Context, params protocol.FileChangeRequestApprovalParams) (*protocol.FileChangeRequestApprovalResponse, error) {
	return nil, errors.New("nope")
}

func (h *errorHandler) ItemToolRequestUserInput(ctx context.Context, params protocol.ToolRequestUserInputParams) (*protocol.ToolRequestUserInputResponse, error) {
	return nil, errors.New("nope")
}

type captureTransport struct {
	last string
}

func (t *captureTransport) ReadLine() (string, error) {
	return "", io.EOF
}

func (t *captureTransport) WriteLine(line string) error {
	t.last = line
	return nil
}

func (t *captureTransport) Close() error {
	return nil
}
