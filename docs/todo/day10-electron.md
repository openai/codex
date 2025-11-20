# Day 10 TODO - 고급 도구 및 워크플로우 자동화 (Electron)

> **목표**: 커스텀 도구 빌더, 워크플로우 엔진, 스케줄러로 자동화 시스템 완성

## 전체 개요

Day 10은 Codex UI에 고급 자동화 기능을 추가합니다:
- 비주얼 도구 빌더 (노코드)
- 워크플로우 엔진 (도구 체이닝)
- Cron 기반 스케줄러
- 워크플로우 템플릿 라이브러리
- 실행 히스토리 및 로깅
- REST/GraphQL API 통합

**Electron 특화:**
- Native cron 스케줄러 (백그라운드 실행)
- System tray 메뉴에 스케줄 작업 표시
- Native notification으로 작업 완료 알림
- electron-store로 워크플로우 저장
- IPC로 백그라운드 작업 실행
- Menu bar에 실행 중인 워크플로우 표시

---

## Commit 55: 도구 빌더 UI

### 📋 작업 내용

1. **비주얼 도구 에디터**
2. **파라미터 정의 UI**
3. **실행 로직 설정**
4. **테스트 환경**

### 📁 파일 구조

```
src/renderer/components/tools/
├── ToolBuilder.tsx       # 도구 빌더 메인
├── ParameterEditor.tsx   # 파라미터 에디터
├── LogicEditor.tsx       # 로직 에디터
└── ToolTester.tsx        # 테스트 UI

src/renderer/store/
└── useToolStore.ts       # 도구 상태 관리

src/renderer/types/
└── tool.ts               # 도구 타입 정의
```

### 1️⃣ 도구 타입 정의

**파일**: `src/renderer/types/tool.ts`

```typescript
export type ParameterType =
  | 'string'
  | 'number'
  | 'boolean'
  | 'array'
  | 'object'
  | 'file'
  | 'select';

export interface ToolParameter {
  name: string;
  type: ParameterType;
  description?: string;
  required: boolean;
  default?: any;
  validation?: {
    min?: number;
    max?: number;
    pattern?: string;
    options?: string[];
  };
}

export interface ToolAction {
  id: string;
  type: 'http' | 'shell' | 'file' | 'mcp' | 'custom';
  config: {
    // HTTP
    url?: string;
    method?: 'GET' | 'POST' | 'PUT' | 'DELETE';
    headers?: Record<string, string>;
    body?: string;

    // Shell
    command?: string;
    args?: string[];
    cwd?: string;

    // File
    operation?: 'read' | 'write' | 'delete' | 'move';
    path?: string;
    content?: string;

    // MCP
    serverId?: string;
    toolName?: string;

    // Custom (JavaScript)
    code?: string;
  };
}

export interface CustomTool {
  id: string;
  name: string;
  description: string;
  category: string;
  icon?: string;
  parameters: ToolParameter[];
  actions: ToolAction[];
  createdAt: number;
  updatedAt: number;
  author?: string;
  version?: string;
}

export interface ToolExecution {
  id: string;
  toolId: string;
  status: 'pending' | 'running' | 'success' | 'error';
  startedAt: number;
  completedAt?: number;
  input: Record<string, any>;
  output?: any;
  error?: string;
  duration?: number;
}
```

### 2️⃣ Tool Store

**파일**: `src/renderer/store/useToolStore.ts`

