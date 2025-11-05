#!/usr/bin/env node

// Prism MCP Server for Claude Code Integration
import { Server } from '@modelcontextprotocol/sdk/server/index.js'
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js'
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from '@modelcontextprotocol/sdk/types.js'
import { createClient } from '@supabase/supabase-js'
import simpleGit from 'simple-git'

const SUPABASE_URL = process.env.PRISM_SUPABASE_URL || ''
const SUPABASE_KEY = process.env.PRISM_SUPABASE_KEY || ''
const API_BASE_URL = process.env.PRISM_API_URL || 'https://prism.dev'

const supabase = createClient(SUPABASE_URL, SUPABASE_KEY)

// Initialize MCP Server
const server = new Server(
  {
    name: 'prism-mcp-server',
    version: '1.2.0',
  },
  {
    capabilities: {
      tools: {},
    },
  }
)

// Tool: Visualize Repository
async function visualizeRepository(repoPath: string) {
  try {
    const git = simpleGit(repoPath)
    
    // Get repo info
    const log = await git.log({ maxCount: 1000 })
    const status = await git.status()
    const branches = await git.branch()
    
    // Create visualization via Supabase
    const { data, error } = await supabase
      .from('repositories')
      .insert({
        repo_url: repoPath,
        repo_name: repoPath.split('/').pop() || 'Unknown',
        total_commits: log.total,
      })
      .select()
      .single()

    if (error) throw error

    const shareUrl = `${API_BASE_URL}/share/${data.id}`

    return {
      success: true,
      totalCommits: log.total,
      branches: branches.all.length,
      currentBranch: branches.current,
      shareUrl,
      message: `Created 3D visualization with ${log.total} commits. View at: ${shareUrl}`
    }
  } catch (error: any) {
    return {
      success: false,
      error: error.message
    }
  }
}

// Tool: Analyze Code Quality
async function analyzeCode(code: string, language: string) {
  // Simple static analysis
  const lines = code.split('\n')
  const issues: string[] = []

  // Basic checks
  if (lines.length > 500) {
    issues.push('File is very long (>500 lines). Consider splitting.')
  }

  if (code.includes('console.log') && language === 'typescript') {
    issues.push('Found console.log statements. Remove before production.')
  }

  if (code.includes('any') && language === 'typescript') {
    issues.push('Usage of "any" type found. Consider using specific types.')
  }

  if (code.includes('TODO') || code.includes('FIXME')) {
    issues.push('Found TODO/FIXME comments.')
  }

  const analysis = {
    totalLines: lines.length,
    language,
    issues,
    complexity: lines.length > 300 ? 'High' : lines.length > 100 ? 'Medium' : 'Low',
    suggestions: issues.length === 0 ? ['Code looks good!'] : issues
  }

  return analysis
}

// Tool: Get Repository Stats
async function getRepoStats(repoPath: string) {
  try {
    const git = simpleGit(repoPath)
    
    const log = await git.log()
    const status = await git.status()
    const branches = await git.branch()
    const tags = await git.tags()

    // Get contributors
    const contributorsRaw = await git.raw(['shortlog', '-sn', '--all'])
    const contributors = contributorsRaw
      .split('\n')
      .filter(line => line.trim())
      .map(line => {
        const [commits, ...nameParts] = line.trim().split(/\s+/)
        return {
          name: nameParts.join(' '),
          commits: parseInt(commits)
        }
      })

    return {
      totalCommits: log.total,
      branches: branches.all.length,
      tags: tags.all.length,
      contributors: contributors.length,
      topContributors: contributors.slice(0, 5),
      currentBranch: branches.current,
      isDirty: status.modified.length > 0 || status.not_added.length > 0,
      modifiedFiles: status.modified.length,
    }
  } catch (error: any) {
    throw new Error(`Failed to get repo stats: ${error.message}`)
  }
}

// Register tools
server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools: [
      {
        name: 'visualize_repository',
        description: 'Create a 3D/4D visualization of a Git repository',
        inputSchema: {
          type: 'object',
          properties: {
            repoPath: {
              type: 'string',
              description: 'Path to the Git repository (local or URL)',
            },
          },
          required: ['repoPath'],
        },
      },
      {
        name: 'analyze_code',
        description: 'Perform static analysis on code snippet',
        inputSchema: {
          type: 'object',
          properties: {
            code: {
              type: 'string',
              description: 'Code to analyze',
            },
            language: {
              type: 'string',
              description: 'Programming language',
              enum: ['typescript', 'javascript', 'python', 'rust', 'go'],
            },
          },
          required: ['code', 'language'],
        },
      },
      {
        name: 'get_repo_stats',
        description: 'Get comprehensive statistics about a Git repository',
        inputSchema: {
          type: 'object',
          properties: {
            repoPath: {
              type: 'string',
              description: 'Path to the Git repository',
            },
          },
          required: ['repoPath'],
        },
      },
    ],
  }
})

// Handle tool calls
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params

  try {
    switch (name) {
      case 'visualize_repository': {
        const result = await visualizeRepository(args.repoPath as string)
        return {
          content: [
            {
              type: 'text',
              text: JSON.stringify(result, null, 2),
            },
          ],
        }
      }

      case 'analyze_code': {
        const result = await analyzeCode(args.code as string, args.language as string)
        return {
          content: [
            {
              type: 'text',
              text: `Code Analysis Results:\n\n${JSON.stringify(result, null, 2)}`,
            },
          ],
        }
      }

      case 'get_repo_stats': {
        const result = await getRepoStats(args.repoPath as string)
        return {
          content: [
            {
              type: 'text',
              text: `Repository Statistics:\n\n${JSON.stringify(result, null, 2)}`,
            },
          ],
        }
      }

      default:
        throw new Error(`Unknown tool: ${name}`)
    }
  } catch (error: any) {
    return {
      content: [
        {
          type: 'text',
          text: `Error: ${error.message}`,
        },
      ],
      isError: true,
    }
  }
})

// Start server
async function main() {
  const transport = new StdioServerTransport()
  await server.connect(transport)
  
  console.error('Prism MCP server started')
}

main().catch((error) => {
  console.error('Fatal error:', error)
  process.exit(1)
})


