/**
 * Plan Execution API
 * 
 * Server-Sent Events (SSE) endpoint for real-time execution progress
 */

import { NextRequest } from 'next/server'
import { exec } from 'child_process'
import { promisify } from 'util'

const execAsync = promisify(exec)

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

interface ExecutionEvent {
  type: 'started' | 'progress' | 'step_completed' | 'file_changed' | 'completed' | 'failed'
  data: any
  timestamp: string
}

export async function POST(request: NextRequest) {
  const { PlanId } = await request.json()

  if (!PlanId) {
    return new Response(JSON.stringify({ error: 'Plan ID required' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json' },
    })
  }

  try {
    // Execute Plan via CLI
    const { stdout, stderr } = await execAsync(`codex Plan execute ${PlanId}`)

    return new Response(
      JSON.stringify({
        success: true,
        output: stdout,
        error: stderr || null,
      }),
      {
        headers: { 'Content-Type': 'application/json' },
      }
    )
  } catch (error: any) {
    return new Response(
      JSON.stringify({
        success: false,
        error: error.message,
      }),
      {
        status: 500,
        headers: { 'Content-Type': 'application/json' },
      }
    )
  }
}

export async function GET(request: NextRequest) {
  const searchParams = request.nextUrl.searchParams
  const PlanId = searchParams.get('PlanId')

  if (!PlanId) {
    return new Response('Plan ID required', { status: 400 })
  }

  // Create Server-Sent Events stream
  const encoder = new TextEncoder()

  const customReadable = new ReadableStream({
    async start(controller) {
      // Send initial connection event
      const connectedEvent: ExecutionEvent = {
        type: 'started',
        data: { PlanId },
        timestamp: new Date().toISOString(),
      }
      
      controller.enqueue(encoder.encode(`data: ${JSON.stringify(connectedEvent)}\n\n`))

      try {
        // Simulate execution progress
        // In production, this would read from execution logs or WebSocket
        for (let i = 1; i <= 5; i++) {
          await new Promise((resolve) => setTimeout(resolve, 1000))

          const progressEvent: ExecutionEvent = {
            type: 'progress',
            data: {
              current_step: i,
              total_steps: 5,
              message: `Executing step ${i}/5`,
            },
            timestamp: new Date().toISOString(),
          }

          controller.enqueue(encoder.encode(`data: ${JSON.stringify(progressEvent)}\n\n`))
        }

        // Send completion event
        const completedEvent: ExecutionEvent = {
          type: 'completed',
          data: {
            success: true,
            message: 'Plan executed successfully',
          },
          timestamp: new Date().toISOString(),
        }

        controller.enqueue(encoder.encode(`data: ${JSON.stringify(completedEvent)}\n\n`))
      } catch (error: any) {
        const failedEvent: ExecutionEvent = {
          type: 'failed',
          data: {
            error: error.message,
          },
          timestamp: new Date().toISOString(),
        }

        controller.enqueue(encoder.encode(`data: ${JSON.stringify(failedEvent)}\n\n`))
      } finally {
        controller.close()
      }
    },
  })

  return new Response(customReadable, {
    headers: {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      'Connection': 'keep-alive',
    },
  })
}