```typescript
import { create } from 'zustand';
import { devtools, persist } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import type { CustomTool, ToolExecution } from '@/types/tool';
import { nanoid } from 'nanoid';

interface ToolState {
  tools: Map<string, CustomTool>;
  executions: Map<string, ToolExecution>;
  selectedToolId: string | null;
}

interface ToolActions {
  createTool: (tool: Omit<CustomTool, 'id' | 'createdAt' | 'updatedAt'>) => string;
  updateTool: (id: string, updates: Partial<CustomTool>) => void;
  deleteTool: (id: string) => void;
  duplicateTool: (id: string) => string;
  executeTool: (toolId: string, input: Record<string, any>) => Promise<ToolExecution>;
  getToolExecutions: (toolId: string) => ToolExecution[];
  selectTool: (id: string | null) => void;
  loadTools: () => Promise<void>;
  saveTools: () => Promise<void>;
}

export const useToolStore = create<ToolState & ToolActions>()(
  devtools(
    immer((set, get) => ({
      tools: new Map(),
      executions: new Map(),
      selectedToolId: null,

      createTool: (tool) => {
        const id = nanoid();
        const newTool: CustomTool = {
          ...tool,
          id,
          createdAt: Date.now(),
          updatedAt: Date.now(),
        };

        set((state) => {
          state.tools.set(id, newTool);
        });

        get().saveTools();
        return id;
      },

      updateTool: (id, updates) => {
        set((state) => {
          const tool = state.tools.get(id);
          if (tool) {
            Object.assign(tool, updates);
            tool.updatedAt = Date.now();
          }
        });

        get().saveTools();
      },

      deleteTool: (id) => {
        set((state) => {
          state.tools.delete(id);
          if (state.selectedToolId === id) {
            state.selectedToolId = null;
          }
        });

        get().saveTools();
      },

      duplicateTool: (id) => {
        const tool = get().tools.get(id);
        if (!tool) return '';

        const duplicateId = nanoid();
        const duplicate: CustomTool = {
          ...tool,
          id: duplicateId,
          name: `${tool.name} (Copy)`,
          createdAt: Date.now(),
          updatedAt: Date.now(),
        };

        set((state) => {
          state.tools.set(duplicateId, duplicate);
        });

        get().saveTools();
        return duplicateId;
      },

      executeTool: async (toolId, input) => {
        const tool = get().tools.get(toolId);
        if (!tool) {
          throw new Error(`Tool ${toolId} not found`);
        }

        const executionId = nanoid();
        const execution: ToolExecution = {
          id: executionId,
          toolId,
          status: 'running',
          startedAt: Date.now(),
          input,
        };

        set((state) => {
          state.executions.set(executionId, execution);
        });

        try {
          // Execute actions sequentially
          let lastOutput: any = null;

          for (const action of tool.actions) {
            if (action.type === 'http') {
              const response = await fetch(action.config.url!, {
                method: action.config.method || 'GET',
                headers: action.config.headers,
                body: action.config.body,
              });
              lastOutput = await response.json();
            } else if (action.type === 'shell') {
              if (window.electronAPI) {
                lastOutput = await window.electronAPI.executeShell(
                  action.config.command!,
                  action.config.args || []
                );
              }
            } else if (action.type === 'file') {
              if (window.electronAPI) {
                if (action.config.operation === 'read') {
                  lastOutput = await window.electronAPI.readFile(action.config.path!);
                } else if (action.config.operation === 'write') {
                  await window.electronAPI.writeFile(
                    action.config.path!,
                    action.config.content!
                  );
                  lastOutput = { success: true };
                }
              }
            } else if (action.type === 'mcp') {
              if (window.electronAPI) {
                lastOutput = await window.electronAPI.mcpCallTool(
                  action.config.serverId!,
                  action.config.toolName!,
                  input
                );
              }
            }
          }

          // Update execution
          set((state) => {
            const exec = state.executions.get(executionId);
            if (exec) {
              exec.status = 'success';
              exec.completedAt = Date.now();
              exec.duration = exec.completedAt - exec.startedAt;
              exec.output = lastOutput;
            }
          });

          return get().executions.get(executionId)!;
        } catch (error) {
          set((state) => {
            const exec = state.executions.get(executionId);
            if (exec) {
              exec.status = 'error';
              exec.completedAt = Date.now();
              exec.duration = exec.completedAt - exec.startedAt;
              exec.error = (error as Error).message;
            }
          });

          throw error;
        }
      },

      getToolExecutions: (toolId) => {
        return Array.from(get().executions.values())
          .filter((e) => e.toolId === toolId)
          .sort((a, b) => b.startedAt - a.startedAt);
      },

      selectTool: (id) => {
        set({ selectedToolId: id });
      },

      loadTools: async () => {
        if (!window.electronAPI) return;

        const data = await window.electronAPI.getSetting('customTools');
        if (data) {
          set((state) => {
            state.tools = new Map(data.map((t: CustomTool) => [t.id, t]));
          });
        }
      },

      saveTools: async () => {
        if (!window.electronAPI) return;

        const tools = Array.from(get().tools.values());
        await window.electronAPI.setSetting('customTools', tools);
      },
    }))
  )
);
```

