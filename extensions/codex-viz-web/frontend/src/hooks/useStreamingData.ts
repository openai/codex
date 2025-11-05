import { useState, useEffect, useCallback, useRef } from 'react'

interface StreamingOptions {
  chunkSize?: number
  onProgress?: (loaded: number, total: number) => void
  onError?: (error: Error) => void
}

/**
 * Hook for streaming large datasets in chunks
 * Prevents memory overflow and UI freezing
 */
export function useStreamingData<T>(
  url: string,
  options: StreamingOptions = {}
) {
  const { chunkSize = 100, onProgress, onError } = options

  const [data, setData] = useState<T[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [progress, setProgress] = useState(0)
  const [error, setError] = useState<Error | null>(null)
  const abortControllerRef = useRef<AbortController | null>(null)

  const loadData = useCallback(async () => {
    setIsLoading(true)
    setError(null)
    setProgress(0)
    setData([])

    // Create abort controller for cancellation
    abortControllerRef.current = new AbortController()

    try {
      const response = await fetch(url, {
        signal: abortControllerRef.current.signal,
      })

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`)
      }

      const contentLength = response.headers.get('content-length')
      const total = contentLength ? parseInt(contentLength, 10) : 0

      const reader = response.body?.getReader()
      if (!reader) {
        throw new Error('ReadableStream not supported')
      }

      const decoder = new TextDecoder()
      let buffer = ''
      let loaded = 0
      let chunk: T[] = []

      while (true) {
        const { done, value } = await reader.read()

        if (done) break

        loaded += value.byteLength

        // Update progress
        if (total > 0) {
          const progressPercent = (loaded / total) * 100
          setProgress(progressPercent)
          onProgress?.(loaded, total)
        }

        // Decode chunk
        buffer += decoder.decode(value, { stream: true })

        // Parse JSON lines
        const lines = buffer.split('\n')
        buffer = lines.pop() || '' // Keep incomplete line in buffer

        for (const line of lines) {
          if (line.trim()) {
            try {
              const item = JSON.parse(line) as T
              chunk.push(item)

              // Emit chunk when size reached
              if (chunk.length >= chunkSize) {
                setData((prev) => [...prev, ...chunk])
                chunk = []
                // Allow UI to update
                await new Promise((resolve) => setTimeout(resolve, 0))
              }
            } catch (parseError) {
              console.warn('Failed to parse line:', line, parseError)
            }
          }
        }
      }

      // Add remaining items
      if (chunk.length > 0) {
        setData((prev) => [...prev, ...chunk])
      }

      setProgress(100)
    } catch (err) {
      if (err instanceof Error) {
        if (err.name !== 'AbortError') {
          setError(err)
          onError?.(err)
        }
      }
    } finally {
      setIsLoading(false)
    }
  }, [url, chunkSize, onProgress, onError])

  // Cancel loading
  const cancel = useCallback(() => {
    abortControllerRef.current?.abort()
    setIsLoading(false)
  }, [])

  // Auto-load on mount
  useEffect(() => {
    loadData()

    return () => {
      abortControllerRef.current?.abort()
    }
  }, [loadData])

  return {
    data,
    isLoading,
    progress,
    error,
    reload: loadData,
    cancel,
  }
}

/**
 * Hook for paginated data loading
 * Alternative to streaming for backends that don't support streaming
 */
export function usePaginatedData<T>(
  baseUrl: string,
  pageSize: number = 100
) {
  const [data, setData] = useState<T[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [hasMore, setHasMore] = useState(true)
  const [page, setPage] = useState(0)
  const [error, setError] = useState<Error | null>(null)

  const loadMore = useCallback(async () => {
    if (isLoading || !hasMore) return

    setIsLoading(true)
    setError(null)

    try {
      const url = `${baseUrl}?page=${page}&limit=${pageSize}`
      const response = await fetch(url)

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`)
      }

      const result = await response.json()
      const items = result.data || result

      if (items.length === 0 || items.length < pageSize) {
        setHasMore(false)
      }

      setData((prev) => [...prev, ...items])
      setPage((prev) => prev + 1)
    } catch (err) {
      if (err instanceof Error) {
        setError(err)
      }
    } finally {
      setIsLoading(false)
    }
  }, [baseUrl, page, pageSize, isLoading, hasMore])

  // Load first page on mount
  useEffect(() => {
    loadMore()
  }, []) // Only on mount

  const reset = useCallback(() => {
    setData([])
    setPage(0)
    setHasMore(true)
    setError(null)
  }, [])

  return {
    data,
    isLoading,
    hasMore,
    error,
    loadMore,
    reset,
  }
}

