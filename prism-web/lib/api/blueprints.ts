/**
 * Blueprint API Client
 * 
 * Communicates with Rust CLI via child process execution
 */

import { exec } from 'child_process'
import { promisify } from 'util'

const execAsync = promisify(exec)

export interface Blueprint {
  id: string
  title: string
  goal: string
  approach: string
  mode: 'single' | 'orchestrated' | 'competition'
  state: 'Drafting' | 'Pending' | 'Approved' | 'Rejected' | 'Executing' | 'Completed' | 'Failed'
  created_at: string
  updated_at: string
  approved_by?: string | null
  rejected_reason?: string | null
  budget: {
    session_cap?: number
    cap_min?: number
  }
  work_items: Array<{
    name: string
    files_touched: string[]
    diff_contract: string
    tests: string[]
  }>
  risks: Array<{
    item: string
    mitigation: string
  }>
}

export interface CreateBlueprintRequest {
  title: string
  mode?: 'single' | 'orchestrated' | 'competition'
  budget_tokens?: number
  budget_time?: number
}

/**
 * List all blueprints
 */
export async function listBlueprints(state?: string): Promise<Blueprint[]> {
  try {
    const stateArg = state ? `--state ${state}` : ''
    const { stdout } = await execAsync(`codex blueprint list ${stateArg} --json`)
    return JSON.parse(stdout)
  } catch (error) {
    console.error('Failed to list blueprints:', error)
    return []
  }
}

/**
 * Create a new blueprint
 */
export async function createBlueprint(data: CreateBlueprintRequest): Promise<Blueprint> {
  const {
    title,
    mode = 'orchestrated',
    budget_tokens = 100000,
    budget_time = 30,
  } = data

  const { stdout } = await execAsync(
    `codex blueprint create "${title}" --mode=${mode} --budget-tokens=${budget_tokens} --budget-time=${budget_time} --json`
  )

  return JSON.parse(stdout)
}

/**
 * Get blueprint status
 */
export async function getBlueprint(id: string): Promise<Blueprint> {
  const { stdout } = await execAsync(`codex blueprint status ${id} --json`)
  return JSON.parse(stdout)
}

/**
 * Approve a blueprint
 */
export async function approveBlueprint(id: string): Promise<Blueprint> {
  const { stdout } = await execAsync(`codex blueprint approve ${id} --json`)
  return JSON.parse(stdout)
}

/**
 * Reject a blueprint
 */
export async function rejectBlueprint(id: string, reason: string): Promise<Blueprint> {
  const { stdout } = await execAsync(`codex blueprint reject ${id} --reason="${reason}" --json`)
  return JSON.parse(stdout)
}

/**
 * Export a blueprint
 */
export async function exportBlueprint(
  id: string,
  format: 'md' | 'json' | 'both' = 'both'
): Promise<{ markdown?: string; json?: string }> {
  const { stdout } = await execAsync(`codex blueprint export ${id} --format=${format} --json`)
  return JSON.parse(stdout)
}

/**
 * Toggle blueprint mode
 */
export async function toggleBlueprintMode(enabled: boolean): Promise<void> {
  const flag = enabled ? 'on' : 'off'
  await execAsync(`codex blueprint toggle ${flag}`)
}

/**
 * Get blueprint mode status
 */
export async function getBlueprintModeStatus(): Promise<{ enabled: boolean; timestamp: string }> {
  try {
    const { stdout } = await execAsync(`codex blueprint mode-status --json`)
    return JSON.parse(stdout)
  } catch (error) {
    return { enabled: false, timestamp: new Date().toISOString() }
  }
}

