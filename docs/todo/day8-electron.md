# Day 8 TODO - MCP 서버 통합 및 컨텍스트 관리 (Electron)

> **목표**: Model Context Protocol (MCP) 서버 연결 및 자동 컨텍스트 수집 시스템 구축

## 전체 개요

Day 8은 Codex UI에 MCP (Model Context Protocol) 통합을 완성합니다:
- MCP 클라이언트 구현
- 서버 설정 및 관리 UI
- 자동 컨텍스트 수집 (파일, Git, 환경변수)
- 리소스 브라우저
- 프롬프트 템플릿 시스템
- 도구 자동 발견 및 등록

**Electron 특화:**
- Native subprocess로 MCP 서버 실행
- IPC를 통한 안전한 서버 통신
- electron-store로 서버 설정 저장
- Native notification으로 연결 상태 알림
- Menu bar에 MCP 상태 표시

---

## Commit 43: MCP 클라이언트 구현

### 📋 작업 내용

1. **MCP SDK 통합**
2. **Server discovery**
3. **Connection pooling**
4. **Health check**

### 📁 파일 구조

```
src/main/mcp/
├── MCPClient.ts          # MCP 클라이언트
├── ServerManager.ts      # 서버 관리
├── types.ts              # MCP 타입 정의
└── index.ts

src/main/handlers/
└── mcp.ts                # MCP IPC handlers

src/renderer/types/
└── mcp.ts                # MCP 타입 (renderer)
```

### 1️⃣ MCP 타입 정의

**파일**: `src/main/mcp/types.ts`

```typescript
export interface MCPServerConfig {
  id: string;
  name: string;
  command: string;
  args: string[];
  env?: Record<string, string>;
  cwd?: string;
  enabled: boolean;
  autoStart: boolean;
}

export interface MCPServerStatus {
  id: string;
  status: 'disconnected' | 'connecting' | 'connected' | 'error';
  pid?: number;
  lastConnected?: number;
  error?: string;
}

export interface MCPResource {
  uri: string;
  name: string;
  description?: string;
  mimeType?: string;
}

export interface MCPTool {
  name: string;
  description: string;
  inputSchema: {
    type: 'object';
    properties: Record<string, any>;
    required?: string[];
  };
}

export interface MCPPrompt {
  name: string;
  description?: string;
  arguments?: Array<{
    name: string;
    description?: string;
    required?: boolean;
  }>;
}
```

### 2️⃣ MCP 클라이언트

**파일**: `src/main/mcp/MCPClient.ts`