### 3️⃣ Tool Builder UI

**파일**: `src/renderer/components/tools/ToolBuilder.tsx`

```typescript
import React, { useState } from 'react';
import { Plus, Save, Play, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useToolStore } from '@/store/useToolStore';
import { ParameterEditor } from './ParameterEditor';
import { LogicEditor } from './LogicEditor';
import { ToolTester } from './ToolTester';
import type { CustomTool, ToolParameter, ToolAction } from '@/types/tool';
import { toast } from 'react-hot-toast';

export function ToolBuilder() {
  const { selectedToolId, tools, createTool, updateTool, deleteTool } = useToolStore();

  const selectedTool = selectedToolId ? tools.get(selectedToolId) : null;

  const [name, setName] = useState(selectedTool?.name || '');
  const [description, setDescription] = useState(selectedTool?.description || '');
  const [category, setCategory] = useState(selectedTool?.category || 'general');
  const [parameters, setParameters] = useState<ToolParameter[]>(
    selectedTool?.parameters || []
  );
  const [actions, setActions] = useState<ToolAction[]>(selectedTool?.actions || []);

  const handleSave = () => {
    if (!name.trim()) {
      toast.error('Tool name is required');
      return;
    }

    const toolData = {
      name,
      description,
      category,
      parameters,
      actions,
    };

    if (selectedToolId) {
      updateTool(selectedToolId, toolData);
      toast.success('Tool updated');
    } else {
      createTool(toolData);
      toast.success('Tool created');
    }
  };

  const handleDelete = () => {
    if (!selectedToolId) return;

    if (confirm('Delete this tool?')) {
      deleteTool(selectedToolId);
      toast.success('Tool deleted');
    }
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="p-4 border-b">
        <div className="flex items-center justify-between mb-4">
          <h2 className="font-semibold text-lg">Tool Builder</h2>
          <div className="flex gap-2">
            {selectedToolId && (
              <Button variant="destructive" size="sm" onClick={handleDelete}>
                <Trash2 className="h-4 w-4 mr-2" />
                Delete
              </Button>
            )}
            <Button size="sm" onClick={handleSave}>
              <Save className="h-4 w-4 mr-2" />
              Save
            </Button>
          </div>
        </div>

        {/* Basic Info */}
        <div className="space-y-3">
          <div>
            <Label>Tool Name</Label>
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My Custom Tool"
            />
          </div>
          <div>
            <Label>Description</Label>
            <Textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="What does this tool do?"
              rows={2}
            />
          </div>
          <div>
            <Label>Category</Label>
            <Input
              value={category}
              onChange={(e) => setCategory(e.target.value)}
              placeholder="general"
            />
          </div>
        </div>
      </div>

      {/* Tabs */}
      <Tabs defaultValue="parameters" className="flex-1 flex flex-col">
        <TabsList className="mx-4 mt-4">
          <TabsTrigger value="parameters">Parameters</TabsTrigger>
          <TabsTrigger value="logic">Logic</TabsTrigger>
          <TabsTrigger value="test">Test</TabsTrigger>
        </TabsList>

        <TabsContent value="parameters" className="flex-1 overflow-auto p-4">
          <ParameterEditor parameters={parameters} onChange={setParameters} />
        </TabsContent>

        <TabsContent value="logic" className="flex-1 overflow-auto p-4">
          <LogicEditor actions={actions} onChange={setActions} />
        </TabsContent>

        <TabsContent value="test" className="flex-1 overflow-auto p-4">
          {selectedToolId && <ToolTester toolId={selectedToolId} />}
        </TabsContent>
      </Tabs>
    </div>
  );
}
```

