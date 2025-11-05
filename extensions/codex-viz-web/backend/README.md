# Codex Viz Backend

Rust backend server for Kamui4d-style repository visualization.

## Features

- **REST API** for git repository analysis
  - `GET /api/commits` - 3D commit history
  - `GET /api/files/heatmap` - File change statistics
  - `GET /api/branches/graph` - Branch structure
- **WebSocket** for real-time updates (`/api/realtime`)
- **Git Analysis Engine** with 3D coordinate calculation
- **File System Watcher** for live monitoring

## Development

```bash
# Build
cargo build

# Run
cargo run

# Server will start on http://127.0.0.1:3001
```

## API Examples

```bash
# Get commits
curl "http://localhost:3001/api/commits?limit=100"

# Get file heatmap
curl "http://localhost:3001/api/files/heatmap"

# Get branch graph
curl "http://localhost:3001/api/branches/graph"
```

## WebSocket

Connect to `ws://localhost:3001/api/realtime` for real-time git events.

