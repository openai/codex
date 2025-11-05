import { BrowserRouter as Router, Routes, Route, Link } from "react-router-dom";
import { useEffect, useState, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { writeText, readText } from "@tauri-apps/api/clipboard";
import Dashboard from "./pages/Dashboard";
import Settings from "./pages/Settings";
import Blueprints from "./pages/Blueprints";
import GitVR from "./pages/GitVR";
import Orchestration from "./pages/Orchestration";
import { CyberpunkBackground } from "./components/CyberpunkBackground";
import "./App.css";
import "./styles/cyberpunk-theme.css";

function App() {
  const [selectedText, setSelectedText] = useState<string>("");
  const [notification, setNotification] = useState<string>("");
  
  // Clipboard operations
  const handleCopy = useCallback(async (text?: string) => {
    try {
      const textToCopy = text || selectedText || window.getSelection()?.toString() || "";
      if (textToCopy) {
        await writeText(textToCopy);
        setNotification("📋 Copied to clipboard");
        setTimeout(() => setNotification(""), 2000);
      }
    } catch (error) {
      console.error("Failed to copy:", error);
    }
  }, [selectedText]);
  
  const handlePaste = useCallback(async () => {
    try {
      const text = await readText();
      return text || "";
    } catch (error) {
      console.error("Failed to paste:", error);
      return "";
    }
  }, []);
  
  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = async (e: KeyboardEvent) => {
      // Ctrl+C / Cmd+C - Copy
      if ((e.ctrlKey || e.metaKey) && e.key === 'c') {
        const selection = window.getSelection()?.toString();
        if (selection) {
          e.preventDefault();
          await handleCopy(selection);
        }
      }
      
      // Ctrl+V / Cmd+V - Paste (handled by individual components)
      // Can be extended per component as needed
    };
    
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleCopy]);
  
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
      <CyberpunkBackground />
      {notification && (
        <div className="cyberpunk-notification">
          {notification}
        </div>
      )}
      <div className="app-container cyberpunk-container">
        <nav className="sidebar">
          <div className="logo">
            <h2>Codex AI</h2>
            <p className="version">v1.5.0</p>
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
              📋 Plans
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
