# SSH portal transport

Use `codex tcp-tunnel --environment-id ENV` as an OpenSSH `ProxyCommand`. The helper prefers QUIC/HTTP/3, starts TLS/HTTP/2 after 250 ms, and selects the first authenticated CONNECT accepted by the verified gateway. A single ten-second setup deadline covers transport, credential retrieval, and admission. SSH bytes are never replayed across transports.

When standard HTTPS proxy settings select a proxy, the helper uses that route without attempting direct QUIC. It honors `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY`, and supported OS proxy configuration, tunnels to the gateway, and verifies the gateway's TLS certificate before sending the admission credential. Direct TCP races resolved addresses through TLS and HTTP/2 readiness.

Desktop uses the same stdio path with a one-use `--token-file` in a private directory. OAuth uses the environment ID and published port; legacy JWT admission additionally uses `--target IP:PORT`. The helper reads and removes the token file only after verifying the gateway. SSH owns each helper process; multiplexing belongs to OpenSSH ControlMaster.

On Linux, run `codex-rs/tcp-tunnel/tests/real-ssh.sh` to verify a complete upload and a remote exit with local stdin still open through a real OpenSSH server and HTTP/3 tunnel. It requires OpenSSH, Python, `timeout`, and the normal Rust test tools.
