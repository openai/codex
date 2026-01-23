package rpc

import (
	"errors"
	"fmt"
	"io"
	"sync"
)

// TranscriptDirection describes the direction of a recorded line.
type TranscriptDirection string

const (
	TranscriptRead  TranscriptDirection = "read"
	TranscriptWrite TranscriptDirection = "write"
)

// TranscriptEntry stores a single JSON-RPC line and its direction.
type TranscriptEntry struct {
	Direction TranscriptDirection `json:"direction"`
	Line      string              `json:"line"`
}

// ReplayTransport replays a transcript of JSON-RPC lines.
type ReplayTransport struct {
	mu         sync.Mutex
	cond       *sync.Cond
	transcript []TranscriptEntry
	index      int
	closed     bool
}

// NewReplayTransport creates a ReplayTransport for a transcript.
func NewReplayTransport(transcript []TranscriptEntry) *ReplayTransport {
	copyTranscript := make([]TranscriptEntry, len(transcript))
	copy(copyTranscript, transcript)
	replay := &ReplayTransport{transcript: copyTranscript}
	replay.cond = sync.NewCond(&replay.mu)
	return replay
}

// ReadLine returns the next recorded read line.
func (t *ReplayTransport) ReadLine() (string, error) {
	t.mu.Lock()
	defer t.mu.Unlock()

	for {
		if t.closed {
			return "", io.EOF
		}
		if t.index < len(t.transcript) {
			entry := t.transcript[t.index]
			if entry.Direction == TranscriptRead {
				t.index++
				t.cond.Broadcast()
				return entry.Line, nil
			}
		}
		t.cond.Wait()
	}
}

// WriteLine validates the next recorded write line.
func (t *ReplayTransport) WriteLine(line string) error {
	t.mu.Lock()
	defer t.mu.Unlock()

	for {
		if t.closed {
			return errors.New("replay transport closed")
		}
		if t.index >= len(t.transcript) {
			return fmt.Errorf("unexpected WriteLine: no transcript entries left")
		}
		entry := t.transcript[t.index]
		if entry.Direction == TranscriptWrite {
			if entry.Line != line {
				return fmt.Errorf("unexpected WriteLine: got %q, want %q", line, entry.Line)
			}
			t.index++
			t.cond.Broadcast()
			return nil
		}
		t.cond.Wait()
	}
}

// Close stops the replay transport.
func (t *ReplayTransport) Close() error {
	t.mu.Lock()
	t.closed = true
	t.cond.Broadcast()
	t.mu.Unlock()
	return nil
}

// RecordTransport records all JSON-RPC traffic to a transcript.
type RecordTransport struct {
	transport  Transport
	mu         sync.Mutex
	transcript []TranscriptEntry
}

// RercordTransport is a misspelled alias for RecordTransport.
type RercordTransport = RecordTransport

// NewRecordTransport wraps a transport and records traffic.
func NewRecordTransport(transport Transport) *RecordTransport {
	return &RecordTransport{transport: transport}
}

// NewRercordTransport wraps a transport and records traffic.
func NewRercordTransport(transport Transport) *RecordTransport {
	return NewRecordTransport(transport)
}

// ReadLine reads from the underlying transport and records the line.
func (t *RecordTransport) ReadLine() (string, error) {
	line, err := t.transport.ReadLine()
	if line != "" {
		t.append(TranscriptEntry{Direction: TranscriptRead, Line: line})
	}
	return line, err
}

// WriteLine writes to the underlying transport and records the line.
func (t *RecordTransport) WriteLine(line string) error {
	if err := t.transport.WriteLine(line); err != nil {
		return err
	}
	t.append(TranscriptEntry{Direction: TranscriptWrite, Line: line})
	return nil
}

// Close closes the underlying transport.
func (t *RecordTransport) Close() error {
	return t.transport.Close()
}

// Transcript returns a copy of the recorded transcript.
func (t *RecordTransport) Transcript() []TranscriptEntry {
	t.mu.Lock()
	defer t.mu.Unlock()

	out := make([]TranscriptEntry, len(t.transcript))
	copy(out, t.transcript)
	return out
}

func (t *RecordTransport) append(entry TranscriptEntry) {
	t.mu.Lock()
	t.transcript = append(t.transcript, entry)
	t.mu.Unlock()
}
