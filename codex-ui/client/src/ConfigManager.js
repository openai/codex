import React, { useState, useEffect } from 'react';
import './ConfigManager.css';

function ConfigManager({ currentApiKey, currentProjectDirectory, onApiKeyChange, onProjectDirChange }) {
  const [inputApiKey, setInputApiKey] = useState('');
  const [inputProjectDirectory, setInputProjectDirectory] = useState('');
  const [loadingKey, setLoadingKey] = useState(false);
  const [loadingDir, setLoadingDir] = useState(false);
  const [errorKey, setErrorKey] = useState('');
  const [errorDir, setErrorDir] = useState('');
  const [successKey, setSuccessKey] = useState('');
  const [successDir, setSuccessDir] = useState('');

  useEffect(() => {
    if (currentApiKey) {
      setInputApiKey(currentApiKey);
    }
    if (currentProjectDirectory) {
      setInputProjectDirectory(currentProjectDirectory);
    }
  }, [currentApiKey, currentProjectDirectory]);

  const handleApiKeySubmit = async (e) => {
    e.preventDefault();
    setLoadingKey(true);
    setErrorKey('');
    setSuccessKey('');
    try {
      const response = await fetch('/api/config/apikey', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ apiKey: inputApiKey }),
      });
      const data = await response.json();
      if (!response.ok) {
        throw new Error(data.error || 'Failed to save API key');
      }
      onApiKeyChange(inputApiKey);
      setSuccessKey('API Key saved successfully!');
    } catch (err) {
      setErrorKey(err.message);
    } finally {
      setLoadingKey(false);
    }
  };

  const handleProjectDirSubmit = async (e) => {
    e.preventDefault();
    setLoadingDir(true);
    setErrorDir('');
    setSuccessDir('');
    try {
      const response = await fetch('/api/config/projectdir', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ projectDirectory: inputProjectDirectory }),
      });
      const data = await response.json();
      if (!response.ok) {
        throw new Error(data.error || 'Failed to save project directory');
      }
      onProjectDirChange(inputProjectDirectory);
      setSuccessDir('Project Directory saved successfully!');
    } catch (err) {
      setErrorDir(err.message);
    } finally {
      setLoadingDir(false);
    }
  };

  return (
    <div className="config-manager">
      <h2>Configuration</h2>
      <form onSubmit={handleApiKeySubmit} className="config-form">
        <label htmlFor="apiKey">API Key:</label>
        <input
          type="password" // Use password type for API keys
          id="apiKey"
          value={inputApiKey}
          onChange={(e) => setInputApiKey(e.target.value)}
          placeholder="Enter your API Key"
        />
        <button type="submit" disabled={loadingKey}>
          {loadingKey ? 'Saving...' : 'Save API Key'}
        </button>
        {errorKey && <p className="error-message">{errorKey}</p>}
        {successKey && <p className="success-message">{successKey}</p>}
      </form>

      <form onSubmit={handleProjectDirSubmit} className="config-form">
        <label htmlFor="projectDirectory">Project Directory:</label>
        <input
          type="text"
          id="projectDirectory"
          value={inputProjectDirectory}
          onChange={(e) => setInputProjectDirectory(e.target.value)}
          placeholder="Enter project directory path"
        />
        <button type="submit" disabled={loadingDir}>
          {loadingDir ? 'Saving...' : 'Save Project Directory'}
        </button>
        {errorDir && <p className="error-message">{errorDir}</p>}
        {successDir && <p className="success-message">{successDir}</p>}
      </form>
    </div>
  );
}

export default ConfigManager;