```typescript
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { spawn, ChildProcess } from 'child_process';
import type { MCPServerConfig, MCPServerStatus, MCPResource, MCPTool, MCPPrompt } from './types';

export class MCPClient {
  private client: Client | null = null;
  private transport: StdioClientTransport | null = null;
  private process: ChildProcess | null = null;
  private config: MCPServerConfig;
  private status: MCPServerStatus;
  private reconnectTimer: NodeJS.Timeout | null = null;

  constructor(config: MCPServerConfig) {
    this.config = config;
    this.status = {
      id: config.id,
      status: 'disconnected',
    };
  }

  async connect(): Promise<void> {
    if (this.status.status === 'connected') {
      console.log(`MCP server ${this.config.name} already connected`);
      return;
    }

    this.status.status = 'connecting';

    try {
      // Spawn MCP server process
      this.process = spawn(this.config.command, this.config.args, {
        env: { ...process.env, ...this.config.env },
        cwd: this.config.cwd,
        stdio: ['pipe', 'pipe', 'pipe'],
      });

      // Create stdio transport
      this.transport = new StdioClientTransport({
        command: this.config.command,
        args: this.config.args,
        env: this.config.env,
      });

      // Create MCP client
      this.client = new Client(
        {
          name: 'codex-ui',
          version: '1.0.0',
        },
        {
          capabilities: {
            roots: {
              listChanged: true,
            },
            sampling: {},
          },
        }
      );

      // Connect to server
      await this.client.connect(this.transport);

      this.status = {
        id: this.config.id,
        status: 'connected',
        pid: this.process.pid,
        lastConnected: Date.now(),
      };

      console.log(`MCP server ${this.config.name} connected`);

      // Set up health check
      this.startHealthCheck();

      // Handle process exit
      this.process.on('exit', (code) => {
        console.log(`MCP server ${this.config.name} exited with code ${code}`);
        this.handleDisconnect();
      });

      this.process.on('error', (error) => {
        console.error(`MCP server ${this.config.name} error:`, error);
        this.handleError(error);
      });
    } catch (error) {
      console.error(`Failed to connect to MCP server ${this.config.name}:`, error);
      this.handleError(error as Error);
    }
  }

  async disconnect(): Promise<void> {
    if (this.reconnectTimer) {
      clearInterval(this.reconnectTimer);
      this.reconnectTimer = null;
    }

    if (this.client) {
      await this.client.close();
      this.client = null;
    }

    if (this.transport) {
      await this.transport.close();
      this.transport = null;
    }

    if (this.process) {
      this.process.kill();
      this.process = null;
    }

    this.status.status = 'disconnected';
  }

  private handleDisconnect(): void {
    this.status.status = 'disconnected';

    if (this.config.autoStart) {
      console.log(`Auto-reconnecting to ${this.config.name} in 5 seconds...`);
      setTimeout(() => this.connect(), 5000);
    }
  }

  private handleError(error: Error): void {
    this.status = {
      id: this.config.id,
      status: 'error',
      error: error.message,
    };
  }

  private startHealthCheck(): void {
    this.reconnectTimer = setInterval(async () => {
      try {
        if (!this.client) return;

        // Ping server
        await this.client.ping();
      } catch (error) {
        console.error(`Health check failed for ${this.config.name}:`, error);
        this.handleDisconnect();
      }
    }, 30000); // Every 30 seconds
  }

  async listResources(): Promise<MCPResource[]> {
    if (!this.client) {
      throw new Error('MCP client not connected');
    }

    const response = await this.client.listResources();
    return response.resources.map((r: any) => ({
      uri: r.uri,
      name: r.name,
      description: r.description,
      mimeType: r.mimeType,
    }));
  }

  async listTools(): Promise<MCPTool[]> {
    if (!this.client) {
      throw new Error('MCP client not connected');
    }

    const response = await this.client.listTools();
    return response.tools.map((t: any) => ({
      name: t.name,
      description: t.description,
      inputSchema: t.inputSchema,
    }));
  }

  async listPrompts(): Promise<MCPPrompt[]> {
    if (!this.client) {
      throw new Error('MCP client not connected');
    }

    const response = await this.client.listPrompts();
    return response.prompts.map((p: any) => ({
      name: p.name,
      description: p.description,
      arguments: p.arguments,
    }));
  }

  async callTool(name: string, args: Record<string, any>): Promise<any> {
    if (!this.client) {
      throw new Error('MCP client not connected');
    }

    const response = await this.client.callTool({
      name,
      arguments: args,
    });

    return response;
  }

  async readResource(uri: string): Promise<any> {
    if (!this.client) {
      throw new Error('MCP client not connected');
    }

    const response = await this.client.readResource({ uri });
    return response;
  }

  async getPrompt(name: string, args?: Record<string, string>): Promise<any> {
    if (!this.client) {
      throw new Error('MCP client not connected');
    }

    const response = await this.client.getPrompt({
      name,
      arguments: args,
    });

    return response;
  }

  getStatus(): MCPServerStatus {
    return this.status;
  }

  getConfig(): MCPServerConfig {
    return this.config;
  }

  updateConfig(config: Partial<MCPServerConfig>): void {
    this.config = { ...this.config, ...config };
  }
}
```

### 3️⃣ Server Manager

**파일**: `src/main/mcp/ServerManager.ts`

