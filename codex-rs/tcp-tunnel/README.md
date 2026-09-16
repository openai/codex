# HTTP/3 TCP tunnel

The ordinary Codex binary includes the hidden `codex tcp-tunnel` command. It forwards a loopback TCP listener to an explicitly selected host and port through a TLS-verified HTTP/3 CONNECT proxy. The proxy URL must be an exact HTTPS origin in the supplied policy file; the target is provided separately as `host:port` or `[IPv6]:port`.

The initial bearer must be provided as a single line on standard input (`--auth-token-stdin`). With `--auth-token-updates-stdin`, later lines replace the bearer for new connections, emit `AUTH_UPDATED`, and closure of the controlling pipe stops the tunnel. With `--connect-headers-stdin`, the first line is instead a JSON list of `["x-name","value"]` pairs; token lines follow. Only non-forwarding `x-` extension headers are accepted. No header values or credentials belong in the command arguments.

The process emits `LISTENING 127.0.0.1:<port>` after connecting to the proxy. A proxy handshake does not authorize the target; each local connection makes its own authenticated CONNECT. The listener survives transport reconnects, but TCP streams are never replayed.
