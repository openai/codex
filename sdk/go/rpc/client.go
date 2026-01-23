package rpc

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"strings"
	"sync"
	"sync/atomic"
)

type ClientOptions struct {
	Logger         *slog.Logger
	RequestHandler ServerRequestHandler
}

// Client manages JSON-RPC requests over a Transport.
type Client struct {
	transport Transport
	logger    *slog.Logger

	nextID int64

	pendingMu sync.Mutex
	pending   map[string]chan response

	subsMu  sync.Mutex
	subs    map[int]chan Notification
	nextSub int

	handlerMu sync.RWMutex
	handler   ServerRequestHandler

	done     chan struct{}
	doneOnce sync.Once
	err      error
}

// NewClient creates a JSON-RPC client over a Transport.
func NewClient(transport Transport, options ClientOptions) *Client {
	logger := options.Logger
	if logger == nil {
		logger = slog.New(slog.NewTextHandler(io.Discard, nil))
	}

	client := &Client{
		transport: transport,
		logger:    logger,
		pending:   make(map[string]chan response),
		subs:      make(map[int]chan Notification),
		handler:   options.RequestHandler,
		done:      make(chan struct{}),
	}

	go client.readLoop()

	return client
}

// Close shuts down the client and transport.
func (c *Client) Close() error {
	c.finish(errors.New("client closed"))
	return c.transport.Close()
}

// SetRequestHandler replaces the server request handler.
func (c *Client) SetRequestHandler(handler ServerRequestHandler) {
	c.handlerMu.Lock()
	defer c.handlerMu.Unlock()
	c.handler = handler
}

// Call sends a JSON-RPC request and decodes the response into result.
func (c *Client) Call(ctx context.Context, method string, params any, result any) error {
	if err := c.ensureOpen(); err != nil {
		return err
	}

	id := c.nextRequestID()
	respCh := make(chan response, 1)

	c.pendingMu.Lock()
	c.pending[id.Key()] = respCh
	c.pendingMu.Unlock()

	payload := JSONRPCRequest{ID: id, Method: method}
	if params != nil {
		data, err := json.Marshal(params)
		if err != nil {
			c.deletePending(id)
			return err
		}
		payload.Params = data
	}

	if err := c.send(payload); err != nil {
		c.deletePending(id)
		return err
	}

	select {
	case <-c.done:
		c.deletePending(id)
		return c.errOrClosed()
	case <-ctx.Done():
		c.deletePending(id)
		return ctx.Err()
	case resp := <-respCh:
		if resp.err != nil {
			return resp.err
		}
		if result == nil {
			return nil
		}
		return json.Unmarshal(resp.result, result)
	}
}

// Notify sends a JSON-RPC notification.
func (c *Client) Notify(ctx context.Context, method string, params any) error {
	if err := c.ensureOpen(); err != nil {
		return err
	}

	payload := JSONRPCNotification{Method: method}
	if params != nil {
		data, err := json.Marshal(params)
		if err != nil {
			return err
		}
		payload.Params = data
	}

	data, err := json.Marshal(payload)
	if err != nil {
		return err
	}

	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-c.done:
		return c.errOrClosed()
	default:
		return c.transport.WriteLine(string(data))
	}
}

// SubscribeNotifications creates an iterator over server notifications.
func (c *Client) SubscribeNotifications(buffer int) *NotificationIterator {
	if buffer <= 0 {
		buffer = 64
	}
	ch := make(chan Notification, buffer)

	c.subsMu.Lock()
	id := c.nextSub
	c.nextSub++
	c.subs[id] = ch
	c.subsMu.Unlock()

	return &NotificationIterator{
		ch:   ch,
		done: c.done,
		err:  c.errOrClosed,
		cancel: func() {
			c.subsMu.Lock()
			delete(c.subs, id)
			close(ch)
			c.subsMu.Unlock()
		},
	}
}

func (c *Client) readLoop() {
	for {
		line, err := c.transport.ReadLine()
		if err != nil {
			c.finish(err)
			return
		}
		if strings.TrimSpace(line) == "" {
			continue
		}

		msg, err := parseMessage([]byte(line))
		if err != nil {
			c.logger.Warn("failed to parse json-rpc message", slog.Any("error", err))
			continue
		}

		switch msg.kind {
		case messageResponse:
			c.handleResponse(msg.response)
		case messageError:
			c.handleError(msg.error)
		case messageRequest:
			c.handleServerRequest(msg.request)
		case messageNotification:
			c.handleNotification(msg.notification)
		}
	}
}

func (c *Client) handleResponse(resp JSONRPCResponse) {
	c.pendingMu.Lock()
	ch := c.pending[resp.ID.Key()]
	delete(c.pending, resp.ID.Key())
	c.pendingMu.Unlock()

	if ch == nil {
		return
	}

	ch <- response{result: resp.Result}
}

func (c *Client) handleError(resp JSONRPCError) {
	c.pendingMu.Lock()
	ch := c.pending[resp.ID.Key()]
	delete(c.pending, resp.ID.Key())
	c.pendingMu.Unlock()

	if ch == nil {
		return
	}

	ch <- response{err: &ResponseError{ID: resp.ID, Detail: resp.Error}}
}

func (c *Client) handleNotification(note JSONRPCNotification) {
	notification, err := parseServerNotification(note.Method, note.Params)
	if err != nil {
		c.logger.Warn("failed to decode notification", slog.String("method", note.Method), slog.Any("error", err))
	}

	c.subsMu.Lock()
	for _, ch := range c.subs {
		select {
		case ch <- notification:
		default:
		}
	}
	c.subsMu.Unlock()
}