```typescript
import { MCPClient } from './MCPClient';
import type { MCPServerConfig, MCPServerStatus } from './types';
import Store from 'electron-store';

const store = new Store();

export class MCPServerManager {
  private clients: Map<string, MCPClient> = new Map();

  async initialize(): Promise<void> {
    const configs = this.loadConfigs();

    for (const config of configs) {
      if (config.enabled && config.autoStart) {
        await this.addServer(config);
      }
    }
  }

  async addServer(config: MCPServerConfig): Promise<void> {
    if (this.clients.has(config.id)) {
      throw new Error(`Server ${config.id} already exists`);
    }

    const client = new MCPClient(config);
    this.clients.set(config.id, client);

    if (config.enabled) {
      await client.connect();
    }

    this.saveConfigs();
  }

  async removeServer(id: string): Promise<void> {
    const client = this.clients.get(id);
    if (!client) {
      throw new Error(`Server ${id} not found`);
    }

    await client.disconnect();
    this.clients.delete(id);

    this.saveConfigs();
  }

  async connectServer(id: string): Promise<void> {
    const client = this.clients.get(id);
    if (!client) {
      throw new Error(`Server ${id} not found`);
    }

    await client.connect();
  }

  async disconnectServer(id: string): Promise<void> {
    const client = this.clients.get(id);
    if (!client) {
      throw new Error(`Server ${id} not found`);
    }

    await client.disconnect();
  }

  getServer(id: string): MCPClient | undefined {
    return this.clients.get(id);
  }

  getAllServers(): MCPClient[] {
    return Array.from(this.clients.values());
  }

  getServerStatus(id: string): MCPServerStatus | null {
    const client = this.clients.get(id);
    return client ? client.getStatus() : null;
  }

  getAllServerStatuses(): MCPServerStatus[] {
    return Array.from(this.clients.values()).map((c) => c.getStatus());
  }

  private loadConfigs(): MCPServerConfig[] {
    const configs = store.get('mcp.servers') as MCPServerConfig[];
    return configs || [];
  }

  private saveConfigs(): void {
    const configs = Array.from(this.clients.values()).map((c) => c.getConfig());
    store.set('mcp.servers', configs);
  }

  async shutdown(): Promise<void> {
    const promises = Array.from(this.clients.values()).map((c) => c.disconnect());
    await Promise.all(promises);
  }
}

// Singleton instance
export const mcpServerManager = new MCPServerManager();
```

### 4️⃣ MCP IPC Handlers

**파일**: `src/main/handlers/mcp.ts`

```typescript
import { ipcMain } from 'electron';
import { mcpServerManager } from '../mcp/ServerManager';
import type { MCPServerConfig } from '../mcp/types';

export function registerMCPHandlers() {
  // Add server
  ipcMain.handle('mcp:addServer', async (_event, config: MCPServerConfig) => {
    await mcpServerManager.addServer(config);
  });

  // Remove server
  ipcMain.handle('mcp:removeServer', async (_event, id: string) => {
    await mcpServerManager.removeServer(id);
  });

  // Connect server
  ipcMain.handle('mcp:connect', async (_event, id: string) => {
    await mcpServerManager.connectServer(id);
  });

  // Disconnect server
  ipcMain.handle('mcp:disconnect', async (_event, id: string) => {
    await mcpServerManager.disconnectServer(id);
  });

  // Get all servers
  ipcMain.handle('mcp:getServers', () => {
    return mcpServerManager.getAllServerStatuses();
  });

  // List resources
  ipcMain.handle('mcp:listResources', async (_event, serverId: string) => {
    const server = mcpServerManager.getServer(serverId);
    if (!server) {
      throw new Error(`Server ${serverId} not found`);
    }
    return await server.listResources();
  });

  // List tools
  ipcMain.handle('mcp:listTools', async (_event, serverId: string) => {
    const server = mcpServerManager.getServer(serverId);
    if (!server) {
      throw new Error(`Server ${serverId} not found`);
    }
    return await server.listTools();
  });

  // List prompts
  ipcMain.handle('mcp:listPrompts', async (_event, serverId: string) => {
    const server = mcpServerManager.getServer(serverId);
    if (!server) {
      throw new Error(`Server ${serverId} not found`);
    }
    return await server.listPrompts();
  });

  // Call tool
  ipcMain.handle(
    'mcp:callTool',
    async (_event, serverId: string, name: string, args: Record<string, any>) => {
      const server = mcpServerManager.getServer(serverId);
      if (!server) {
        throw new Error(`Server ${serverId} not found`);
      }
      return await server.callTool(name, args);
    }
  );

  // Read resource
  ipcMain.handle('mcp:readResource', async (_event, serverId: string, uri: string) => {
    const server = mcpServerManager.getServer(serverId);
    if (!server) {
      throw new Error(`Server ${serverId} not found`);
    }
    return await server.readResource(uri);
  });

  // Get prompt
  ipcMain.handle(
    'mcp:getPrompt',
    async (_event, serverId: string, name: string, args?: Record<string, string>) => {
      const server = mcpServerManager.getServer(serverId);
      if (!server) {
        throw new Error(`Server ${serverId} not found`);
      }
      return await server.getPrompt(name, args);
    }
  );
}
```

### 5️⃣ Main.ts 통합

