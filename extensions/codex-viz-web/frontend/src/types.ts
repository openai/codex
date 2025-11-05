// Mirror backend types

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

export interface FileStats {
  path: string
  change_count: number
  additions: number
  deletions: number
  last_modified: string
  authors: string[]
  heat_level: number
  size: number
}

export interface BranchNode {
  name: string
  head_sha: string
  is_active: boolean
  merge_count: number
  created_at: string
  last_commit: string
  x: number
  y: number
  z: number
  connections: BranchConnection[]
}

export interface BranchConnection {
  target_branch: string
  merge_sha: string
  connection_type: 'merge' | 'fork' | 'rebase'
}

export type RealtimeEvent =
  | {
      type: 'new_commit'
      commit: Commit3D
    }
  | {
      type: 'file_changed'
      path: string
      change_type: 'added' | 'modified' | 'deleted'
    }
  | {
      type: 'branch_created'
      branch: BranchNode
    }
  | {
      type: 'branch_deleted'
      branch_name: string
    }

export interface ApiResponse<T> {
  success: boolean
  data?: T
  error?: string
}

