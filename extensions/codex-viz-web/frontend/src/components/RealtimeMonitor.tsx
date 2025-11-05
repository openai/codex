import { useEffect, useState, useCallback } from 'react'
import { createWebSocket } from '../lib/api'
import type { RealtimeEvent } from '../types'
import './RealtimeMonitor.css'

interface RealtimeMonitorProps {
  repoPath?: string
}

export default function RealtimeMonitor({ repoPath }: RealtimeMonitorProps) {
  const [events, setEvents] = useState<RealtimeEvent[]>([])
  const [isConnected, setIsConnected] = useState(false)

  const addEvent = useCallback((event: RealtimeEvent) => {
    setEvents((prev) => [event, ...prev].slice(0, 10)) // Keep last 10 events
  }, [])

  useEffect(() => {
    let ws: WebSocket | null = null
    let reconnectTimeout: NodeJS.Timeout

    const connect = () => {
      try {
        ws = createWebSocket(repoPath)

        ws.onopen = () => {
          console.log('🔌 WebSocket connected')
          setIsConnected(true)
        }

        ws.onmessage = (event) => {
          try {
            const data = JSON.parse(event.data)
            if (data.type && data.type !== 'connected') {
              addEvent(data as RealtimeEvent)
            }
          } catch (error) {
            console.error('Failed to parse WebSocket message:', error)
          }
        }

        ws.onerror = (error) => {
          console.error('WebSocket error:', error)
        }

        ws.onclose = () => {
          console.log('🔌 WebSocket disconnected')
          setIsConnected(false)

          // Attempt to reconnect after 5 seconds
          reconnectTimeout = setTimeout(() => {
            console.log('🔄 Attempting to reconnect...')
            connect()
          }, 5000)
        }
      } catch (error) {
        console.error('Failed to create WebSocket:', error)
        setIsConnected(false)
      }
    }

    connect()

    return () => {
      if (ws) {
        ws.close()
      }
      if (reconnectTimeout) {
        clearTimeout(reconnectTimeout)
      }
    }
  }, [repoPath, addEvent])

  if (events.length === 0 && !isConnected) {
    return null
  }

  return (
    <div className="realtime-monitor">
      <div className="monitor-header">
        <span className={`status-indicator ${isConnected ? 'connected' : 'disconnected'}`} />
        <span className="monitor-title">Real-time Updates</span>
      </div>

      <div className="event-list">
        {events.map((event, index) => (
          <div key={index} className="event-item">
            {event.type === 'new_commit' && (
              <>
                <span className="event-icon">📝</span>
                <span className="event-text">
                  New commit: {event.commit.message.substring(0, 40)}...
                </span>
              </>
            )}
            {event.type === 'file_changed' && (
              <>
                <span className="event-icon">📄</span>
                <span className="event-text">
                  {event.change_type}: {event.path}
                </span>
              </>
            )}
            {event.type === 'branch_created' && (
              <>
                <span className="event-icon">🌿</span>
                <span className="event-text">
                  Branch created: {event.branch.name}
                </span>
              </>
            )}
            {event.type === 'branch_deleted' && (
              <>
                <span className="event-icon">🗑️</span>
                <span className="event-text">
                  Branch deleted: {event.branch_name}
                </span>
              </>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}