**파일**: `src/main/index.ts` (수정)

```typescript
import { app, BrowserWindow } from 'electron';
import { mcpServerManager } from './mcp/ServerManager';
import { registerMCPHandlers } from './handlers/mcp';

// ... 기존 코드 ...

app.whenReady().then(async () => {
  // Initialize MCP servers
  await mcpServerManager.initialize();

  // Register MCP handlers
  registerMCPHandlers();

  // ... 기존 코드 ...
});

app.on('before-quit', async () => {
  await mcpServerManager.shutdown();
});
```

### 6️⃣ Renderer 타입

**파일**: `src/preload/index.d.ts` (확장)

```typescript
export interface ElectronAPI {
  // ... 기존 메서드들 ...

  // MCP
  mcpAddServer: (config: MCPServerConfig) => Promise<void>;
  mcpRemoveServer: (id: string) => Promise<void>;
  mcpConnect: (id: string) => Promise<void>;
  mcpDisconnect: (id: string) => Promise<void>;
  mcpGetServers: () => Promise<MCPServerStatus[]>;
  mcpListResources: (serverId: string) => Promise<MCPResource[]>;
  mcpListTools: (serverId: string) => Promise<MCPTool[]>;
  mcpListPrompts: (serverId: string) => Promise<MCPPrompt[]>;
  mcpCallTool: (serverId: string, name: string, args: Record<string, any>) => Promise<any>;
  mcpReadResource: (serverId: string, uri: string) => Promise<any>;
  mcpGetPrompt: (serverId: string, name: string, args?: Record<string, string>) => Promise<any>;
}
```

### ✅ 완료 기준

- [ ] MCP SDK 통합 완료
- [ ] Server manager 작동
- [ ] Connection pooling 구현
- [ ] Health check 자동 실행
- [ ] IPC handlers 등록
- [ ] Auto-reconnect 작동

### 📝 Commit Message

```
feat(mcp): implement MCP client with server management

- Integrate @modelcontextprotocol/sdk
- Create MCPClient with stdio transport
- Implement ServerManager with connection pooling
- Add health check and auto-reconnect
- Register IPC handlers for MCP operations
- Support multiple concurrent MCP servers

Electron-specific:
- Spawn MCP servers as child processes
- IPC for secure server communication
- electron-store for server configs
```

---

## Commit 44: MCP 서버 설정 UI

### 📋 작업 내용

1. **서버 목록 UI**
2. **서버 추가/편집 다이얼로그**
3. **Connection status 표시**
4. **실시간 상태 업데이트**

### 📁 파일 구조

```
src/renderer/components/mcp/
├── MCPServerList.tsx     # 서버 목록
├── MCPServerCard.tsx     # 서버 카드
├── MCPServerDialog.tsx   # 추가/편집 다이얼로그
└── MCPStatusIndicator.tsx # 상태 표시기

src/renderer/store/
└── useMCPStore.ts        # MCP 상태 관리
```

### 1️⃣ MCP Store

**파일**: `src/renderer/store/useMCPStore.ts`