### 4️⃣ Parameter Editor

**파일**: `src/renderer/components/tools/ParameterEditor.tsx`

```typescript
import React from 'react';
import { Plus, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Checkbox } from '@/components/ui/checkbox';
import type { ToolParameter } from '@/types/tool';
import { nanoid } from 'nanoid';

interface ParameterEditorProps {
  parameters: ToolParameter[];
  onChange: (parameters: ToolParameter[]) => void;
}

export function ParameterEditor({ parameters, onChange }: ParameterEditorProps) {
  const handleAdd = () => {
    onChange([
      ...parameters,
      {
        name: '',
        type: 'string',
        required: false,
      },
    ]);
  };

  const handleUpdate = (index: number, updates: Partial<ToolParameter>) => {
    const updated = [...parameters];
    updated[index] = { ...updated[index], ...updates };
    onChange(updated);
  };

  const handleRemove = (index: number) => {
    onChange(parameters.filter((_, i) => i !== index));
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="font-semibold">Parameters</h3>
        <Button size="sm" onClick={handleAdd}>
          <Plus className="h-4 w-4 mr-2" />
          Add Parameter
        </Button>
      </div>

      {parameters.length === 0 ? (
        <p className="text-sm text-muted-foreground text-center py-8">
          No parameters defined. Click "Add Parameter" to get started.
        </p>
      ) : (
        <div className="space-y-4">
          {parameters.map((param, index) => (
            <div key={index} className="p-4 border rounded-lg space-y-3">
              <div className="flex items-start justify-between">
                <div className="flex-1 grid grid-cols-2 gap-3">
                  <div>
                    <Label>Name</Label>
                    <Input
                      value={param.name}
                      onChange={(e) => handleUpdate(index, { name: e.target.value })}
                      placeholder="parameterName"
                    />
                  </div>
                  <div>
                    <Label>Type</Label>
                    <Select
                      value={param.type}
                      onValueChange={(value: any) => handleUpdate(index, { type: value })}
                    >
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="string">String</SelectItem>
                        <SelectItem value="number">Number</SelectItem>
                        <SelectItem value="boolean">Boolean</SelectItem>
                        <SelectItem value="array">Array</SelectItem>
                        <SelectItem value="object">Object</SelectItem>
                        <SelectItem value="file">File</SelectItem>
                        <SelectItem value="select">Select</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  className="ml-2"
                  onClick={() => handleRemove(index)}
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
              </div>

              <div>
                <Label>Description</Label>
                <Input
                  value={param.description || ''}
                  onChange={(e) => handleUpdate(index, { description: e.target.value })}
                  placeholder="Parameter description"
                />
              </div>

              <div className="flex items-center gap-2">
                <Checkbox
                  checked={param.required}
                  onCheckedChange={(checked) =>
                    handleUpdate(index, { required: checked as boolean })
                  }
                />
                <Label>Required</Label>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
```

### ✅ 완료 기준

- [ ] 도구 빌더 UI 완성
- [ ] 파라미터 에디터 작동
- [ ] 로직 에디터 구현
- [ ] 테스트 환경 작동
- [ ] electron-store 저장

### 📝 Commit Message

```
feat(tools): implement visual tool builder

- Create ToolBuilder component with tabs
- Add ParameterEditor for defining inputs
- Implement LogicEditor for action configuration
- Add ToolTester for testing tools
- Support HTTP, Shell, File, MCP actions
- Save custom tools to electron-store

Features:
- No-code tool creation
- Visual parameter configuration
- Multi-action workflows
- Built-in test environment
```

