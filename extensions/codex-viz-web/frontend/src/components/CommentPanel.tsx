import { useState, useEffect } from 'react'
import axios from 'axios'
import './CommentPanel.css'

interface Comment {
  id: string
  commit_sha: string
  author: string
  content: string
  created_at: string
  updated_at: string
}

interface CommentPanelProps {
  commitSha: string | null
  onClose: () => void
}

export default function CommentPanel({ commitSha, onClose }: CommentPanelProps) {
  const [comments, setComments] = useState<Comment[]>([])
  const [newComment, setNewComment] = useState('')
  const [author, setAuthor] = useState('Anonymous')
  const [isLoading, setIsLoading] = useState(false)

  // Load comments
  useEffect(() => {
    if (!commitSha) return

    const loadComments = async () => {
      try {
        const response = await axios.get(`/api/comments/${commitSha}`)
        setComments(response.data)
      } catch (error) {
        console.error('Failed to load comments:', error)
      }
    }

    loadComments()
  }, [commitSha])

  // Add comment
  const handleAddComment = async () => {
    if (!commitSha || !newComment.trim()) return

    setIsLoading(true)

    try {
      const response = await axios.post(`/api/comments/${commitSha}`, {
        author,
        content: newComment,
      })

      setComments((prev) => [...prev, response.data])
      setNewComment('')
    } catch (error) {
      console.error('Failed to add comment:', error)
    } finally {
      setIsLoading(false)
    }
  }

  // Delete comment
  const handleDeleteComment = async (commentId: string) => {
    try {
      await axios.delete(`/api/comments/${commentId}`)
      setComments((prev) => prev.filter((c) => c.id !== commentId))
    } catch (error) {
      console.error('Failed to delete comment:', error)
    }
  }

  if (!commitSha) return null

  return (
    <div className="comment-panel">
      <div className="comment-header">
        <h3 className="comment-title">
          💬 Comments for {commitSha.substring(0, 7)}
        </h3>
        <button className="close-button" onClick={onClose} aria-label="Close">
          ✕
        </button>
      </div>

      <div className="comments-list">
        {comments.length === 0 ? (
          <div className="empty-comments">
            <p>No comments yet. Be the first to comment!</p>
          </div>
        ) : (
          comments.map((comment) => (
            <div key={comment.id} className="comment-item">
              <div className="comment-meta">
                <span className="comment-author">{comment.author}</span>
                <span className="comment-date">
                  {new Date(comment.created_at).toLocaleString()}
                </span>
              </div>
              <p className="comment-content">{comment.content}</p>
              <button
                className="delete-comment"
                onClick={() => handleDeleteComment(comment.id)}
                aria-label="Delete comment"
              >
                Delete
              </button>
            </div>
          ))
        )}
      </div>

      <div className="comment-form">
        <input
          type="text"
          className="author-input"
          placeholder="Your name..."
          value={author}
          onChange={(e) => setAuthor(e.target.value)}
        />
        <textarea
          className="comment-textarea"
          placeholder="Add your comment..."
          value={newComment}
          onChange={(e) => setNewComment(e.target.value)}
          rows={3}
        />
        <button
          className="submit-comment"
          onClick={handleAddComment}
          disabled={isLoading || !newComment.trim()}
        >
          {isLoading ? 'Adding...' : 'Add Comment'}
        </button>
      </div>
    </div>
  )
}

