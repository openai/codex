import React, { useState, useEffect } from 'react';
import './AgentsMdManager.css';

function AgentsMdManager({ initialContent, onSave, filePath }) {
  const [editableContent, setEditableContent] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState('');
  const [successMessage, setSuccessMessage] = useState('');

  useEffect(() => {
    setEditableContent(initialContent);
  }, [initialContent]);

  const handleContentChange = (event) => {
    setEditableContent(event.target.value);
  };

  const handleSave = async () => {
    setIsLoading(true);
    setError('');
    setSuccessMessage('');
    try {
      // onSave is expected to be an async function that returns a promise
      // The promise should resolve if the save was successful, or reject with an error.
      await onSave(filePath, editableContent);
      setSuccessMessage('AGENTS.md saved successfully!');
    } catch (err) {
      setError(err.message || 'Failed to save AGENTS.md. Please check console for details.');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="agents-md-manager">
      <h2>Edit AGENTS.md ({filePath})</h2>
      <textarea
        value={editableContent}
        onChange={handleContentChange}
        placeholder="Enter content for AGENTS.md"
        rows={15} // Reasonable default size
      />
      <button onClick={handleSave} disabled={isLoading}>
        {isLoading ? 'Saving...' : 'Save AGENTS.md'}
      </button>
      {error && <p className="error-message">{error}</p>}
      {successMessage && <p className="success-message">{successMessage}</p>}
    </div>
  );
}

export default AgentsMdManager;
