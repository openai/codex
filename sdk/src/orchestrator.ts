/**
 * ClaudeCode-style autonomous orchestrator for Codex.
 * 
 * Provides transparent sub-agent coordination via MCP protocol.
 */

import { spawn, ChildProcess } from 'child_process';
import { EventEmitter } from 'events';

/**
 * Options for orchestration execution.
 */
export interface OrchestrateOptions {
    /** Complexity threshold (0.0-1.0) for triggering orchestration */
    complexityThreshold?: number;
    
    /** Execution strategy: sequential, parallel, or hybrid */
    strategy?: 'sequential' | 'parallel' | 'hybrid';
    
    /** Output format: text or json */
    format?: 'text' | 'json';
}

/**
 * Result of orchestrated execution.
 */
export interface OrchestratedResult {
    /** Whether orchestration was actually used */
    wasOrchestrated: boolean;
    
    /** Agents that were used */
    agentsUsed: string[];
    
    /** Execution summary */
    executionSummary: string;
    
    /** Individual agent results */
    agentResults?: AgentResult[];
    
    /** Total execution time in seconds */
    totalExecutionTimeSecs?: number;
    
    /** Task analysis */
    taskAnalysis?: TaskAnalysis;
}

/**
 * Individual agent execution result.
 */
export interface AgentResult {
    agentName: string;
    status: 'Pending' | 'Running' | 'Completed' | 'Failed' | 'Cancelled';
    artifacts: string[];
    tokensUsed: number;
    durationSecs: number;
    error?: string;
}

/**
 * Task analysis metadata.
 */
export interface TaskAnalysis {
    complexityScore: number;
    detectedKeywords: string[];
    recommendedAgents: string[];
    subtasks: string[];
    originalInput: string;
}

/**
 * Orchestration event for streaming.
 */
export interface OrchestrationEvent {
    type: 'plan_created' | 'agent_started' | 'agent_completed' | 'orchestration_completed';
    timestamp: string;
    message: string;
    data?: any;
}

/**
 * MCP JSON-RPC message types.
 */
interface MCPRequest {
    jsonrpc: '2.0';
    id: number;
    method: string;
    params?: any;
}

interface MCPResponse {
    jsonrpc: '2.0';
    id: number;
    result?: any;
    error?: {
        code: number;
        message: string;
    };
}

/**
 * CodexOrchestrator - ClaudeCode-style autonomous sub-agent orchestration.
 * 
 * @example
 * ```typescript
 * const orchestrator = new CodexOrchestrator();
 * 
 * // Execute with auto-orchestration
 * const result = await orchestrator.execute(
 *   "Implement user authentication with tests and security review"
 * );
 * 
 * console.log(`Orchestrated: ${result.wasOrchestrated}`);
 * console.log(`Agents used: ${result.agentsUsed.join(', ')}`);
 * ```
 */
export class CodexOrchestrator extends EventEmitter {
    private mcpProcess: ChildProcess | null = null;
    private nextRequestId = 1;
    private pendingRequests = new Map<number, {
        resolve: (value: any) => void;
        reject: (error: Error) => void;
    }>();
    private initializePromise: Promise<void> | null = null;

    /**
     * Create a new CodexOrchestrator instance.
     * 
     * @param codexCommand - Path to codex binary (default: 'codex')
     */
    constructor(private codexCommand: string = 'codex') {
        super();
    }

    /**
     * Initialize MCP connection to Codex.
     */
    private async initialize(): Promise<void> {
        if (this.initializePromise) {
            return this.initializePromise;
        }

        this.initializePromise = new Promise((resolve, reject) => {
            // Spawn codex mcp-server
            this.mcpProcess = spawn(this.codexCommand, ['mcp-server'], {
                stdio: ['pipe', 'pipe', 'inherit'],
            });

            if (!this.mcpProcess.stdout || !this.mcpProcess.stdin) {
                reject(new Error('Failed to create MCP process pipes'));
                return;
            }

            let buffer = '';
            this.mcpProcess.stdout.on('data', (chunk: Buffer) => {
                buffer += chunk.toString();
                const lines = buffer.split('\n');
                buffer = lines.pop() || '';

                for (const line of lines) {
                    if (!line.trim()) continue;
                    
                    try {
                        const message: MCPResponse = JSON.parse(line);
                        this.handleMCPResponse(message);
                    } catch (e) {
                        this.emit('error', new Error(`Failed to parse MCP response: ${e}`));
                    }
                }
            });

            this.mcpProcess.on('error', (err) => {
                this.emit('error', err);
                reject(err);
            });

            this.mcpProcess.on('close', (code) => {
                this.emit('close', code);
            });

            // Send initialize request
            this.sendRequest('initialize', {
                protocolVersion: '2024-11-05',
                capabilities: {},
                clientInfo: {
                    name: 'codex-orchestrator',
                    version: '0.1.0',
                },
            }).then(() => {
                // Send initialized notification
                this.sendNotification('notifications/initialized', {});
                resolve();
            }).catch(reject);
        });

        return this.initializePromise;
    }

