import { BrowserRouter as Router, Routes, Route, Link } from "react-router-dom";
import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import Dashboard from "./pages/Dashboard";
import Settings from "./pages/Settings";
import Blueprints from "./pages/Blueprints";
import GitVR from "./pages/GitVR";
import Orchestration from "./pages/Orchestration";
import "./App.css";

function App() {
  useEffect(() => {
    // Listen for navigation events from tray
    const unlisten = listen<string>("navigate", (event) => {
      const path = event.payload;
      window.location.hash = path;
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <Router>
      <div className="app-container">
        <nav className="sidebar">
          <div className="logo">
            <h2>Codex AI</h2>
            <p className="version">v1.4.0</p>
          </div>
          
          <div className="nav-links">
            <Link to="/" className="nav-link">
              📊 Dashboard
            </Link>
            <Link to="/git-viz" className="nav-link">
              🌐 Git Visualization
            </Link>
            <Link to="/orchestration" className="nav-link">
              🎭 Orchestration
            </Link>
            <Link to="/blueprints" className="nav-link">
              📋 Blueprints
            </Link>
            <Link to="/settings" className="nav-link">
              ⚙️ Settings
            </Link>
          </div>
          
          <div className="nav-footer">
            <p>AI Native OS</p>
          </div>
        </nav>

        <main className="main-content">
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/git-viz" element={<GitVR />} />
            <Route path="/orchestration" element={<Orchestration />} />
            <Route path="/blueprints" element={<Blueprints />} />
            <Route path="/settings" element={<Settings />} />
          </Routes>
        </main>
      </div>
    </Router>
  );
}

export default App;
