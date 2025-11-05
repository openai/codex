import { useEffect, useRef, useState, useCallback } from 'react'
import type { WorkerMessage, WorkerResponse } from '../workers/gitAnalyzer.worker'

export function useWebWorker<T = unknown>() {
  const workerRef = useRef<Worker | null>(null)
  const [isProcessing, setIsProcessing] = useState(false)
  const [progress, setProgress] = useState(0)
  const [error, setError] = useState<string | null>(null)

  // Initialize worker
  useEffect(() => {
    try {
      workerRef.current = new Worker(
        new URL('../workers/gitAnalyzer.worker.ts', import.meta.url),
        { type: 'module' }
      )

      workerRef.current.onerror = (event: ErrorEvent) => {
        console.error('Worker error:', event)
        setError('Worker initialization failed')
        setIsProcessing(false)
      }
    } catch (err) {
      console.error('Failed to create worker:', err)
      setError('Worker creation failed')
    }

    return () => {
      workerRef.current?.terminate()
    }
  }, [])

  // Send message to worker
  const postMessage = useCallback(
    (message: WorkerMessage): Promise<T> => {
      return new Promise((resolve, reject) => {
        if (!workerRef.current) {
          reject(new Error('Worker not initialized'))
          return
        }

        setIsProcessing(true)
        setProgress(0)
        setError(null)

        const handleMessage = (event: MessageEvent<WorkerResponse>) => {
          const response = event.data

          switch (response.type) {
            case 'progress':
              setProgress(response.progress)
              break

            case 'error':
              setError(response.error)
              setIsProcessing(false)
              reject(new Error(response.error))
              workerRef.current?.removeEventListener('message', handleMessage)
              break

            default:
              // Success response
              setIsProcessing(false)
              setProgress(100)
              resolve(response.data as T)
              workerRef.current?.removeEventListener('message', handleMessage)
              break
          }
        }

        workerRef.current.addEventListener('message', handleMessage)
        workerRef.current.postMessage(message)

        // Timeout after 30 seconds
        setTimeout(() => {
          if (isProcessing) {
            setError('Worker timeout')
            setIsProcessing(false)
            reject(new Error('Worker timeout'))
          }
        }, 30000)
      })
    },
    [isProcessing]
  )

  return {
    postMessage,
    isProcessing,
    progress,
    error,
  }
}

