## Overview
`rmcp_test_server` boots the stdio-based MCP test server described in `mod.spec.md`, wiring the shared `TestToolServer` with the echo tool and serving until the client disconnects.

## Notes
- See `rmcp-client/src/bin/mod.spec.md` for shared behavior across all helper binaries.
