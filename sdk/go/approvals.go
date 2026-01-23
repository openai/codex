package codex

import (
	"context"
	"errors"
	"log/slog"

	"github.com/openai/codex/sdk/go/protocol"
)

// AutoApproveHandler accepts every approval request it can.
// Logger controls approval logging. When nil, logs are discarded.
type AutoApproveHandler struct {
	Logger *slog.Logger
}

// ItemCommandExecutionRequestApproval approves command execution requests.
func (h AutoApproveHandler) ItemCommandExecutionRequestApproval(ctx context.Context, params protocol.CommandExecutionRequestApprovalParams) (*protocol.CommandExecutionRequestApprovalResponse, error) {
	logger := resolveLogger(h.Logger)
	logger.Info(
		"codex auto-approving command execution",
		"thread_id", params.ThreadID,
		"turn_id", params.TurnID,
		"item_id", params.ItemID,
		"command", params.Command,
		"cwd", params.Cwd,
	)
	resp := protocol.CommandExecutionRequestApprovalResponse(map[string]any{"decision": "accept"})
	return &resp, nil
}

// ItemFileChangeRequestApproval approves file change requests.
func (h AutoApproveHandler) ItemFileChangeRequestApproval(ctx context.Context, params protocol.FileChangeRequestApprovalParams) (*protocol.FileChangeRequestApprovalResponse, error) {
	logger := resolveLogger(h.Logger)
	logger.Info(
		"codex auto-approving file change",
		"thread_id", params.ThreadID,
		"turn_id", params.TurnID,
		"item_id", params.ItemID,
		"grant_root", params.GrantRoot,
	)
	resp := protocol.FileChangeRequestApprovalResponse(map[string]any{"decision": "accept"})
	return &resp, nil
}

// ItemToolRequestUserInput returns an error for tool user input prompts.
func (h AutoApproveHandler) ItemToolRequestUserInput(ctx context.Context, params protocol.ToolRequestUserInputParams) (*protocol.ToolRequestUserInputResponse, error) {
	logger := resolveLogger(h.Logger)
	logger.Info(
		"codex auto-approve handler cannot answer tool user input",
		"thread_id", params.ThreadID,
		"turn_id", params.TurnID,
		"item_id", params.ItemID,
		"questions", len(params.Questions),
	)
	return nil, errors.New("tool user input requires a custom handler")
}

// ApplyPatchApproval approves legacy patch requests.
func (h AutoApproveHandler) ApplyPatchApproval(ctx context.Context, params protocol.ApplyPatchApprovalParams) (*protocol.ApplyPatchApprovalResponse, error) {
	logger := resolveLogger(h.Logger)
	logger.Info(
		"codex auto-approving patch",
		"conversation_id", params.ConversationID,
		"call_id", params.CallID,
		"file_changes", len(params.FileChanges),
	)
	resp := protocol.ApplyPatchApprovalResponse(map[string]any{"decision": "approved"})
	return &resp, nil
}

// ExecCommandApproval approves legacy command requests.
func (h AutoApproveHandler) ExecCommandApproval(ctx context.Context, params protocol.ExecCommandApprovalParams) (*protocol.ExecCommandApprovalResponse, error) {
	logger := resolveLogger(h.Logger)
	logger.Info(
		"codex auto-approving command",
		"conversation_id", params.ConversationID,
		"call_id", params.CallID,
		"command", params.Command,
		"cwd", params.Cwd,
	)
	resp := protocol.ExecCommandApprovalResponse(map[string]any{"decision": "approved"})
	return &resp, nil
}
