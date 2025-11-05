// ContextMenu.tsx - Cyberpunk-styled Context Menu
import { useEffect, useRef } from 'react'

export interface ContextMenuItem {
  label: string
  icon?: React.ReactNode
  onClick: () => void
  disabled?: boolean
  divider?: boolean
}

interface ContextMenuProps {
  x: number
  y: number
  items: ContextMenuItem[]
  onClose: () => void
}

export const ContextMenu = ({ x, y, items, onClose }: ContextMenuProps) => {
  const menuRef = useRef<HTMLDivElement>(null)
  
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        onClose()
      }
    }
    
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose()
      }
    }
    
    document.addEventListener('mousedown', handleClickOutside)
    document.addEventListener('keydown', handleEscape)
    
    return () => {
      document.removeEventListener('mousedown', handleClickOutside)
      document.removeEventListener('keydown', handleEscape)
    }
  }, [onClose])
  
  // Ensure menu stays within viewport
  const adjustedX = Math.min(x, window.innerWidth - 220)
  const adjustedY = Math.min(y, window.innerHeight - (items.length * 40 + 20))
  
  return (
    <div 
      ref={menuRef}
      className="cyberpunk-context-menu"
      style={{
        left: `${adjustedX}px`,
        top: `${adjustedY}px`,
      }}
    >
      {items.map((item, index) => (
        <div key={index}>
          {item.divider ? (
            <div className="cyberpunk-divider" style={{ margin: '0.5rem 0' }} />
          ) : (
            <button
              className={`cyberpunk-menu-item ${item.disabled ? 'disabled' : ''}`}
              onClick={() => {
                if (!item.disabled) {
                  item.onClick()
                  onClose()
                }
              }}
              disabled={item.disabled}
            >
              {item.icon && <span className="menu-icon">{item.icon}</span>}
              <span className="menu-label">{item.label}</span>
            </button>
          )}
        </div>
      ))}
    </div>
  )
}

// Hook for managing context menu state
export const useContextMenu = () => {
  const [contextMenu, setContextMenu] = useState<{
    x: number
    y: number
    items: ContextMenuItem[]
  } | null>(null)
  
  const showContextMenu = (x: number, y: number, items: ContextMenuItem[]) => {
    setContextMenu({ x, y, items })
  }
  
  const hideContextMenu = () => {
    setContextMenu(null)
  }
  
  const handleContextMenu = (e: React.MouseEvent, items: ContextMenuItem[]) => {
    e.preventDefault()
    showContextMenu(e.clientX, e.clientY, items)
  }
  
  return {
    contextMenu,
    showContextMenu,
    hideContextMenu,
    handleContextMenu,
  }
}

// Fix: Add missing import
import { useState } from 'react'