    /**
     * Send MCP request and wait for response.
     */
    private async sendRequest(method: string, params?: any): Promise<any> {
        await this.initialize();

        const id = this.nextRequestId++;
        const request: MCPRequest = {
            jsonrpc: '2.0',
            id,
            method,
            params,
        };

        return new Promise((resolve, reject) => {
            this.pendingRequests.set(id, { resolve, reject });

            const requestLine = JSON.stringify(request) + '\n';
            this.mcpProcess?.stdin?.write(requestLine, (err) => {
                if (err) {
                    this.pendingRequests.delete(id);
                    reject(err);
                }
            });

            // Timeout after 60 seconds
            setTimeout(() => {
                if (this.pendingRequests.has(id)) {
                    this.pendingRequests.delete(id);
                    reject(new Error(`Request ${id} (${method}) timed out`));
                }
            }, 60000);
        });
    }

    /**
     * Send MCP notification (no response expected).
     */
    private sendNotification(method: string, params?: any): void {
        const notification = {
            jsonrpc: '2.0',
            method,
            params,
        };

        const notificationLine = JSON.stringify(notification) + '\n';
        this.mcpProcess?.stdin?.write(notificationLine);
    }

    /**
     * Handle MCP response from server.
     */
    private handleMCPResponse(message: MCPResponse): void {
        const pending = this.pendingRequests.get(message.id);
        if (!pending) return;

        this.pendingRequests.delete(message.id);

        if (message.error) {
            pending.reject(new Error(`MCP Error: ${message.error.message}`));
        } else {
            pending.resolve(message.result);
        }
    }

    /**
     * Execute a task with automatic orchestration.
     * 
     * @param goal - The task goal to execute
     * @param options - Orchestration options
     * @returns Orchestrated result
     */
    async execute(goal: string, options?: OrchestrateOptions): Promise<OrchestratedResult> {
        const result = await this.sendRequest('tools/call', {
            name: 'codex-auto-orchestrate',
            arguments: {
                goal,
                auto_threshold: options?.complexityThreshold ?? 0.7,
                strategy: options?.strategy ?? 'hybrid',
                format: options?.format ?? 'json',
            },
        });

        return this.parseResult(result);
    }

    /**
     * Execute a task with streaming progress updates.
     * 
     * @param goal - The task goal to execute
     * @param options - Orchestration options
     * @yields Orchestration events
     */
    async *executeStream(
        goal: string,
        options?: OrchestrateOptions
    ): AsyncIterableIterator<OrchestrationEvent> {
        // For now, execute normally and yield final result
        // Full streaming would require SSE support in MCP
        const result = await this.execute(goal, options);

        yield {
            type: 'orchestration_completed',
            timestamp: new Date().toISOString(),
            message: result.executionSummary,
            data: result,
        };
    }

    /**
     * Parse MCP tool call result into OrchestratedResult.
     */
    private parseResult(mcpResult: any): OrchestratedResult {
        // Extract text content from MCP result
        const textContent = mcpResult.content?.[0]?.text || '';

        // Try to parse as JSON first
        try {
            const jsonData = JSON.parse(textContent);
            return {
                wasOrchestrated: jsonData.was_orchestrated ?? false,
                agentsUsed: jsonData.recommended_agents || [],
                executionSummary: jsonData.execution_summary || textContent,
                agentResults: jsonData.agent_results,
                totalExecutionTimeSecs: jsonData.total_execution_time_secs,
                taskAnalysis: jsonData.task_analysis,
            };
        } catch {
            // If not JSON, return as text summary
            return {
                wasOrchestrated: textContent.includes('Will Orchestrate'),
                agentsUsed: this.extractAgentsFromText(textContent),
                executionSummary: textContent,
            };
        }
    }

    /**
     * Extract agent names from text output (fallback).
     */
    private extractAgentsFromText(text: string): string[] {
        const match = text.match(/Recommended Agents:\s*([^\n]+)/);
        if (match) {
            return match[1].split(',').map(s => s.trim());
        }
        return [];
    }

    /**
     * Close MCP connection and cleanup.
     */
    async close(): Promise<void> {
        if (this.mcpProcess) {
            this.mcpProcess.kill();
            this.mcpProcess = null;
        }
        this.pendingRequests.clear();
        this.initializePromise = null;
    }
}

/**
 * Create a new orchestrator instance (convenience function).
 */
export function createOrchestrator(codexCommand?: string): CodexOrchestrator {
    return new CodexOrchestrator(codexCommand);
}

