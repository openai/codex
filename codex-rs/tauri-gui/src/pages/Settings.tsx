import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "../styles/Settings.css";

function Settings() {
  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [theme, setTheme] = useState<"light" | "dark" | "system">("system");
  const [notificationsEnabled, setNotificationsEnabled] = useState(true);
  const [soundEnabled, setSoundEnabled] = useState(false);

  const handleAutostartToggle = async () => {
    try {
      await invoke("enable_autostart", { enabled: !autostartEnabled });
      setAutostartEnabled(!autostartEnabled);
      alert("Autostart setting updated successfully!");
    } catch (error) {
      console.error("Failed to update autostart:", error);
      alert(`Failed to update autostart: ${error}`);
    }
  };

  const handleSaveSettings = () => {
    // Save settings to local storage or backend
    localStorage.setItem("theme", theme);
    localStorage.setItem("notificationsEnabled", String(notificationsEnabled));
    localStorage.setItem("soundEnabled", String(soundEnabled));
    alert("Settings saved successfully!");
  };

  return (
    <div className="settings">
      <h1>Settings</h1>

      <div className="settings-section">
        <h2>General</h2>
        
        <div className="setting-item">
          <div className="setting-info">
            <label>Auto-start on Windows boot</label>
            <p className="setting-description">
              Launch Codex automatically when Windows starts
            </p>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={autostartEnabled}
              onChange={handleAutostartToggle}
            />
            <span className="toggle-slider"></span>
          </label>
        </div>

        <div className="setting-item">
          <div className="setting-info">
            <label>Theme</label>
            <p className="setting-description">
              Choose your preferred color scheme
            </p>
          </div>
          <select
            value={theme}
            onChange={(e) => setTheme(e.target.value as any)}
            className="select-field"
          >
            <option value="light">Light</option>
            <option value="dark">Dark</option>
            <option value="system">System</option>
          </select>
        </div>
      </div>

      <div className="settings-section">
        <h2>Notifications</h2>
        
        <div className="setting-item">
          <div className="setting-info">
            <label>Enable desktop notifications</label>
            <p className="setting-description">
              Show notifications for file changes and blueprint events
            </p>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={notificationsEnabled}
              onChange={(e) => setNotificationsEnabled(e.target.checked)}
            />
            <span className="toggle-slider"></span>
          </label>
        </div>

        <div className="setting-item">
          <div className="setting-info">
            <label>Enable notification sound</label>
            <p className="setting-description">
              Play sound effect with notifications
            </p>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={soundEnabled}
              onChange={(e) => setSoundEnabled(e.target.checked)}
            />
            <span className="toggle-slider"></span>
          </label>
        </div>
      </div>

      <div className="settings-section">
        <h2>File Monitoring</h2>
        
        <div className="setting-item">
          <div className="setting-info">
            <label>Monitored file extensions</label>
            <p className="setting-description">
              File types to monitor for changes (comma-separated)
            </p>
          </div>
          <input
            type="text"
            defaultValue=".rs,.ts,.tsx,.js,.jsx,.py,.md,.toml,.json,.yaml,.yml"
            className="input-field"
          />
        </div>

        <div className="setting-item">
          <div className="setting-info">
            <label>Exclude patterns (.gitignore style)</label>
            <p className="setting-description">
              Patterns to exclude from monitoring
            </p>
          </div>
          <textarea
            defaultValue="node_modules/&#10;target/&#10;.git/&#10;dist/"
            className="textarea-field"
            rows={4}
          />
        </div>
      </div>

      <div className="settings-actions">
        <button onClick={handleSaveSettings} className="btn btn-primary">
          Save Settings
        </button>
        <button className="btn btn-secondary">
          Reset to Defaults
        </button>
      </div>

      <div className="settings-section">
        <h2>About</h2>
        <div className="about-info">
          <p><strong>Codex AI-Native OS</strong></p>
          <p>Version: 0.1.0</p>
          <p>Built with Tauri + React + Rust</p>
          <p className="copyright">© 2025 zapabob. All rights reserved.</p>
        </div>
      </div>
    </div>
  );
}

export default Settings;

