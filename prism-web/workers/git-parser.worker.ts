/**
 * Git Parser Web Worker
 * 
 * Parses Git data and calculates 3D coordinates off the main thread
 */

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

interface ParseRequest {
  type: 'parse'
  commits: Commit3D[]
}

interface ParseResponse {
  type: 'parsed'
  commits: Commit3D[]
  stats: {
    totalCommits: number
    uniqueAuthors: number
    branches: number
    processingTime: number
  }
}

// Worker message handler
self.onmessage = (event: MessageEvent<ParseRequest>) => {
  const startTime = performance.now()

  if (event.data.type === 'parse') {
    const commits = event.data.commits

    // Normalize Y coordinates
    const timestamps = commits.map((c) => new Date(c.timestamp).getTime())
    const minY = Math.min(...timestamps)
    const maxY = Math.max(...timestamps)
    const rangeY = maxY - minY || 1

    // Normalize X coordinates (branch positions)
    const branchMap = new Map<string, number>()
    commits.forEach((c) => {
      if (!branchMap.has(c.branch)) {
        branchMap.set(c.branch, branchMap.size * 10)
      }
    })

    // Process commits
    const normalizedCommits = commits.map((commit) => {
      const timestamp = new Date(commit.timestamp).getTime()
      const normalizedY = ((timestamp - minY) / rangeY) * 100

      return {
        ...commit,
        x: branchMap.get(commit.branch) || 0,
        y: normalizedY,
        z: commit.z, // Keep existing Z (depth)
      }
    })

    // Calculate statistics
    const uniqueAuthors = new Set(commits.map((c) => c.author_email)).size
    const branches = branchMap.size

    const endTime = performance.now()

    const response: ParseResponse = {
      type: 'parsed',
      commits: normalizedCommits,
      stats: {
        totalCommits: commits.length,
        uniqueAuthors,
        branches,
        processingTime: endTime - startTime,
      },
    }

    self.postMessage(response)
  }
}

export {}