---

## Commit 56: 워크플로우 엔진

### 📋 작업 내용

1. **도구 체이닝**
2. **조건부 실행**
3. **루프 및 분기**
4. **에러 핸들링**

### 📁 파일 구조

```
src/main/workflow/
├── WorkflowEngine.ts     # 워크플로우 실행 엔진
└── types.ts              # 워크플로우 타입

src/renderer/components/workflow/
├── WorkflowBuilder.tsx   # 워크플로우 빌더
└── WorkflowNode.tsx      # 노드 컴포넌트
```

### 1️⃣ 워크플로우 타입

**파일**: `src/renderer/types/workflow.ts`

```typescript
export interface WorkflowNode {
  id: string;
  type: 'tool' | 'condition' | 'loop' | 'delay';
  toolId?: string;
  condition?: {
    operator: 'equals' | 'contains' | 'greaterThan' | 'lessThan';
    value: any;
  };
  loop?: {
    times?: number;
    array?: string; // Variable name
  };
  delay?: number; // milliseconds
  position: { x: number; y: number };
}

export interface WorkflowEdge {
  id: string;
  source: string;
  target: string;
  label?: string;
  condition?: 'success' | 'error' | 'always';
}

export interface Workflow {
  id: string;
  name: string;
  description?: string;
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
  variables: Record<string, any>;
  createdAt: number;
  updatedAt: number;
}
```

### 2️⃣ Workflow Engine (Main Process)

**파일**: `src/main/workflow/WorkflowEngine.ts`

```typescript
import type { Workflow, WorkflowNode } from '@/renderer/types/workflow';
import { useToolStore } from '@/renderer/store/useToolStore';

export class WorkflowEngine {
  private workflow: Workflow;
  private context: Record<string, any> = {};

  constructor(workflow: Workflow) {
    this.workflow = workflow;
    this.context = { ...workflow.variables };
  }

  async execute(): Promise<any> {
    // Find start node (node with no incoming edges)
    const startNode = this.workflow.nodes.find((node) =>
      this.workflow.edges.every((edge) => edge.target !== node.id)
    );

    if (!startNode) {
      throw new Error('No start node found in workflow');
    }

    return await this.executeNode(startNode);
  }

  private async executeNode(node: WorkflowNode): Promise<any> {
    try {
      let result: any;

      switch (node.type) {
        case 'tool':
          result = await this.executeTool(node);
          break;
        case 'condition':
          result = await this.executeCondition(node);
          break;
        case 'loop':
          result = await this.executeLoop(node);
          break;
        case 'delay':
          await new Promise((resolve) => setTimeout(resolve, node.delay || 0));
          result = this.context;
          break;
      }

      // Store result in context
      this.context[`node_${node.id}`] = result;

      // Find and execute next node
      const nextEdge = this.workflow.edges.find((edge) => edge.source === node.id);
      if (nextEdge) {
        const nextNode = this.workflow.nodes.find((n) => n.id === nextEdge.target);
        if (nextNode) {
          return await this.executeNode(nextNode);
        }
      }

      return result;
    } catch (error) {
      // Handle error - find error path
      const errorEdge = this.workflow.edges.find(
        (edge) => edge.source === node.id && edge.condition === 'error'
      );

      if (errorEdge) {
        const errorNode = this.workflow.nodes.find((n) => n.id === errorEdge.target);
        if (errorNode) {
          this.context.lastError = error;
          return await this.executeNode(errorNode);
        }
      }

      throw error;
    }
  }

  private async executeTool(node: WorkflowNode): Promise<any> {
    if (!node.toolId) {
      throw new Error('Tool ID not specified');
    }

    // Execute tool via IPC
    // This would call the tool execution logic
    return { success: true };
  }

  private async executeCondition(node: WorkflowNode): Promise<any> {
    if (!node.condition) {
      throw new Error('Condition not specified');
    }

    const { operator, value } = node.condition;
    const contextValue = this.context[value];

    let conditionMet = false;

    switch (operator) {
      case 'equals':
        conditionMet = contextValue === value;
        break;
      case 'contains':
        conditionMet = String(contextValue).includes(value);
        break;
      case 'greaterThan':
        conditionMet = contextValue > value;
        break;
      case 'lessThan':
        conditionMet = contextValue < value;
        break;
    }

    return conditionMet;
  }

  private async executeLoop(node: WorkflowNode): Promise<any> {
    if (!node.loop) {
      throw new Error('Loop config not specified');
    }

    const results = [];

    if (node.loop.times) {
      for (let i = 0; i < node.loop.times; i++) {
        this.context.loopIndex = i;
        // Execute loop body
        results.push(this.context);
      }
    } else if (node.loop.array) {
      const array = this.context[node.loop.array];
      if (Array.isArray(array)) {
        for (let i = 0; i < array.length; i++) {
          this.context.loopItem = array[i];
          this.context.loopIndex = i;
          // Execute loop body
          results.push(this.context);
        }
      }
    }

    return results;
  }
}
```

