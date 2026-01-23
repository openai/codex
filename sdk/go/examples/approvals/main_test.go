package main

import (
	"bytes"
	"io"
	"log/slog"
	"os"
	"strings"
	"testing"
)

func TestMainReplay(t *testing.T) {
	t.Setenv(exampleReplayEnv, "1")

	output := captureOutput(main)
	if strings.TrimSpace(output) != "Approved summary" {
		t.Fatalf("unexpected output: %q", output)
	}
}

func TestExampleOptionsDefault(t *testing.T) {
	t.Setenv(exampleReplayEnv, "")
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	opts := exampleOptions("prompt", logger)
	if opts.Transport != nil {
		t.Fatalf("expected nil transport for default options")
	}
	if opts.ApprovalHandler == nil {
		t.Fatalf("expected approval handler for default options")
	}
}

func captureOutput(fn func()) string {
	original := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		panic(err)
	}
	os.Stdout = w

	fn()

	_ = w.Close()
	os.Stdout = original

	var buf bytes.Buffer
	_, _ = buf.ReadFrom(r)
	_ = r.Close()
	return buf.String()
}
