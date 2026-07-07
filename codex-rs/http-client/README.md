# codex-http-client

Low-level HTTP transport shared by Codex crates.

- Defines the request, response, streaming, and transport types used for outbound HTTP calls.
- Owns the `reqwest` implementation, custom CA handling, and ChatGPT Cloudflare cookie policy.
- Resolves system, PAC/WPAD, environment, and direct proxy routes for supported clients.

Higher-level retry, SSE, and request telemetry policy remains in `codex-client`.
