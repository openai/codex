package rpc

import (
	"bufio"
	"context"
	"errors"
	"io"
	"os"
	"os/exec"
	"strings"
	"sync"
)

// Transport reads and writes JSON-RPC lines.
type Transport interface {
	ReadLine() (string, error)
	WriteLine(line string) error
	Close() error
}

// StdioTransport wraps a spawned process using stdin/stdout JSONL.
type StdioTransport struct {
	cmd    *exec.Cmd
	stdin  io.WriteCloser
	stdout *bufio.Reader
	mu     sync.Mutex
}

// SpawnStdio starts a command and uses its stdin/stdout for JSON-RPC.
func SpawnStdio(ctx context.Context, binary string, args []string, stderr io.Writer) (*StdioTransport, error) {
	if binary == "" {
		return nil, errors.New("codex binary path is empty")
	}

	cmd := exec.CommandContext(ctx, binary, args...)
	cmd.Stderr = stderr

	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, err
	}

	stdin, err := cmd.StdinPipe()
	if err != nil {
		return nil, err
	}

	if err := cmd.Start(); err != nil {
		return nil, err
	}

	return &StdioTransport{
		cmd:    cmd,
		stdin:  stdin,
		stdout: bufio.NewReader(stdout),
	}, nil
}

// ReadLine reads a single line from stdout.
func (t *StdioTransport) ReadLine() (string, error) {
	line, err := t.stdout.ReadString('\n')
	if err != nil {
		if errors.Is(err, io.EOF) && line != "" {
			return strings.TrimRight(line, "\n"), nil
		}
		return "", err
	}
	return strings.TrimRight(line, "\n"), nil
}

// WriteLine writes a single line to stdin.
func (t *StdioTransport) WriteLine(line string) error {
	t.mu.Lock()
	defer t.mu.Unlock()

	if !strings.HasSuffix(line, "\n") {
		line += "\n"
	}

	_, err := io.WriteString(t.stdin, line)
	return err
}

// Close shuts down the process.
func (t *StdioTransport) Close() error {
	_ = t.stdin.Close()
	if t.cmd != nil && t.cmd.Process != nil {
		_ = t.cmd.Process.Kill()
	}
	if t.cmd != nil {
		_ = t.cmd.Wait()
	}
	return nil
}

// ConnTransport wraps an io.ReadWriteCloser.
type ConnTransport struct {
	conn   io.ReadWriteCloser
	reader *bufio.Reader
	mu     sync.Mutex
}

// NewConnTransport wraps the connection in a Transport.
func NewConnTransport(conn io.ReadWriteCloser) *ConnTransport {
	return &ConnTransport{conn: conn, reader: bufio.NewReader(conn)}
}

// ReadLine reads a line from the connection.
func (t *ConnTransport) ReadLine() (string, error) {
	line, err := t.reader.ReadString('\n')
	if err != nil {
		if errors.Is(err, io.EOF) && line != "" {
			return strings.TrimRight(line, "\n"), nil
		}
		return "", err
	}
	return strings.TrimRight(line, "\n"), nil
}

// WriteLine writes a line to the connection.
func (t *ConnTransport) WriteLine(line string) error {
	t.mu.Lock()
	defer t.mu.Unlock()

	if !strings.HasSuffix(line, "\n") {
		line += "\n"
	}

	_, err := io.WriteString(t.conn, line)
	return err
}

// Close closes the connection.
func (t *ConnTransport) Close() error {
	return t.conn.Close()
}

// DefaultStderr returns a safe default for spawned processes.
func DefaultStderr() io.Writer {
	return os.Stderr
}
