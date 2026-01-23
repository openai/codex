package codex

import (
	"io"
	"log/slog"

	"github.com/openai/codex/sdk/go/rpc"
)

func resolveLogger(logger *slog.Logger) *slog.Logger {
	if logger != nil {
		return logger
	}
	return slog.New(slog.NewTextHandler(io.Discard, nil))
}

func attachApprovalLogger(handler rpc.ServerRequestHandler, logger *slog.Logger) rpc.ServerRequestHandler {
	switch value := handler.(type) {
	case AutoApproveHandler:
		if value.Logger == nil {
			value.Logger = logger
		}
		return value
	case *AutoApproveHandler:
		if value != nil && value.Logger == nil {
			value.Logger = logger
		}
		return value
	default:
		return handler
	}
}
