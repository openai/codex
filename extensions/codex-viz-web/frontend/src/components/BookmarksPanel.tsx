import { useState } from 'react'
import { useBookmarks } from '../hooks/useBookmarks'
import './BookmarksPanel.css'

interface BookmarksPanelProps {
  isOpen: boolean
  onClose: () => void
}

export default function BookmarksPanel({ isOpen, onClose }: BookmarksPanelProps) {
  const { bookmarks, removeBookmark, updateBookmark, clearBookmarks } = useBookmarks()
  const [editingId, setEditingId] = useState<string | null>(null)
  const [editNotes, setEditNotes] = useState('')

  if (!isOpen) return null

  const handleEdit = (bookmark: { id: string; notes?: string }) => {
    setEditingId(bookmark.id)
    setEditNotes(bookmark.notes || '')
  }

  const handleSave = (id: string) => {
    updateBookmark(id, { notes: editNotes })
    setEditingId(null)
    setEditNotes('')
  }

  const handleCancel = () => {
    setEditingId(null)
    setEditNotes('')
  }

  return (
    <div className="bookmarks-panel">
      <div className="bookmarks-header">
        <h2 className="bookmarks-title">
          🔖 Bookmarks ({bookmarks.length})
        </h2>
        <div className="bookmarks-actions">
          {bookmarks.length > 0 && (
            <button
              className="action-button danger"
              onClick={() => {
                if (confirm('Clear all bookmarks?')) {
                  clearBookmarks()
                }
              }}
              aria-label="Clear all bookmarks"
            >
              Clear All
            </button>
          )}
          <button
            className="action-button"
            onClick={onClose}
            aria-label="Close bookmarks"
          >
            ✕
          </button>
        </div>
      </div>

      <div className="bookmarks-list">
        {bookmarks.length === 0 ? (
          <div className="empty-state">
            <span className="empty-icon">🔖</span>
            <p className="empty-message">No bookmarks yet</p>
            <p className="empty-hint">
              Click on commits in the graph to bookmark them
            </p>
          </div>
        ) : (
          bookmarks.map((bookmark) => (
            <div key={bookmark.id} className="bookmark-item">
              <div className="bookmark-main">
                <div className="bookmark-header">
                  <span className="bookmark-sha">{bookmark.sha.substring(0, 7)}</span>
                  <span className="bookmark-date">
                    {new Date(bookmark.timestamp).toLocaleDateString()}
                  </span>
                </div>
                <p className="bookmark-message">{bookmark.message}</p>

                {editingId === bookmark.id ? (
                  <div className="bookmark-edit">
                    <textarea
                      className="edit-textarea"
                      value={editNotes}
                      onChange={(e) => setEditNotes(e.target.value)}
                      placeholder="Add your notes..."
                      rows={3}
                    />
                    <div className="edit-actions">
                      <button
                        className="edit-button save"
                        onClick={() => handleSave(bookmark.id)}
                      >
                        Save
                      </button>
                      <button
                        className="edit-button cancel"
                        onClick={handleCancel}
                      >
                        Cancel
                      </button>
                    </div>
                  </div>
                ) : (
                  <>
                    {bookmark.notes && (
                      <p className="bookmark-notes">{bookmark.notes}</p>
                    )}
                    <div className="bookmark-actions">
                      <button
                        className="action-button small"
                        onClick={() => handleEdit(bookmark)}
                      >
                        {bookmark.notes ? 'Edit' : 'Add'} Notes
                      </button>
                      <button
                        className="action-button small danger"
                        onClick={() => removeBookmark(bookmark.id)}
                      >
                        Remove
                      </button>
                    </div>
                  </>
                )}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  )
}

