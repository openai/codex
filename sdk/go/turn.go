package codex

import (
	"context"
	"errors"
	"reflect"

	"github.com/openai/codex/sdk/go/rpc"
)

// TurnOptions configures a turn/start request.
type TurnOptions struct {
	Cwd               string
	ApprovalPolicy    any
	SandboxPolicy     any
	Model             string
	Effort            any
	Summary           any
	OutputSchema      any
	CollaborationMode any
}

// TurnResult aggregates notifications for a completed turn.
type TurnResult struct {
	TurnID        string
	Notifications []rpc.Notification
	Items         []any
	FinalResponse string
}

// TurnStream iterates turn notifications.
type TurnStream struct {
	iter     *rpc.NotificationIterator
	threadID string
}

// Next returns the next notification for this turn.
func (s *TurnStream) Next(ctx context.Context) (rpc.Notification, error) {
	for {
		note, err := s.iter.Next(ctx)
		if err != nil {
			return note, err
		}
		if s.threadID == "" {
			return note, nil
		}
		if matchesThreadID(note, s.threadID) {
			return note, nil
		}
	}
}

// Close stops the iterator.
func (s *TurnStream) Close() {
	s.iter.Close()
}

func updateTurnResult(result *TurnResult, note rpc.Notification) {
	if note.Method == "item/completed" {
		if item, ok := extractItem(note); ok {
			result.Items = append(result.Items, item)
			if text, ok := extractText(item); ok {
				result.FinalResponse = text
			}
		}
	}

	if note.Method == "turn/started" {
		if id := extractTurnID(note.Params); id != "" {
			result.TurnID = id
		}
	}

	if note.Method == "turn/completed" {
		if id := extractTurnID(note.Params); id != "" {
			result.TurnID = id
		}
	}
}

func notificationError(note rpc.Notification) error {
	if note.Method == "error" {
		payload := asMap(note.Params)
		if payload == nil {
			return errors.New("turn error")
		}
		if willRetry, ok := payload["willRetry"].(bool); ok && willRetry {
			return nil
		}
		if errMap, ok := payload["error"].(map[string]any); ok {
			if msg, ok := errMap["message"].(string); ok && msg != "" {
				return errors.New(msg)
			}
		}
		return errors.New("turn error")
	}
	if note.Method == "turn/completed" {
		payload := asMap(note.Params)
		if payload == nil {
			return nil
		}
		if turnMap, ok := payload["turn"].(map[string]any); ok {
			if status, ok := turnMap["status"].(string); ok && status == "failed" {
				if message := extractErrorMessage(turnMap["error"]); message != "" {
					return errors.New(message)
				}
				return errors.New("turn failed")
			}
		}
	}
	return nil
}

func matchesThreadID(note rpc.Notification, threadID string) bool {
	if id := extractThreadID(note.Params); id != "" {
		return id == threadID
	}
	return true
}

func extractItem(note rpc.Notification) (any, bool) {
	if payload, ok := note.Params.(map[string]any); ok {
		if item, ok := payload["item"]; ok {
			return item, true
		}
	}
	return nil, false
}

func extractText(item any) (string, bool) {
	switch value := item.(type) {
	case map[string]any:
		if text, ok := value["text"].(string); ok {
			return text, true
		}
		if len(value) == 1 {
			for _, inner := range value {
				if innerMap, ok := inner.(map[string]any); ok {
					if text, ok := innerMap["text"].(string); ok {
						return text, true
					}
				}
			}
		}
	default:
		rv := reflect.ValueOf(item)
		if rv.Kind() == reflect.Struct {
			field := rv.FieldByName("Text")
			if field.IsValid() && field.Kind() == reflect.String {
				return field.String(), true
			}
		}
	}
	return "", false
}

func extractThreadID(params any) string {
	if params == nil {
		return ""
	}
	rv := reflect.ValueOf(params)
	if rv.Kind() == reflect.Struct {
		if field := rv.FieldByName("ThreadID"); field.IsValid() && field.Kind() == reflect.String {
			return field.String()
		}
		if field := rv.FieldByName("ThreadId"); field.IsValid() && field.Kind() == reflect.String {
			return field.String()
		}
		if field := rv.FieldByName("Thread"); field.IsValid() && field.Kind() == reflect.Struct {
			if idField := field.FieldByName("ID"); idField.IsValid() && idField.Kind() == reflect.String {
				return idField.String()
			}
			if idField := field.FieldByName("Id"); idField.IsValid() && idField.Kind() == reflect.String {
				return idField.String()
			}
		}
	}
	if payload, ok := params.(map[string]any); ok {
		if id, ok := payload["threadId"].(string); ok {
			return id
		}
		if thread, ok := payload["thread"].(map[string]any); ok {
			if id, ok := thread["id"].(string); ok {
				return id
			}
		}
	}
	return ""
}

func extractTurnID(params any) string {
	if params == nil {
		return ""
	}
	rv := reflect.ValueOf(params)
	if rv.Kind() == reflect.Struct {
		if field := rv.FieldByName("Turn"); field.IsValid() && field.Kind() == reflect.Struct {
			if idField := field.FieldByName("ID"); idField.IsValid() && idField.Kind() == reflect.String {
				return idField.String()
			}
		}
	}
	if payload, ok := params.(map[string]any); ok {
		if turn, ok := payload["turn"].(map[string]any); ok {
			if id, ok := turn["id"].(string); ok {
				return id
			}
		}
	}
	return ""
}

func extractErrorMessage(errValue any) string {
	if errValue == nil {
		return ""
	}
	if value, ok := errValue.(map[string]any); ok {
		if msg, ok := value["message"].(string); ok {
			return msg
		}
	}
	return ""
}

func asMap(value any) map[string]any {
	if payload, ok := value.(map[string]any); ok {
		return payload
	}
	return nil
}
