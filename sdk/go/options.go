package codex

import (
	"io"
	"log/slog"

	"github.com/openai/codex/sdk/go/protocol"
	"github.com/openai/codex/sdk/go/rpc"
)

// Options configures the Codex client.
type Options struct {
	// Transport overrides the default stdio spawn.
	Transport rpc.Transport

	// Spawn controls how the default stdio process is launched.
	Spawn SpawnOptions

	// Logger receives SDK logs. If nil, logging is disabled.
	Logger *slog.Logger

	// ClientInfo identifies this SDK to the app-server.
	ClientInfo protocol.ClientInfo

	// ApprovalHandler handles server approval requests.
	ApprovalHandler rpc.ServerRequestHandler
}

// SpawnOptions configures the spawned codex app-server process.
type SpawnOptions struct {
	// CodexPath is the path to the codex binary (defaults to "codex").
	CodexPath string
	// ConfigOverrides are passed as --config key=value flags.
	ConfigOverrides []string
	// ExtraArgs are appended to the command line.
	ExtraArgs []string
	// Stderr captures stderr from the codex process (defaults to os.Stderr).
	Stderr io.Writer
}
