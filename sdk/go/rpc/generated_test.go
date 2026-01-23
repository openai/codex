package rpc

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"os"
	"reflect"
	"regexp"
	"sync"
	"testing"

	"github.com/openai/codex/sdk/go/protocol"
)

func TestGeneratedClientRequests(t *testing.T) {
	transport := newScriptedTransport()
	client := NewClient(transport, ClientOptions{})
	defer client.Close()

	methods := clientRequestMethods()
	for _, name := range methods {
		method := reflect.ValueOf(client).MethodByName(name)
		if !method.IsValid() {
			t.Fatalf("missing method %s", name)
		}

		resultType := method.Type().Out(0)
		var resultValue reflect.Value
		if resultType.Kind() == reflect.Ptr {
			resultValue = reflect.New(resultType.Elem()).Elem()
		} else {
			resultValue = reflect.Zero(resultType)
		}
		transport.enqueueResult(resultValue.Interface())
		transport.enqueueError(-1, "boom")

		args := []reflect.Value{reflect.ValueOf(context.Background())}
		if method.Type().NumIn() == 2 {
			args = append(args, reflect.New(method.Type().In(1)).Elem())
		}
		out := method.Call(args)
		if !out[1].IsNil() {
			t.Fatalf("method %s returned error: %v", name, out[1].Interface())
		}

		out = method.Call(args)
		if out[1].IsNil() {
			t.Fatalf("method %s expected error", name)
		}
	}
}

func TestGeneratedNotifications(t *testing.T) {
	if len(notificationParsers) == 0 {
		t.Fatalf("expected notification methods")
	}
	for method := range notificationParsers {
		note, err := parseServerNotification(method, json.RawMessage("{}"))
		if err != nil {
			t.Fatalf("parseServerNotification %s: %v", method, err)
		}
		if note.Method != method {
			t.Fatalf("unexpected method: %s", note.Method)
		}
	}
}

func TestDispatchServerRequests(t *testing.T) {
	methods := extractCaseMethods(t, "server_requests_gen.go")
	if len(methods) == 0 {
		t.Fatalf("expected server request methods")
	}

	handler := &recordingHandler{}
	for i, method := range methods {
		req := JSONRPCRequest{ID: NewIntRequestID(int64(i + 1)), Method: method}
		if i == 0 {
			req.Params = json.RawMessage(`{}`)
		}
		if _, err := dispatchServerRequest(context.Background(), handler, req); err != nil {
			t.Fatalf("dispatch %s: %v", method, err)
		}
		if handler.lastMethod != method {
			t.Fatalf("handler not invoked for %s", method)
		}
	}
}

func clientRequestMethods() []string {
	iface := reflect.TypeOf((*ClientRequests)(nil)).Elem()
	methods := make([]string, 0, iface.NumMethod())
	for i := 0; i < iface.NumMethod(); i++ {
		methods = append(methods, iface.Method(i).Name)
	}
	return methods
}

func extractCaseMethods(t *testing.T, filename string) []string {
	t.Helper()
	data, err := os.ReadFile(filename)
	if err != nil {
		t.Fatalf("read %s: %v", filename, err)
	}
	re := regexp.MustCompile(`case "([^"]+)"`)
	matches := re.FindAllStringSubmatch(string(data), -1)
	methods := make([]string, 0, len(matches))
	for _, match := range matches {
		methods = append(methods, match[1])
	}
	return methods
}

type scriptedTransport struct {
	mu        sync.Mutex
	queue     []scriptedResponse
	responses chan string
	closed    chan struct{}
}

func newScriptedTransport() *scriptedTransport {
	return &scriptedTransport{
		responses: make(chan string, 128),
		closed:    make(chan struct{}),
	}
}

func (t *scriptedTransport) enqueueResult(result any) {
	data, err := json.Marshal(result)
	if err != nil {
		panic(err)
	}
	t.mu.Lock()
	t.queue = append(t.queue, scriptedResponse{result: data})
	t.mu.Unlock()
}

func (t *scriptedTransport) enqueueError(code int64, message string) {
	t.mu.Lock()
	t.queue = append(t.queue, scriptedResponse{err: &JSONRPCErrorError{Code: code, Message: message}})
	t.mu.Unlock()
}

func (t *scriptedTransport) ReadLine() (string, error) {
	select {
	case line := <-t.responses:
		return line, nil
	case <-t.closed:
		return "", io.EOF
	}
}

func (t *scriptedTransport) WriteLine(line string) error {
	var envelope struct {
		ID json.RawMessage `json:"id"`
	}
	if err := json.Unmarshal([]byte(line), &envelope); err != nil {
		return err
	}
	if len(envelope.ID) == 0 {
		return nil
	}
	id, err := parseRequestID(envelope.ID)
	if err != nil {
		return err
	}

	t.mu.Lock()
	if len(t.queue) == 0 {
		t.mu.Unlock()
		return errors.New("missing scripted result")
	}
	next := t.queue[0]
	t.queue = t.queue[1:]
	t.mu.Unlock()
	if next.err != nil {
		payload := JSONRPCError{ID: id, Error: *next.err}
		data, err := json.Marshal(payload)
		if err != nil {
			return err
		}
		t.responses <- string(data)
		return nil
	}
	payload := JSONRPCResponse{ID: id, Result: next.result}
	data, err := json.Marshal(payload)
	if err != nil {
		return err
	}
	t.responses <- string(data)
	return nil
}

func (t *scriptedTransport) Close() error {
	select {
	case <-t.closed:
	default:
		close(t.closed)
	}
	return nil
}

type recordingHandler struct {
	lastMethod string
}

type scriptedResponse struct {
	result json.RawMessage
	err    *JSONRPCErrorError
}

func (h *recordingHandler) ApplyPatchApproval(ctx context.Context, params protocol.ApplyPatchApprovalParams) (*protocol.ApplyPatchApprovalResponse, error) {
	h.lastMethod = "applyPatchApproval"
	resp := protocol.ApplyPatchApprovalResponse(map[string]any{"decision": "approved"})
	return &resp, nil
}

func (h *recordingHandler) ExecCommandApproval(ctx context.Context, params protocol.ExecCommandApprovalParams) (*protocol.ExecCommandApprovalResponse, error) {
	h.lastMethod = "execCommandApproval"
	resp := protocol.ExecCommandApprovalResponse(map[string]any{"decision": "approved"})
	return &resp, nil
}

func (h *recordingHandler) ItemCommandExecutionRequestApproval(ctx context.Context, params protocol.CommandExecutionRequestApprovalParams) (*protocol.CommandExecutionRequestApprovalResponse, error) {
	h.lastMethod = "item/commandExecution/requestApproval"
	resp := protocol.CommandExecutionRequestApprovalResponse(map[string]any{"decision": "accept"})
	return &resp, nil
}

func (h *recordingHandler) ItemFileChangeRequestApproval(ctx context.Context, params protocol.FileChangeRequestApprovalParams) (*protocol.FileChangeRequestApprovalResponse, error) {
	h.lastMethod = "item/fileChange/requestApproval"
	resp := protocol.FileChangeRequestApprovalResponse(map[string]any{"decision": "accept"})
	return &resp, nil
}

func (h *recordingHandler) ItemToolRequestUserInput(ctx context.Context, params protocol.ToolRequestUserInputParams) (*protocol.ToolRequestUserInputResponse, error) {
	h.lastMethod = "item/tool/requestUserInput"
	resp := protocol.ToolRequestUserInputResponse(map[string]any{"answers": map[string]any{}})
	return &resp, nil
}
