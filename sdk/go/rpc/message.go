package rpc

import (
	"encoding/json"
	"fmt"
)

// RequestID represents a JSON-RPC request id (string or integer).
type RequestID struct {
	str *string
	num *int64
}

// NewStringRequestID creates a string request id.
func NewStringRequestID(value string) RequestID {
	return RequestID{str: &value}
}

// NewIntRequestID creates an integer request id.
func NewIntRequestID(value int64) RequestID {
	return RequestID{num: &value}
}

// IsZero reports whether the id is unset.
func (id RequestID) IsZero() bool {
	return id.str == nil && id.num == nil
}

// Key returns a stable string key for map usage.
func (id RequestID) Key() string {
	if id.str != nil {
		return "s:" + *id.str
	}
	if id.num != nil {
		return fmt.Sprintf("i:%d", *id.num)
	}
	return ""
}

// String returns a printable representation.
func (id RequestID) String() string {
	if id.str != nil {
		return *id.str
	}
	if id.num != nil {
		return fmt.Sprintf("%d", *id.num)
	}
	return ""
}

// MarshalJSON implements json.Marshaler.
func (id RequestID) MarshalJSON() ([]byte, error) {
	switch {
	case id.str != nil:
		return json.Marshal(*id.str)
	case id.num != nil:
		return json.Marshal(*id.num)
	default:
		return []byte("null"), nil
	}
}

// UnmarshalJSON implements json.Unmarshaler.
func (id *RequestID) UnmarshalJSON(data []byte) error {
	if len(data) == 0 || string(data) == "null" {
		*id = RequestID{}
		return nil
	}

	var s string
	if err := json.Unmarshal(data, &s); err == nil {
		id.str = &s
		id.num = nil
		return nil
	}

	var n int64
	if err := json.Unmarshal(data, &n); err == nil {
		id.num = &n
		id.str = nil
		return nil
	}

	return fmt.Errorf("invalid request id: %s", string(data))
}

// JSONRPCRequest represents a JSON-RPC request.
type JSONRPCRequest struct {
	ID     RequestID       `json:"id"`
	Method string          `json:"method"`
	Params json.RawMessage `json:"params,omitempty"`
}

// JSONRPCNotification represents a JSON-RPC notification.
type JSONRPCNotification struct {
	Method string          `json:"method"`
	Params json.RawMessage `json:"params,omitempty"`
}

// JSONRPCResponse represents a JSON-RPC response.
type JSONRPCResponse struct {
	ID     RequestID       `json:"id"`
	Result json.RawMessage `json:"result"`
}

// JSONRPCError represents a JSON-RPC error response.
type JSONRPCError struct {
	ID    RequestID         `json:"id"`
	Error JSONRPCErrorError `json:"error"`
}

// JSONRPCErrorError describes the error payload.
type JSONRPCErrorError struct {
	Code    int64           `json:"code"`
	Message string          `json:"message"`
	Data    json.RawMessage `json:"data,omitempty"`
}

// ResponseError wraps a JSON-RPC error as a Go error.
type ResponseError struct {
	ID     RequestID
	Detail JSONRPCErrorError
}

func (err *ResponseError) Error() string {
	return fmt.Sprintf("json-rpc error %d: %s", err.Detail.Code, err.Detail.Message)
}
