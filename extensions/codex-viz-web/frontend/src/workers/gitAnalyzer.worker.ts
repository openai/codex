/**
 * Git Analyzer Web Worker
 * Offloads heavy git data processing from main thread
 */

import type { Commit3D } from '../types'

// Worker message types
export type WorkerMessage =
  | { type: 'analyze'; data: Commit3D[] }
  | { type: 'normalize'; data: Commit3D[] }
  | { type: 'filter'; data: { commits: Commit3D[]; filter: FilterOptions } }

export type WorkerResponse =
  | { type: 'analyzed'; data: ProcessedData }
  | { type: 'normalized'; data: Commit3D[] }
  | { type: 'filtered'; data: Commit3D[] }
  | { type: 'error'; error: string }
  | { type: 'progress'; progress: number }

export interface FilterOptions {
  authors?: string[]
  branches?: string[]
  dateFrom?: string
  dateTo?: string
  searchText?: string
}

export interface ProcessedData {
  commits: Commit3D[]
  statistics: {
    totalCommits: number
    uniqueAuthors: number
    branches: string[]
    dateRange: { min: number; max: number }
  }
}

// Main worker logic
self.onmessage = (event: MessageEvent<WorkerMessage>) => {
  try {
    const message = event.data

    switch (message.type) {
      case 'analyze':
        analyzeCommits(message.data)
        break

      case 'normalize':
        normalizeCommits(message.data)
        break

      case 'filter':
        filterCommits(message.data.commits, message.data.filter)
        break

      default:
        postMessage({
          type: 'error',
          error: 'Unknown message type',
        } as WorkerResponse)
    }
  } catch (error) {
    postMessage({
      type: 'error',
      error: error instanceof Error ? error.message : 'Unknown error',
    } as WorkerResponse)
  }
}

/**
 * Analyze commits and extract statistics
 */
function analyzeCommits(commits: Commit3D[]): void {
  const total = commits.length
  let processed = 0

  // Extract unique authors
  const authors = new Set<string>()
  const branches = new Set<string>()
  let minDate = Infinity
  let maxDate = -Infinity

  for (const commit of commits) {
    authors.add(commit.author_email)
    branches.add(commit.branch)
    
    const timestamp = new Date(commit.timestamp).getTime()
    minDate = Math.min(minDate, timestamp)
    maxDate = Math.max(maxDate, timestamp)

    processed++
    if (processed % 100 === 0) {
      postMessage({
        type: 'progress',
        progress: (processed / total) * 100,
      } as WorkerResponse)
    }
  }

  postMessage({
    type: 'analyzed',
    data: {
      commits,
      statistics: {
        totalCommits: total,
        uniqueAuthors: authors.size,
        branches: Array.from(branches),
        dateRange: { min: minDate, max: maxDate },
      },
    },
  } as WorkerResponse)
}

/**
 * Normalize commit coordinates for visualization
 */
function normalizeCommits(commits: Commit3D[]): void {
  if (commits.length === 0) {
    postMessage({
      type: 'normalized',
      data: [],
    } as WorkerResponse)
    return
  }

  const minY = Math.min(...commits.map((c) => c.y))
  const maxY = Math.max(...commits.map((c) => c.y))
  const timeRange = maxY - minY || 1

  const normalized = commits.map((commit) => ({
    ...commit,
    normalizedY: ((commit.y - minY) / timeRange) * 100,
  }))

  postMessage({
    type: 'normalized',
    data: normalized,
  } as WorkerResponse)
}

/**
 * Filter commits based on criteria
 */
function filterCommits(commits: Commit3D[], filter: FilterOptions): void {
  let filtered = commits

  // Filter by authors
  if (filter.authors && filter.authors.length > 0) {
    filtered = filtered.filter((c) =>
      filter.authors!.includes(c.author_email)
    )
  }

  // Filter by branches
  if (filter.branches && filter.branches.length > 0) {
    filtered = filtered.filter((c) =>
      filter.branches!.includes(c.branch)
    )
  }

  // Filter by date range
  if (filter.dateFrom) {
    const fromTime = new Date(filter.dateFrom).getTime()
    filtered = filtered.filter(
      (c) => new Date(c.timestamp).getTime() >= fromTime
    )
  }

  if (filter.dateTo) {
    const toTime = new Date(filter.dateTo).getTime()
    filtered = filtered.filter(
      (c) => new Date(c.timestamp).getTime() <= toTime
    )
  }

  // Filter by search text
  if (filter.searchText && filter.searchText.length > 0) {
    const searchLower = filter.searchText.toLowerCase()
    filtered = filtered.filter(
      (c) =>
        c.message.toLowerCase().includes(searchLower) ||
        c.author.toLowerCase().includes(searchLower) ||
        c.sha.toLowerCase().includes(searchLower)
    )
  }

  postMessage({
    type: 'filtered',
    data: filtered,
  } as WorkerResponse)
}

