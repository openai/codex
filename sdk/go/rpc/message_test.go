package rpc

import (
	"context"
	"encoding/json"
	"errors"
	"testing"
	"time"
)

func TestRequestIDJSON(t *testing.T) {
	empty := RequestID{}
	if !empty.IsZero() {
		t.Fatalf("expected zero request id")
	}
	if empty.Key() != "" || empty.String() != "" {
		t.Fatalf("expected empty key/string")
	}

	stringID := NewStringRequestID("abc")
	if stringID.IsZero() {
		t.Fatalf("expected non-zero string id")
	}
	if stringID.Key() != "s:abc" {
		t.Fatalf("unexpected key: %s", stringID.Key())
	}
	if stringID.String() != "abc" {
		t.Fatalf("unexpected string: %s", stringID.String())
	}
	if data, err := json.Marshal(stringID); err != nil || string(data) != `"abc"` {
		t.Fatalf("unexpected string marshal: %s err=%v", data, err)
	}

	id := NewIntRequestID(42)
	data, err := json.Marshal(id)
	if err != nil {
		t.Fatalf("marshal error: %v", err)
	}
	if string(data) != "42" {
		t.Fatalf("unexpected int json: %s", data)
	}

	var parsed RequestID
	if err := json.Unmarshal([]byte(`"abc"`), &parsed); err != nil {
		t.Fatalf("unmarshal error: %v", err)
	}
	if parsed.String() != "abc" {
		t.Fatalf("unexpected string id: %s", parsed.String())
	}

	if err := json.Unmarshal([]byte(`{}`), &parsed); err == nil {
		t.Fatalf("expected invalid request id error")
	}
}

func TestParseMessageVariants(t *testing.T) {
	msg, err := parseMessage([]byte(`{"id":1,"method":"ping","params":{"ok":true}}`))
	if err != nil || msg.kind != messageRequest {
		t.Fatalf("expected request message, got %#v err=%v", msg, err)
	}

	msg, err = parseMessage([]byte(`{"method":"notify","params":{"ok":true}}`))
	if err != nil || msg.kind != messageNotification {
		t.Fatalf("expected notification message, got %#v err=%v", msg, err)
	}

	msg, err = parseMessage([]byte(`{"id":2,"result":{"ok":true}}`))
	if err != nil || msg.kind != messageResponse {
		t.Fatalf("expected response message, got %#v err=%v", msg, err)
	}

	msg, err = parseMessage([]byte(`{"id":3,"error":{"code":-1,"message":"bad"}}`))
	if err != nil || msg.kind != messageError {
		t.Fatalf("expected error message, got %#v err=%v", msg, err)
	}

	if _, err := parseMessage([]byte(`{"jsonrpc":"2.0"}`)); err == nil {
		t.Fatalf("expected unrecognized message error")
	}
}

func TestNotificationIteratorNext(t *testing.T) {
	done := make(chan struct{})
	errFn := func() error { return errors.New("closed") }
	ch := make(chan Notification, 1)
	iter := NotificationIterator{ch: ch, done: done, err: errFn}

	ch <- Notification{Method: "note"}
	note, err := iter.Next(context.Background())
	if err != nil || note.Method != "note" {
		t.Fatalf("unexpected note: %#v err=%v", note, err)
	}

	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := iter.Next(ctx); err == nil {
		t.Fatalf("expected context error")
	}

	close(done)
	if _, err := iter.Next(context.Background()); err == nil {
		t.Fatalf("expected done error")
	}
}

func TestReplayTransportWaitsForRead(t *testing.T) {
	replay := NewReplayTransport([]TranscriptEntry{
		{Direction: TranscriptWrite, Line: "write"},
		{Direction: TranscriptRead, Line: "read"},
	})

	done := make(chan struct{})
	go func() {
		defer close(done)
		if err := replay.WriteLine("write"); err != nil {
			t.Errorf("write error: %v", err)
		}
	}()

	select {
	case <-done:
	case <-time.After(1 * time.Second):
		t.Fatalf("write did not complete")
	}

	line, err := replay.ReadLine()
	if err != nil {
		t.Fatalf("read error: %v", err)
	}
	if line != "read" {
		t.Fatalf("unexpected line: %s", line)
	}
}
