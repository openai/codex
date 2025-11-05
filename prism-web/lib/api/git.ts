/**
 * Git Analysis API Client
 * 
 * Communicates with Rust CLI via child process execution
 */

import { exec } from 'child_process'
import { promisify } from 'util'

const execAsync = promisify(exec)

export interface Commit3D {
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

export interface FileHeat {
  path: string
  change_count: number
  additions: number
  deletions: number
  last_modified: string
  authors: string[]
  heat_level: number
  size?: number
}

export interface BranchNode {
  name: string
  head_sha: string
  is_active: boolean
  merge_count: number
  created_at?: string
  last_commit: string
  x: number
  y: number
  z: number
}

/**
 * Get commit history with 3D coordinates
 */
export async function getCommits(repoPath: string = '.', limit: number = 1000): Promise<Commit3D[]> {
  try {
    const { stdout } = await execAsync(`codex git-analyze commits --repo-path="${repoPath}" --limit=${limit}`)
    return JSON.parse(stdout)
  } catch (error) {
    console.error('Failed to get commits:', error)
    return []
  }
}

/**
 * Get file change heatmap
 */
export async function getHeatmap(repoPath: string = '.', limit: number = 1000): Promise<FileHeat[]> {
  try {
    const { stdout } = await execAsync(`codex git-analyze heatmap --repo-path="${repoPath}" --limit=${limit}`)
    return JSON.parse(stdout)
  } catch (error) {
    console.error('Failed to get heatmap:', error)
    return []
  }
}

/**
 * Get branch structure
 */
export async function getBranches(repoPath: string = '.'): Promise<BranchNode[]> {
  try {
    const { stdout } = await execAsync(`codex git-analyze branches --repo-path="${repoPath}"`)
    return JSON.parse(stdout)
  } catch (error) {
    console.error('Failed to get branches:', error)
    return []
  }
}

