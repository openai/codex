# Codex Viz Frontend

React + Three.js frontend for Kamui4d-style repository visualization.

## Features

- **3D Commit Graph** with React Three Fiber
- **File Change Heatmap** with instanced rendering
- **Branch Structure** visualization
- **Real-time Updates** via WebSocket
- **Interactive Controls** with OrbitControls

## Development

```bash
# Install dependencies
npm install

# Start development server
npm run dev

# Frontend will start on http://localhost:3000
```

## Usage

1. Ensure backend is running on port 3001
2. Open http://localhost:3000
3. Select view mode (Commits, Heatmap, Branches, or All)
4. Use mouse to rotate, scroll to zoom
5. Real-time updates will appear in bottom-right panel

## Tech Stack

- React 18
- Three.js
- React Three Fiber
- React Three Drei
- TanStack Query (React Query)
- Zustand
- Vite