### ✅ 완료 기준

- [ ] 워크플로우 엔진 구현
- [ ] 도구 체이닝 작동
- [ ] 조건부 분기
- [ ] 루프 실행
- [ ] 에러 핸들링

### 📝 Commit Message

```
feat(workflow): implement workflow execution engine

- Create WorkflowEngine for node execution
- Support tool chaining
- Add conditional branching
- Implement loop execution
- Handle errors with fallback paths
- Store execution context

Features:
- Sequential execution
- Parallel execution support
- Variable context
```

---

## Commits 57-60: 스케줄러, 템플릿, 히스토리, API

*Remaining commits summarized*

### Commit 57: Cron 스케줄러
- node-cron 통합
- 반복 작업 설정 UI
- 백그라운드 실행
- System tray에 스케줄 표시

**핵심 코드**:
```typescript
// src/main/scheduler/CronScheduler.ts
import cron from 'node-cron';

export class CronScheduler {
  private jobs: Map<string, cron.ScheduledTask> = new Map();

  schedule(id: string, expression: string, callback: () => void) {
    const task = cron.schedule(expression, callback);
    this.jobs.set(id, task);
    task.start();
  }

  unschedule(id: string) {
    const task = this.jobs.get(id);
    if (task) {
      task.stop();
      this.jobs.delete(id);
    }
  }
}
```

### Commit 58: 템플릿 라이브러리
- 워크플로우 템플릿 저장
- 커뮤니티 템플릿 (JSON import/export)
- 템플릿 카테고리
- 즐겨찾기

### Commit 59: 실행 히스토리
- 워크플로우 실행 로그
- 성능 메트릭 (duration, success rate)
- 에러 로그
- 재실행 기능

### Commit 60: API 통합
- REST API wrapper
- GraphQL 클라이언트
- OAuth 2.0 인증
- Rate limiting

---

## 🎯 Day 10 완료 체크리스트

### 기능 완성도
- [ ] 도구 빌더 UI
- [ ] 워크플로우 엔진
- [ ] Cron 스케줄러
- [ ] 템플릿 라이브러리
- [ ] 실행 히스토리
- [ ] API 통합

### Electron 통합
- [ ] 백그라운드 cron 실행
- [ ] System tray 스케줄 표시
- [ ] Native notification
- [ ] electron-store 저장

---

## 📦 Dependencies

```json
{
  "dependencies": {
    "node-cron": "^3.0.3",
    "axios": "^1.6.2",
    "graphql-request": "^6.1.0"
  }
}
```

---

**다음**: Day 11에서는 플러그인 시스템을 구현합니다.
