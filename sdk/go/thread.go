package codex

import (
	"context"
	"log/slog"

	"github.com/openai/codex/sdk/go/rpc"
)

// Thread represents an active conversation thread.
type Thread struct {
	client *rpc.Client
	id     string
	logger *slog.Logger
}

// ID returns the thread id.
func (t *Thread) ID() string {
	return t.id
}

// Run sends a text prompt and waits for the turn to finish.
func (t *Thread) Run(ctx context.Context, prompt string, opts *TurnOptions) (*TurnResult, error) {
	return t.RunInputs(ctx, []Input{TextInput(prompt)}, opts)
}

// RunInputs sends structured inputs and waits for the turn to finish.
func (t *Thread) RunInputs(ctx context.Context, inputs []Input, opts *TurnOptions) (*TurnResult, error) {
	logger := resolveLogger(t.logger)
	stream, err := t.RunStreamed(ctx, inputs, opts)
	if err != nil {
		return nil, err
	}
	defer stream.Close()

	result := &TurnResult{}
	for {
		note, err := stream.Next(ctx)
		if err != nil {
			return nil, err
		}
		result.Notifications = append(result.Notifications, note)
		updateTurnResult(result, note)

		if note.Method == "turn/completed" {
			logger.Info("codex turn completed", "thread_id", t.id, "turn_id", result.TurnID)
			return result, nil
		}
		if note.Method == "turn/failed" || note.Method == "error" {
			if turnErr := notificationError(note); turnErr != nil {
				logger.Error("codex turn failed", "thread_id", t.id, "turn_id", result.TurnID, "error", turnErr)
				return nil, turnErr
			}
		}
	}
}

// RunStreamed sends structured inputs and returns a streaming iterator.
func (t *Thread) RunStreamed(ctx context.Context, inputs []Input, opts *TurnOptions) (*TurnStream, error) {
	logger := resolveLogger(t.logger)
	iter := t.client.SubscribeNotifications(0)

	params := buildTurnParams(t.id, inputs, opts)
	logger.Info("codex starting turn", "thread_id", t.id, "input_count", len(inputs))
	if err := t.client.Call(ctx, "turn/start", params, nil); err != nil {
		logger.Error("codex turn start failed", "thread_id", t.id, "error", err)
		iter.Close()
		return nil, err
	}

	return &TurnStream{iter: iter, threadID: t.id}, nil
}

func buildTurnParams(threadID string, inputs []Input, opts *TurnOptions) map[string]any {
	payload := map[string]any{
		"threadId": threadID,
		"input":    inputs,
	}
	if opts == nil {
		return payload
	}

	if opts.Cwd != "" {
		payload["cwd"] = opts.Cwd
	}
	if opts.ApprovalPolicy != nil {
		payload["approvalPolicy"] = opts.ApprovalPolicy
	}
	if opts.SandboxPolicy != nil {
		payload["sandboxPolicy"] = opts.SandboxPolicy
	}
	if opts.Model != "" {
		payload["model"] = opts.Model
	}
	if opts.Effort != nil {
		payload["effort"] = opts.Effort
	}
	if opts.Summary != nil {
		payload["summary"] = opts.Summary
	}
	if opts.OutputSchema != nil {
		payload["outputSchema"] = opts.OutputSchema
	}
	if opts.CollaborationMode != nil {
		payload["collaborationMode"] = opts.CollaborationMode
	}

	return payload
}
