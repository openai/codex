/**
 * Git Worker Hook
 * 
 * Manages Git parsing Web Worker
 */

import { useState, useEffect, useRef, useCallback } from 'react'

interface Commit3D {
  sha: string
  message: string
  author: string
  author_email: string
  timestamp: string
  branch: string
  parents: string[]
  x: number
  y: number
  z: number
  color: string
}

interface ParseStats {
  totalCommits: number
  uniqueAuthors: number
  branches: number
  processingTime: number
}

export function useGitWorker() {
  const [processing, setProcessing] = useState(false)
  const [stats, setStats] = useState<ParseStats | null>(null)
  const workerRef = useRef<Worker | null>(null)

  useEffect(() => {
    // Initialize worker
    workerRef.current = new Worker(
      new URL('../../workers/git-parser.worker.ts', import.meta.url),
      { type: 'module' }
    )

    return () => {
      if (workerRef.current) {
        workerRef.current.terminate()
      }
    }
  }, [])

  const parseCommits = useCallback(
    (commits: Commit3D[]): Promise<{ commits: Commit3D[]; stats: ParseStats }> => {
      return new Promise((resolve, reject) => {
        if (!workerRef.current) {
          reject(new Error('Worker not initialized'))
          return
        }

        setProcessing(true)

        const handleMessage = (event: MessageEvent) => {
          if (event.data.type === 'parsed') {
            setStats(event.data.stats)
            setProcessing(false)
            resolve({
              commits: event.data.commits,
              stats: event.data.stats,
            })
            workerRef.current?.removeEventListener('message', handleMessage)
          }
        }

        const handleError = (error: ErrorEvent) => {
          setProcessing(false)
          reject(error)
          workerRef.current?.removeEventListener('error', handleError)
        }

        workerRef.current.addEventListener('message', handleMessage)
        workerRef.current.addEventListener('error', handleError)

        workerRef.current.postMessage({
          type: 'parse',
          commits,
        })
      })
    },
    []
  )

  return {
    parseCommits,
    processing,
    stats,
  }
}