```typescript
import { create } from 'zustand';
import { devtools } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import type { MCPServerConfig, MCPServerStatus } from '@/types/mcp';

interface MCPState {
  servers: MCPServerStatus[];
  selectedServerId: string | null;
  isLoading: boolean;
}

interface MCPActions {
  loadServers: () => Promise<void>;
  addServer: (config: MCPServerConfig) => Promise<void>;
  removeServer: (id: string) => Promise<void>;
  connectServer: (id: string) => Promise<void>;
  disconnectServer: (id: string) => Promise<void>;
  selectServer: (id: string | null) => void;
  refreshStatus: () => Promise<void>;
}

export const useMCPStore = create<MCPState & MCPActions>()(
  devtools(
    immer((set, get) => ({
      servers: [],
      selectedServerId: null,
      isLoading: false,

      loadServers: async () => {
        if (!window.electronAPI) return;

        set({ isLoading: true });
        try {
          const servers = await window.electronAPI.mcpGetServers();
          set({ servers });
        } catch (error) {
          console.error('Failed to load MCP servers:', error);
        } finally {
          set({ isLoading: false });
        }
      },

      addServer: async (config) => {
        if (!window.electronAPI) return;

        try {
          await window.electronAPI.mcpAddServer(config);
          await get().loadServers();
        } catch (error) {
          console.error('Failed to add MCP server:', error);
          throw error;
        }
      },

      removeServer: async (id) => {
        if (!window.electronAPI) return;

        try {
          await window.electronAPI.mcpRemoveServer(id);
          await get().loadServers();

          if (get().selectedServerId === id) {
            set({ selectedServerId: null });
          }
        } catch (error) {
          console.error('Failed to remove MCP server:', error);
          throw error;
        }
      },

      connectServer: async (id) => {
        if (!window.electronAPI) return;

        try {
          await window.electronAPI.mcpConnect(id);
          await get().refreshStatus();
        } catch (error) {
          console.error('Failed to connect MCP server:', error);
          throw error;
        }
      },

      disconnectServer: async (id) => {
        if (!window.electronAPI) return;

        try {
          await window.electronAPI.mcpDisconnect(id);
          await get().refreshStatus();
        } catch (error) {
          console.error('Failed to disconnect MCP server:', error);
          throw error;
        }
      },

      selectServer: (id) => {
        set({ selectedServerId: id });
      },

      refreshStatus: async () => {
        if (!window.electronAPI) return;

        try {
          const servers = await window.electronAPI.mcpGetServers();
          set({ servers });
        } catch (error) {
          console.error('Failed to refresh MCP server status:', error);
        }
      },
    }))
  )
);

// Auto-refresh status every 5 seconds
if (typeof window !== 'undefined') {
  setInterval(() => {
    useMCPStore.getState().refreshStatus();
  }, 5000);
}
```

### 2️⃣ Server List UI

**파일**: `src/renderer/components/mcp/MCPServerList.tsx`

```typescript
import React, { useEffect, useState } from 'react';
import { Plus, RefreshCw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { useMCPStore } from '@/store/useMCPStore';
import { MCPServerCard } from './MCPServerCard';
import { MCPServerDialog } from './MCPServerDialog';

export function MCPServerList() {
  const { servers, loadServers, refreshStatus, isLoading } = useMCPStore();
  const [dialogOpen, setDialogOpen] = useState(false);

  useEffect(() => {
    loadServers();
  }, []);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="p-4 border-b">
        <div className="flex items-center justify-between mb-2">
          <h2 className="font-semibold">MCP Servers</h2>
          <div className="flex gap-2">
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              onClick={refreshStatus}
              disabled={isLoading}
            >
              <RefreshCw className={`h-4 w-4 ${isLoading ? 'animate-spin' : ''}`} />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              onClick={() => setDialogOpen(true)}
            >
              <Plus className="h-4 w-4" />
            </Button>
          </div>
        </div>
        <p className="text-xs text-muted-foreground">
          {servers.length} server{servers.length !== 1 ? 's' : ''} configured
        </p>
      </div>

      {/* Server List */}
      <ScrollArea className="flex-1">
        {servers.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full p-8 text-center">
            <p className="text-sm text-muted-foreground mb-4">
              No MCP servers configured
            </p>
            <Button onClick={() => setDialogOpen(true)}>
              <Plus className="h-4 w-4 mr-2" />
              Add Server
            </Button>
          </div>
        ) : (
          <div className="p-4 space-y-3">
            {servers.map((server) => (
              <MCPServerCard key={server.id} server={server} />
            ))}
          </div>
        )}
      </ScrollArea>

      {/* Add Server Dialog */}
      <MCPServerDialog open={dialogOpen} onOpenChange={setDialogOpen} />
    </div>
  );
}
```

### 3️⃣ Server Card

**파일**: `src/renderer/components/mcp/MCPServerCard.tsx`

