import { useState, useEffect, useCallback } from 'react'

export interface Bookmark {
  id: string
  sha: string
  message: string
  timestamp: string
  createdAt: string
  notes?: string
}

const STORAGE_KEY = 'codex-viz-bookmarks'

export function useBookmarks() {
  const [bookmarks, setBookmarks] = useState<Bookmark[]>([])

  // Load bookmarks from localStorage
  useEffect(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY)
      if (stored) {
        const parsed = JSON.parse(stored)
        setBookmarks(parsed)
      }
    } catch (error) {
      console.error('Failed to load bookmarks:', error)
    }
  }, [])

  // Save bookmarks to localStorage
  const saveBookmarks = useCallback((newBookmarks: Bookmark[]) => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(newBookmarks))
      setBookmarks(newBookmarks)
    } catch (error) {
      console.error('Failed to save bookmarks:', error)
    }
  }, [])

  const addBookmark = useCallback(
    (bookmark: Omit<Bookmark, 'id' | 'createdAt'>) => {
      const newBookmark: Bookmark = {
        ...bookmark,
        id: `${bookmark.sha}-${Date.now()}`,
        createdAt: new Date().toISOString(),
      }

      // Check if already bookmarked
      if (bookmarks.find((b) => b.sha === bookmark.sha)) {
        return false
      }

      saveBookmarks([...bookmarks, newBookmark])
      return true
    },
    [bookmarks, saveBookmarks]
  )

  const removeBookmark = useCallback(
    (id: string) => {
      saveBookmarks(bookmarks.filter((b) => b.id !== id))
    },
    [bookmarks, saveBookmarks]
  )

  const updateBookmark = useCallback(
    (id: string, updates: Partial<Bookmark>) => {
      saveBookmarks(
        bookmarks.map((b) => (b.id === id ? { ...b, ...updates } : b))
      )
    },
    [bookmarks, saveBookmarks]
  )

  const isBookmarked = useCallback(
    (sha: string) => {
      return bookmarks.some((b) => b.sha === sha)
    },
    [bookmarks]
  )

  const clearBookmarks = useCallback(() => {
    saveBookmarks([])
  }, [saveBookmarks])

  return {
    bookmarks,
    addBookmark,
    removeBookmark,
    updateBookmark,
    isBookmarked,
    clearBookmarks,
  }
}

