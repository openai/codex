import {
  APIRequest,
  APIResponse,
  APIError,
  Conversation,
  Message,
  Model,
  Agent,
  MCPConnection,
  SecurityScan,
  ResearchResult,
  SystemMetrics,
  LoginForm,
  NewConversationForm,
} from '../types';

class CodexAPIError extends Error {
  constructor(
    public code: number,
    message: string,
    public data?: any
  ) {
    super(message);
    this.name = 'CodexAPIError';
  }
}

export class CodexAPIClient {
  private baseUrl: string;
  private wsConnection?: WebSocket;
  private requestId = 0;
  private pendingRequests = new Map<string | number, {
    resolve: (value: any) => void;
    reject: (error: Error) => void;
  }>();

  constructor(baseUrl = 'http://localhost:8787') {
    this.baseUrl = baseUrl;
  }

  // HTTP Request helper
  private async httpRequest<T>(
    method: string,
    params?: any,
    endpoint?: string
  ): Promise<T> {
    const id = ++this.requestId;
    const request: APIRequest = {
      method,
      params,
      id,
    };

    try {
      const response = await fetch(endpoint || this.baseUrl, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(request),
      });

      if (!response.ok) {
        throw new CodexAPIError(
          response.status,
          `HTTP ${response.status}: ${response.statusText}`
        );
      }

      const data: APIResponse<T> = await response.json();

      if (data.error) {
        throw new CodexAPIError(data.error.code, data.error.message, data.error.data);
      }

      return data.result!;
    } catch (error) {
      if (error instanceof CodexAPIError) {
        throw error;
      }
      throw new CodexAPIError(-1, `Network error: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
  }

  // WebSocket connection for real-time updates
  connectWebSocket(onMessage: (message: any) => void): void {
    try {
      this.wsConnection = new WebSocket(`ws://localhost:8787`);

      this.wsConnection.onopen = () => {
        console.log('WebSocket connected');
      };

      this.wsConnection.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data);
          onMessage(message);
        } catch (error) {
          console.error('Failed to parse WebSocket message:', error);
        }
      };

      this.wsConnection.onclose = () => {
        console.log('WebSocket disconnected');
        // Auto-reconnect after 5 seconds
        setTimeout(() => this.connectWebSocket(onMessage), 5000);
      };

      this.wsConnection.onerror = (error) => {
        console.error('WebSocket error:', error);
      };
    } catch (error) {
      console.error('Failed to connect WebSocket:', error);
    }
  }

  disconnectWebSocket(): void {
    if (this.wsConnection) {
      this.wsConnection.close();
      this.wsConnection = undefined;
    }
  }

  // Authentication
  async login(credentials: LoginForm): Promise<{ token?: string; authUrl?: string }> {
    const params = credentials.method === 'api-key'
      ? { type: 'apiKey', apiKey: credentials.apiKey }
      : { type: 'chatgpt' };

    return this.httpRequest('account/login', params);
  }

  async logout(): Promise<void> {
    return this.httpRequest('account/logout');
  }

  async getAccount(): Promise<any> {
    return this.httpRequest('account/read');
  }

  // Conversations
  async createConversation(config: NewConversationForm): Promise<Conversation> {
    const params = {
      model: config.model,
      initialMessage: config.initialMessage,
      // Add attachments handling if needed
    };
    return this.httpRequest('newConversation', params);
  }

  async sendMessage(
    conversationId: string,
    content: string,
    attachments?: File[]
  ): Promise<Message> {
    const params = {
      conversationId,
      items: [{
        type: 'text',
        text: content,
      }],
      // Handle attachments if needed
    };
    return this.httpRequest('sendUserTurn', params);
  }

  async listConversations(): Promise<Conversation[]> {
    return this.httpRequest('listConversations');
  }

  async resumeConversation(path: string): Promise<Conversation> {
    return this.httpRequest('resumeConversation', { path });
  }

  async archiveConversation(conversationId: string, path: string): Promise<void> {
    return this.httpRequest('archiveConversation', { conversationId, rolloutPath: path });
  }

  // Models
  async listModels(): Promise<Model[]> {
    const response = await this.httpRequest<{ items: Model[] }>('model/list');
    return response.items;
  }

  // Agents (Custom implementation - not in official API yet)
  async getAgents(): Promise<Agent[]> {
    // This would be implemented when the agent API is available
    // For now, return mock data
    return [
      {
        id: 'code-reviewer',
        name: 'Code Reviewer',
        type: 'code-reviewer',
        status: 'idle',
        description: 'コードの品質とセキュリティをレビューします',
      },
      {
        id: 'test-gen',
        name: 'Test Generator',
        type: 'test-gen',
        status: 'idle',
        description: '自動的にテストコードを生成します',
      },
      {
        id: 'sec-audit',
        name: 'Security Auditor',
        type: 'sec-audit',
        status: 'idle',
        description: 'セキュリティ脆弱性をスキャンします',
      },
      {
        id: 'researcher',
        name: 'Deep Researcher',
        type: 'researcher',
        status: 'idle',
        description: '高度な研究と分析を行います',
      },
    ];
  }

  async runAgent(agentId: string, context: any): Promise<any> {
    // Delegate to specific agent types
    switch (agentId) {
      case 'code-reviewer':
        return this.runCodeReview(context);
      case 'test-gen':
        return this.runTestGeneration(context);
      case 'sec-audit':
        return this.runSecurityAudit(context);
      case 'researcher':
        return this.runResearch(context);
      default:
        throw new CodexAPIError(-1, `Unknown agent: ${agentId}`);
    }
  }

  private async runCodeReview(context: { code: string; language?: string }): Promise<any> {
    // Implement code review logic
    return { status: 'completed', findings: [] };
  }

  private async runTestGeneration(context: { code: string; language?: string }): Promise<any> {
    // Implement test generation logic
    return { status: 'completed', tests: [] };
  }

  private async runSecurityAudit(context: { path?: string }): Promise<SecurityScan> {
    // Implement security audit logic
    return {
      id: 'scan-' + Date.now(),
      type: 'code',
      status: 'completed',
      findings: [],
      startedAt: new Date(),
      completedAt: new Date(),
    };
  }

  private async runResearch(context: { query: string }): Promise<ResearchResult> {
    // Implement research logic
    return {
      id: 'research-' + Date.now(),
      query: context.query,
      status: 'completed',
      sources: [],
      startedAt: new Date(),
      completedAt: new Date(),
    };
  }

  // MCP Connections (Mock implementation)
  async getMCPConnections(): Promise<MCPConnection[]> {
    return [
      {
        id: 'filesystem',
        name: 'File System',
        type: 'filesystem',
        status: 'connected',
        lastConnected: new Date(),
      },
      {
        id: 'github',
        name: 'GitHub',
        type: 'github',
        status: 'connected',
        lastConnected: new Date(),
      },
      {
        id: 'gemini',
        name: 'Gemini AI',
        type: 'gemini',
        status: 'connected',
        lastConnected: new Date(),
      },
    ];
  }

  // System metrics
  async getSystemMetrics(): Promise<SystemMetrics> {
    // In a real implementation, this would come from the server
    // For now, return mock data
    return {
      cpuUsage: 45,
      memoryUsage: 67,
      diskUsage: 23,
      activeProcesses: 12,
      uptime: 3600, // 1 hour
    };
  }

  // File operations
  async executeCommand(command: string[], cwd?: string): Promise<{ exitCode: number; stdout: string; stderr: string }> {
    return this.httpRequest('execOneOffCommand', {
      command,
      cwd: cwd || process.cwd(),
    });
  }

  async fuzzyFileSearch(query: string, roots: string[]): Promise<any[]> {
    return this.httpRequest('fuzzyFileSearch', { query, roots });
  }

  // Utility methods
  isConnected(): boolean {
    return this.wsConnection?.readyState === WebSocket.OPEN;
  }

  getBaseUrl(): string {
    return this.baseUrl;
  }

  setBaseUrl(url: string): void {
    this.baseUrl = url;
  }
}

// Singleton instance
export const apiClient = new CodexAPIClient();

// Export types
export { CodexAPIError };