```typescript
import React from 'react';
import { Circle, Trash2, Power, PowerOff } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { cn } from '@/lib/utils';
import { useMCPStore } from '@/store/useMCPStore';
import type { MCPServerStatus } from '@/types/mcp';
import { toast } from 'react-hot-toast';

interface MCPServerCardProps {
  server: MCPServerStatus;
}

export function MCPServerCard({ server }: MCPServerCardProps) {
  const { connectServer, disconnectServer, removeServer, selectServer, selectedServerId } =
    useMCPStore();

  const isSelected = selectedServerId === server.id;

  const handleConnect = async () => {
    try {
      await connectServer(server.id);
      toast.success(`Connected to ${server.id}`);
    } catch (error) {
      toast.error(`Failed to connect: ${error}`);
    }
  };

  const handleDisconnect = async () => {
    try {
      await disconnectServer(server.id);
      toast.success(`Disconnected from ${server.id}`);
    } catch (error) {
      toast.error(`Failed to disconnect: ${error}`);
    }
  };

  const handleRemove = async () => {
    if (confirm(`Remove server ${server.id}?`)) {
      try {
        await removeServer(server.id);
        toast.success(`Removed ${server.id}`);
      } catch (error) {
        toast.error(`Failed to remove: ${error}`);
      }
    }
  };

  const statusColors = {
    connected: 'text-green-500',
    connecting: 'text-yellow-500',
    disconnected: 'text-gray-500',
    error: 'text-red-500',
  };

  return (
    <Card
      className={cn(
        'p-4 cursor-pointer hover:bg-accent transition-colors',
        isSelected && 'border-primary'
      )}
      onClick={() => selectServer(server.id)}
    >
      <div className="flex items-start justify-between">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <Circle className={cn('h-2 w-2 fill-current', statusColors[server.status])} />
            <h3 className="font-semibold text-sm truncate">{server.id}</h3>
          </div>
          <p className="text-xs text-muted-foreground capitalize">{server.status}</p>
          {server.error && (
            <p className="text-xs text-destructive mt-1">{server.error}</p>
          )}
          {server.pid && (
            <p className="text-xs text-muted-foreground mt-1">PID: {server.pid}</p>
          )}
        </div>

        <div className="flex gap-1">
          {server.status === 'connected' ? (
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7"
              onClick={(e) => {
                e.stopPropagation();
                handleDisconnect();
              }}
            >
              <PowerOff className="h-3 w-3" />
            </Button>
          ) : (
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7"
              onClick={(e) => {
                e.stopPropagation();
                handleConnect();
              }}
            >
              <Power className="h-3 w-3" />
            </Button>
          )}
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-destructive hover:text-destructive"
            onClick={(e) => {
              e.stopPropagation();
              handleRemove();
            }}
          >
            <Trash2 className="h-3 w-3" />
          </Button>
        </div>
      </div>
    </Card>
  );
}
```

### ✅ 완료 기준

- [ ] MCP 서버 목록 표시
- [ ] 서버 추가/삭제 UI
- [ ] 연결/연결 해제 버튼
- [ ] 실시간 상태 업데이트
- [ ] electron-store 저장

### 📝 Commit Message

```
feat(mcp): add MCP server configuration UI

- Create MCPServerList component
- Add server status cards with indicators
- Implement add/remove/connect/disconnect actions
- Real-time status updates every 5 seconds
- Persist server configs in electron-store

UI features:
- Color-coded status indicators
- Server selection
- Connection controls
```

---

## Commits 45-48: 컨텍스트, 리소스, 프롬프트, 도구

*Remaining commits consolidated for brevity*

### Commit 45: 컨텍스트 관리 시스템
- 파일 컨텍스트 자동 수집
- Git 정보 (branch, commit, diff)
- 환경 변수 컨텍스트
- 컨텍스트 우선순위 설정

### Commit 46: 리소스 브라우저
- MCP 리소스 탐색 UI
- 리소스 검색 및 필터링
- 리소스 미리보기 (텍스트, JSON)
- 즐겨찾기 관리

### Commit 47: 프롬프트 템플릿
- MCP prompt templates 목록
- 변수 치환 UI
- 템플릿 에디터
- 커스텀 템플릿 저장

### Commit 48: 도구 자동 발견
- MCP 도구 자동 등록
- 도구 파라미터 UI 생성
- 도구 실행 인터페이스
- 실행 히스토리

---

## 🎯 Day 8 완료 체크리스트

### 기능 완성도
- [ ] MCP 클라이언트 작동
- [ ] 서버 추가/제거
- [ ] 연결/연결 해제
- [ ] 리소스 탐색
- [ ] 프롬프트 템플릿
- [ ] 도구 자동 발견

### Electron 통합
- [ ] Subprocess로 서버 실행
- [ ] IPC 통신 작동
- [ ] electron-store 저장
- [ ] Native notification

### 코드 품질
- [ ] TypeScript 타입 완성
- [ ] 빌드 성공
- [ ] Console 에러 없음

---

## 📦 Dependencies

```json
{
  "dependencies": {
    "@modelcontextprotocol/sdk": "^0.1.0"
  }
}
```

---

**다음**: Day 9에서는 멀티모달 지원 (이미지, PDF)을 구현합니다.
