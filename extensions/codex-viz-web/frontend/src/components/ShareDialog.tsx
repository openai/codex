import { useState } from 'react'
import axios from 'axios'
import './ShareDialog.css'

interface ShareDialogProps {
  isOpen: boolean
  onClose: () => void
  viewState: {
    repo_path: string
    view_mode: string
    filters: any
    camera_position: [number, number, number]
  }
}

export default function ShareDialog({ isOpen, onClose, viewState }: ShareDialogProps) {
  const [isLoading, setIsLoading] = useState(false)
  const [shareUrl, setShareUrl] = useState('')
  const [copied, setCopied] = useState(false)

  const handleShare = async () => {
    setIsLoading(true)

    try {
      const response = await axios.post('/api/views/share', {
        created_by: 'User',
        ...viewState,
      })

      const viewId = response.data.id
      const url = `${window.location.origin}?view=${viewId}`
      setShareUrl(url)
    } catch (error) {
      console.error('Failed to create share link:', error)
    } finally {
      setIsLoading(false)
    }
  }

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(shareUrl)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch (error) {
      console.error('Failed to copy:', error)
    }
  }

  if (!isOpen) return null

  return (
    <div className="share-overlay" onClick={onClose}>
      <div className="share-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="share-header">
          <h3 className="share-title">🔗 Share This View</h3>
          <button className="close-button" onClick={onClose}>
            ✕
          </button>
        </div>

        {!shareUrl ? (
          <div className="share-content">
            <p className="share-description">
              Create a shareable link to this visualization. Others can view the
              same commits, filters, and camera angle.
            </p>

            <div className="share-info">
              <div className="info-item">
                <span className="info-label">View Mode:</span>
                <span className="info-value">{viewState.view_mode}</span>
              </div>
              <div className="info-item">
                <span className="info-label">Repository:</span>
                <span className="info-value">
                  {viewState.repo_path || 'Current directory'}
                </span>
              </div>
            </div>

            <button
              className="create-link-button"
              onClick={handleShare}
              disabled={isLoading}
            >
              {isLoading ? 'Creating...' : 'Create Share Link'}
            </button>
          </div>
        ) : (
          <div className="share-content">
            <p className="share-success">✅ Share link created!</p>

            <div className="share-url-container">
              <input
                type="text"
                className="share-url-input"
                value={shareUrl}
                readOnly
              />
              <button
                className="copy-button"
                onClick={handleCopy}
              >
                {copied ? '✓ Copied!' : '📋 Copy'}
              </button>
            </div>

            <p className="share-hint">
              Anyone with this link can view this visualization
            </p>
          </div>
        )}
      </div>
    </div>
  )
}

