import React, { useState } from 'react';
import './CodexTaskManager.css';

function CodexTaskManager({ onExecute }) {
  const [prompt, setPrompt] = useState('');
  const [options, setOptions] = useState({
    mode: 'interactive', // Default mode
    model: 'o4-mini',    // Default model
  });
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState(''); // For general errors (network, API issues)
  const [stdoutContent, setStdoutContent] = useState('');
  const [stderrContent, setStderrContent] = useState('');
  const [userActionMessage, setUserActionMessage] = useState(''); // For Approve/Reject feedback

  const handleOptionChange = (e) => {
    const { name, value } = e.target;
    setOptions(prevOptions => ({
      ...prevOptions,
      [name]: value,
    }));
  };

  const handleSubmit = async (e) => {
    e.preventDefault();
    setIsLoading(true);
    setError('');
    setStdoutContent('');
    setStderrContent('');
    setUserActionMessage(''); // Clear previous action messages

    try {
      const result = await onExecute(prompt, options);
      if (result.status === "success") {
        setStdoutContent(result.stdout || '');
        setStderrContent(result.stderr || '');
      } else if (result.status === "error") {
        setError(result.message || 'Codex execution reported an error.');
        setStdoutContent(result.stdout || '');
        setStderrContent(result.stderr || '');
      } else {
        setError('Received an unexpected response structure from the server.');
      }
    } catch (err) {
      setError(err.message || 'An unexpected error occurred during execution.');
      setStdoutContent('');
      setStderrContent('');
    } finally {
      setIsLoading(false);
    }
  };

  const handleApprove = () => {
    console.log("Approve button clicked");
    setUserActionMessage("Changes approved (mock action).");
    // Future: Send approval to backend or trigger next step
  };

  const handleReject = () => {
    console.log("Reject button clicked");
    setUserActionMessage("Changes rejected (mock action).");
    // Future: Handle rejection, perhaps clear output or send feedback
  };

  // Basic heuristic to check if stdout might contain a diff
  const mightContainDiff = stdoutContent.includes('--- a/') && stdoutContent.includes('+++ b/');

  return (
    <div className="codex-task-manager">
      <h2>Execute Codex Task</h2>
      <form onSubmit={handleSubmit} className="codex-form">
        {/* Form elements for prompt and options - unchanged */}
        <div className="form-group">
          <label htmlFor="prompt">Prompt:</label>
          <textarea
            id="prompt"
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder="Enter your natural language prompt here..."
            rows={5}
            required
          />
        </div>
        <div className="options-group">
          <div className="form-group">
            <label htmlFor="mode">Mode:</label>
            <select id="mode" name="mode" value={options.mode} onChange={handleOptionChange}>
              <option value="interactive">Interactive (suggest)</option>
              <option value="auto-edit">Auto-Edit</option>
              <option value="full-auto">Full-Auto</option>
            </select>
          </div>
          <div className="form-group">
            <label htmlFor="model">Model:</label>
            <input type="text" id="model" name="model" value={options.model} onChange={handleOptionChange} placeholder="e.g., o4-mini" />
          </div>
        </div>
        <button type="submit" disabled={isLoading} className="execute-button">
          {isLoading ? 'Executing...' : 'Execute Codex'}
        </button>
      </form>

      {isLoading && <p className="loading-message">Executing Codex Task...</p>}
      
      {error && (
        <div className="general-error-container">
          <h4>Execution Error:</h4>
          <pre className="general-error-display">{error}</pre>
        </div>
      )}

      {stdoutContent && (
        <div className="stdout-container">
          <h4>Standard Output:</h4>
          <pre className="stdout-display">{stdoutContent}</pre>
        </div>
      )}

      {stderrContent && (
        <div className="stderr-container">
          <h4>Standard Error:</h4>
          <pre className="stderr-display">{stderrContent}</pre>
        </div>
      )}

      {/* User Actions Section - Approve/Reject and Diff Placeholder */}
      {stdoutContent && !isLoading && ( // Only show if there's stdout and not loading
        <div className="user-actions-container">
          {mightContainDiff && (
            <div className="diff-placeholder">
              <p><em>Diff display will appear here.</em></p>
            </div>
          )}
          <div className="action-buttons">
            <button onClick={handleApprove} className="approve-button">Approve</button>
            <button onClick={handleReject} className="reject-button">Reject</button>
          </div>
          {userActionMessage && <p className="user-action-message">{userActionMessage}</p>}
        </div>
      )}
      
      {!isLoading && !error && !stdoutContent && !stderrContent && (
         <p className="no-output-message">Submit a prompt to see output.</p>
      )}
    </div>
  );
}

export default CodexTaskManager;