func (c *Client) handleServerRequest(req JSONRPCRequest) {
	handler := c.currentHandler()
	if handler == nil {
		_ = c.replyError(req.ID, -32601, "no handler configured", nil)
		return
	}

	result, err := dispatchServerRequest(context.Background(), handler, req)
	if err != nil {
		_ = c.replyError(req.ID, -32602, err.Error(), nil)
		return
	}

	_ = c.replyResult(req.ID, result)
}

func (c *Client) replyResult(id RequestID, result any) error {
	data, err := json.Marshal(result)
	if err != nil {
		return err
	}
	resp := JSONRPCResponse{ID: id, Result: data}
	return c.send(resp)
}

func (c *Client) replyError(id RequestID, code int64, message string, data json.RawMessage) error {
	resp := JSONRPCError{
		ID: id,
		Error: JSONRPCErrorError{
			Code:    code,
			Message: message,
			Data:    data,
		},
	}
	return c.send(resp)
}

func (c *Client) send(payload any) error {
	data, err := json.Marshal(payload)
	if err != nil {
		return err
	}
	return c.transport.WriteLine(string(data))
}

func (c *Client) nextRequestID() RequestID {
	next := atomic.AddInt64(&c.nextID, 1)
	return NewIntRequestID(next)
}

func (c *Client) deletePending(id RequestID) {
	c.pendingMu.Lock()
	delete(c.pending, id.Key())
	c.pendingMu.Unlock()
}

func (c *Client) currentHandler() ServerRequestHandler {
	c.handlerMu.RLock()
	defer c.handlerMu.RUnlock()
	return c.handler
}

func (c *Client) ensureOpen() error {
	select {
	case <-c.done:
		return c.errOrClosed()
	default:
		return nil
	}
}

func (c *Client) errOrClosed() error {
	if c.err != nil {
		return c.err
	}
	return errors.New("connection closed")
}

func (c *Client) finish(err error) {
	c.doneOnce.Do(func() {
		c.err = err
		close(c.done)
		c.pendingMu.Lock()
		for _, ch := range c.pending {
			ch <- response{err: err}
		}
		c.pending = map[string]chan response{}
		c.pendingMu.Unlock()

		c.subsMu.Lock()
		for _, ch := range c.subs {
			_ = ch
		}
		c.subs = map[int]chan Notification{}
		c.subsMu.Unlock()
	})
}

type response struct {
	result json.RawMessage
	err    error
}

func (c *Client) errResult(id RequestID, err error) {
	c.pendingMu.Lock()
	ch := c.pending[id.Key()]
	delete(c.pending, id.Key())
	c.pendingMu.Unlock()

	if ch != nil {
		ch <- response{err: err}
	}
}

// NotificationIterator iterates notifications from the server.
type NotificationIterator struct {
	ch     <-chan Notification
	done   <-chan struct{}
	err    func() error
	cancel func()
}

// Next returns the next notification or an error.
func (it *NotificationIterator) Next(ctx context.Context) (Notification, error) {
	select {
	case <-ctx.Done():
		return Notification{}, ctx.Err()
	case <-it.done:
		return Notification{}, it.err()
	case note, ok := <-it.ch:
		if !ok {
			return Notification{}, it.err()
		}
		return note, nil
	}
}

// Close unsubscribes the iterator.
func (it *NotificationIterator) Close() {
	if it.cancel != nil {
		it.cancel()
	}
}

// parseMessage decodes a JSON-RPC line into a typed message.
func parseMessage(data []byte) (message, error) {
	var envelope struct {
		ID     json.RawMessage    `json:"id"`
		Method string             `json:"method"`
		Params json.RawMessage    `json:"params"`
		Result json.RawMessage    `json:"result"`
		Error  *JSONRPCErrorError `json:"error"`
	}

	if err := json.Unmarshal(data, &envelope); err != nil {
		return message{}, err
	}

	if envelope.Method != "" {
		if len(envelope.ID) > 0 {
			id, err := parseRequestID(envelope.ID)
			if err != nil {
				return message{}, err
			}
			return message{kind: messageRequest, request: JSONRPCRequest{ID: id, Method: envelope.Method, Params: envelope.Params}}, nil
		}
		return message{kind: messageNotification, notification: JSONRPCNotification{Method: envelope.Method, Params: envelope.Params}}, nil
	}

	if len(envelope.Result) > 0 {
		id, err := parseRequestID(envelope.ID)
		if err != nil {
			return message{}, err
		}
		return message{kind: messageResponse, response: JSONRPCResponse{ID: id, Result: envelope.Result}}, nil
	}

	if envelope.Error != nil {
		id, err := parseRequestID(envelope.ID)
		if err != nil {
			return message{}, err
		}
		return message{kind: messageError, error: JSONRPCError{ID: id, Error: *envelope.Error}}, nil
	}

	return message{}, fmt.Errorf("unrecognized json-rpc message")
}

func parseRequestID(raw json.RawMessage) (RequestID, error) {
	var id RequestID
	if err := id.UnmarshalJSON(raw); err != nil {
		return RequestID{}, err
	}
	return id, nil
}

type messageKind int

const (
	messageResponse messageKind = iota
	messageError
	messageRequest
	messageNotification
)

type message struct {
	kind         messageKind
	response     JSONRPCResponse
	error        JSONRPCError
	request      JSONRPCRequest
	notification JSONRPCNotification
}
