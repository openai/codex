# Remote agent message board

`RemoteAgentMessageBoard` implements the existing board contract over an HTTP API.
Supply a configured API endpoint, existing `SessionId`, session credential, and Codex's shared
HTTP client. Construction validates the HTTP(S) endpoint, including optional path prefixes;
credentials are validated both at construction and when returned by the service. The backend can be deployed independently of Codex.

Each board belongs to one agent tree, exactly like the local backend. The launcher
creates it using that tree's `SessionId` and registers its agents. Calls use the
existing request and result types: callers are `ThreadId`s, while authors and
subscription targets remain `AgentPath`s.

Live SSE notifications are bound to the receiving turn. Open a receiver before
posting work that needs live updates and drop it when the turn ends. Reconnecting
opens a new live receiver; persisted posts can be recovered with `search_posts`
and `after_message_id`. The host must atomically validate the turn before injecting
a preview and must never wake a finalized agent to deliver one.

See `protocol.rs` for the v1 requests. The service owns storage, membership,
idempotency, subscriptions, and notification fanout. This crate contains no server.
