# Codex Repository Visualizer

Kamui4d-style 3D repository visualization powered by Rust + React + Three.js

## 🎨 Features

### 3D/4D Commit History
- **X-axis**: Branch separation
- **Y-axis**: Time (commit timestamp)
- **Z-axis**: Depth (parent-child relationships)
- **Color**: Author differentiation
- Interactive navigation with mouse controls

### File Change Heatmap
- Hot/cold visualization of file modifications
- Change frequency represented by height
- Color gradation from blue (cold) to red (hot)
- File size correlation

### Branch Structure
- 3D branch topology
- Active branch highlighting
- Merge point visualization
- Branch lifespan statistics

### Real-time Monitoring
- WebSocket-based live updates
- New commit notifications
- File change tracking
- Collaborative visualization for team development

## 🚀 Quick Start

### Prerequisites

- **Rust** (2024 edition or later)
- **Node.js** (v18+ recommended)
- **Git** repository to visualize

### Backend Setup

```bash
cd extensions/codex-viz-web/backend

# Build and run
cargo run

# Server starts on http://127.0.0.1:3001
```

### Frontend Setup

```bash
cd extensions/codex-viz-web/frontend

# Install dependencies
npm install

# Start development server
npm run dev

# Frontend starts on http://localhost:3000
```

### Access

Open your browser to **http://localhost:3000**

## 📖 Usage

### Basic Workflow

1. **Start Backend**: Run the Rust API server
2. **Start Frontend**: Launch the React app
3. **Select View Mode**:
   - 📊 **Commits**: 3D/4D commit history
   - 🔥 **Heatmap**: File change statistics
   - 🌿 **Branches**: Branch structure
   - 🌐 **All**: Combined view
4. **Specify Repository** (optional): Leave empty for current directory
5. **Interact**:
   - **Mouse drag**: Rotate view
   - **Scroll**: Zoom in/out
   - **Real-time panel**: Monitor live updates

### API Endpoints

```bash
# Get 3D commit history (last 1000 commits)
curl "http://localhost:3001/api/commits?limit=1000"

# Get file change heatmap
curl "http://localhost:3001/api/files/heatmap?limit=1000"

# Get branch structure
curl "http://localhost:3001/api/branches/graph"

# WebSocket for real-time updates
ws://localhost:3001/api/realtime
```

### Custom Repository Path

Add `?repo_path=/path/to/repo` to API requests:

```bash
curl "http://localhost:3001/api/commits?repo_path=/home/user/projects/myrepo"
```

Or use the frontend control panel to specify the path.

## 🏗️ Architecture

```
codex-viz-web/
├── backend/            # Rust API server
│   ├── src/
│   │   ├── api/        # REST endpoints
│   │   ├── git/        # Git analysis engine
│   │   ├── types.rs    # Data structures
│   │   └── main.rs     # Server entry
│   └── Cargo.toml
└── frontend/           # React + Three.js
    ├── src/
    │   ├── components/ # 3D visualization components
    │   ├── hooks/      # Data fetching hooks
    │   ├── lib/        # API client
    │   └── main.tsx    # App entry
    └── package.json
```

### Tech Stack

**Backend (Rust)**:
- axum (Web framework)
- git2 (Git operations)
- tokio-tungstenite (WebSocket)
- notify (File system watcher)
- serde_json (JSON serialization)

**Frontend (React + TypeScript)**:
- React 18
- Three.js + React Three Fiber
- @react-three/drei (Helpers)
- @react-three/postprocessing (Effects)
- TanStack Query (Data fetching)
- Vite (Build tool)

## 🎯 Performance Optimizations

### Large Repositories

The visualizer handles large repositories efficiently:

1. **Commit Limit**: Default 1000 commits (configurable)
2. **Instanced Rendering**: Three.js InstancedMesh for performance
3. **Level of Detail (LOD)**: Simplified rendering for distant objects
4. **Virtual Scrolling**: Efficient event list rendering
5. **Debounced File Watching**: Prevents event flooding

### Recommended Limits

| Repository Size | Commit Limit | Expected Performance |
|----------------|--------------|---------------------|
| Small (<1000)  | 1000         | Instant |
| Medium (1K-10K)| 1000-5000    | 1-3 seconds |
| Large (>10K)   | 5000         | 3-10 seconds |

## 🔧 Configuration

### Backend

Edit `backend/Cargo.toml` to adjust dependencies.

Server port: Change `127.0.0.1:3001` in `main.rs`.

### Frontend

Edit `frontend/vite.config.ts` to change port or proxy settings.

API base URL: Modify `frontend/src/lib/api.ts`.

## 🐛 Troubleshooting

### Backend fails to start

```bash
# Check Rust installation
cargo --version

# Clean and rebuild
cargo clean
cargo build
```

### Frontend build errors

```bash
# Clear node_modules
rm -rf node_modules
npm install
```

### WebSocket connection fails

- Ensure backend is running on port 3001
- Check firewall settings
- Verify CORS configuration in `main.rs`

### No commits displayed

- Ensure you're in a Git repository
- Check repository path in control panel
- Verify backend logs for errors

## 🚀 Future Enhancements

- [ ] **Performance Profiling**: Integrated Chrome DevTools
- [ ] **Export Features**: Screenshots, video recording, data export
- [ ] **Advanced Filters**: Date range, author, file type
- [ ] **Collaborative Mode**: Multi-user visualization
- [ ] **Plugin System**: Custom visualization extensions
- [ ] **Mobile Support**: Touch controls and responsive design

## 📝 License

Same as parent project (Apache 2.0)

## 🙏 Acknowledgments

- Inspired by [Kamui4d](https://github.com/kamui4d/kamui4d)
- Built on [Gource](https://gource.io/) concepts
- Powered by [React Three Fiber](https://docs.pmnd.rs/react-three-fiber/)

## 📧 Support

For issues and questions, see the main Codex project README.

---

**Version**: 0.1.0  
**Status**: 🚧 Alpha (Functional but under development)

