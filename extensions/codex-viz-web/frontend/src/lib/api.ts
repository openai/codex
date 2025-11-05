import axios from 'axios'
import type { ApiResponse, Commit3D, FileStats, BranchNode } from '../types'

const api = axios.create({
  baseURL: '/api',
  timeout: 30000,
})

export async function fetchCommits(
  repoPath?: string,
  limit = 1000
): Promise<Commit3D[]> {
  const params = new URLSearchParams()
  if (repoPath) params.set('repo_path', repoPath)
  params.set('limit', limit.toString())

  const response = await api.get<ApiResponse<Commit3D[]>>(
    `/commits?${params.toString()}`
  )

  if (response.data.success && response.data.data) {
    return response.data.data
  }

  throw new Error(response.data.error || 'Failed to fetch commits')
}

export async function fetchFileHeatmap(
  repoPath?: string,
  limit = 1000
): Promise<FileStats[]> {
  const params = new URLSearchParams()
  if (repoPath) params.set('repo_path', repoPath)
  params.set('limit', limit.toString())

  const response = await api.get<ApiResponse<FileStats[]>>(
    `/files/heatmap?${params.toString()}`
  )

  if (response.data.success && response.data.data) {
    return response.data.data
  }

  throw new Error(response.data.error || 'Failed to fetch file heatmap')
}

export async function fetchBranchGraph(repoPath?: string): Promise<BranchNode[]> {
  const params = new URLSearchParams()
  if (repoPath) params.set('repo_path', repoPath)

  const response = await api.get<ApiResponse<BranchNode[]>>(
    `/branches/graph?${params.toString()}`
  )

  if (response.data.success && response.data.data) {
    return response.data.data
  }

  throw new Error(response.data.error || 'Failed to fetch branch graph')
}

export function createWebSocket(repoPath?: string): WebSocket {
  const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const wsHost = window.location.hostname
  const wsPort = '3001' // Backend WebSocket port
  
  const params = new URLSearchParams()
  if (repoPath) params.set('repo_path', repoPath)
  
  const wsUrl = `${wsProtocol}//${wsHost}:${wsPort}/api/realtime?${params.toString()}`
  return new WebSocket(wsUrl)
}

