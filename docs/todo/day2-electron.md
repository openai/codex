# Day 2 TODO - 실시간 채팅 및 상태 관리 (Electron)

> **목표**: Electron 환경에서 실시간 채팅 기능을 완전히 구현하고, 메시지 영속화 및 Native 통합 완료

## 전체 개요

Day 2는 Codex UI의 핵심 기능인 실시간 채팅을 구현합니다. Day 1에서 구축한 Electron 기반 위에서:
- Zustand로 상태 관리
- electron-store로 메시지 영속화
- WebSocket으로 실시간 통신
- Streaming 응답 처리
- Native notification 통합

**Electron 특화 기능:**
- electron-store를 통한 메시지 자동 저장
- IPC를 통한 파일 시스템 접근
- Native notification으로 백그라운드 알림
- Main Process를 통한 서버 URL 관리

---

## Commit 7: Zustand 상태 관리 및 Electron Store 통합

### 📋 작업 내용

1. **메시지 타입 정의**
2. **Zustand 스토어 구현**
3. **electron-store 통합**
4. **IPC handlers 추가**

### 📁 파일 구조

```
src/renderer/
├── types/
│   └── message.ts          # 메시지 타입 정의
├── store/
│   ├── useChatStore.ts     # Zustand chat store
│   └── useSettingsStore.ts # Settings store
└── hooks/
    └── useElectronStore.ts # electron-store 연동

src/main/
└── handlers/
    ├── store.ts            # electron-store IPC handlers
    └── index.ts            # Handler 등록
```

### 1️⃣ 메시지 타입 정의

**파일**: `src/renderer/types/message.ts`

```typescript
// Message types
export type MessageRole = 'user' | 'assistant' | 'system';

export type MessageStatus = 'pending' | 'streaming' | 'completed' | 'error';

export interface ToolCall {
  id: string;
  type: 'function' | 'file_operation' | 'code_execution';
  function: {
    name: string;
    arguments: string; // JSON string
  };
  status: 'pending' | 'approved' | 'rejected' | 'executed';
  result?: {
    success: boolean;
    output?: string;
    error?: string;
  };
}

export interface MessageContent {
  type: 'text' | 'code' | 'image' | 'file' | 'tool_call';
  text?: string;
  code?: {
    language: string;
    content: string;
  };
  image?: {
    url: string;
    alt?: string;
  };
  file?: {
    name: string;
    path: string;
    size: number;
  };
  toolCall?: ToolCall;
}

export interface Message {
  id: string;
  role: MessageRole;
  content: MessageContent[];
  status: MessageStatus;
  timestamp: number;
  metadata?: {
    model?: string;
    tokens?: {
      prompt: number;
      completion: number;
      total: number;
    };
    cost?: number;
    duration?: number;
  };
  parentId?: string; // For threading
  editedAt?: number;
  deleted?: boolean;
}

// Chat session type
export interface ChatSession {
  id: string;
  title: string;
  messages: Message[];
  createdAt: number;
  updatedAt: number;
  metadata?: {
    model: string;
    totalTokens: number;
    totalCost: number;
  };
}

// WebSocket message types
export interface WSMessage {
  type: 'message' | 'delta' | 'tool_call' | 'error' | 'done';
  data: any;
  messageId?: string;
}

// Store state type
export interface ChatState {
  // Current session
  currentSessionId: string | null;
  sessions: Map<string, ChatSession>;

  // Current streaming state
  streamingMessageId: string | null;
  isStreaming: boolean;

  // WebSocket connection
  wsConnected: boolean;
  wsError: string | null;

  // UI state
  selectedMessageId: string | null;
  searchQuery: string;
}
```

### 2️⃣ Zustand Chat Store

**파일**: `src/renderer/store/useChatStore.ts`

```typescript
import { create } from 'zustand';
import { devtools, persist } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import type { ChatState, Message, ChatSession, MessageContent } from '@/types/message';
import { nanoid } from 'nanoid';

interface ChatActions {
  // Session management
  createSession: (title?: string) => string;
  switchSession: (sessionId: string) => void;
  deleteSession: (sessionId: string) => void;
  updateSessionTitle: (sessionId: string, title: string) => void;

  // Message operations
  addMessage: (content: MessageContent[], role: 'user' | 'assistant') => Message;
  updateMessage: (messageId: string, updates: Partial<Message>) => void;
  deleteMessage: (messageId: string) => void;
  appendToMessage: (messageId: string, content: MessageContent) => void;

  // Streaming operations
  startStreaming: (messageId: string) => void;
  appendStreamingContent: (messageId: string, delta: string) => void;
  finishStreaming: (messageId: string) => void;

  // WebSocket state
  setWSConnected: (connected: boolean) => void;
  setWSError: (error: string | null) => void;

  // UI state
  selectMessage: (messageId: string | null) => void;
  setSearchQuery: (query: string) => void;

  // Persistence
  loadFromElectronStore: () => Promise<void>;
  saveToElectronStore: () => Promise<void>;
}

type ChatStore = ChatState & ChatActions;

export const useChatStore = create<ChatStore>()(
  devtools(
    immer((set, get) => ({
      // Initial state
      currentSessionId: null,
      sessions: new Map(),
      streamingMessageId: null,
      isStreaming: false,
      wsConnected: false,
      wsError: null,
      selectedMessageId: null,
      searchQuery: '',

      // Session management
      createSession: (title?: string) => {
        const sessionId = nanoid();
        const session: ChatSession = {
          id: sessionId,
          title: title || `New Chat ${new Date().toLocaleString()}`,
          messages: [],
          createdAt: Date.now(),
          updatedAt: Date.now(),
        };

        set((state) => {
          state.sessions.set(sessionId, session);
          state.currentSessionId = sessionId;
        });

        // Save to electron-store
        get().saveToElectronStore();

        return sessionId;
      },

      switchSession: (sessionId: string) => {
        set((state) => {
          if (state.sessions.has(sessionId)) {
            state.currentSessionId = sessionId;
          }
        });
      },

      deleteSession: (sessionId: string) => {
        set((state) => {
          state.sessions.delete(sessionId);
          if (state.currentSessionId === sessionId) {
            const remainingSessions = Array.from(state.sessions.keys());
            state.currentSessionId = remainingSessions[0] || null;
          }
        });

        get().saveToElectronStore();
      },

      updateSessionTitle: (sessionId: string, title: string) => {
        set((state) => {
          const session = state.sessions.get(sessionId);
          if (session) {
            session.title = title;
            session.updatedAt = Date.now();
          }
        });

        get().saveToElectronStore();
      },

      // Message operations
      addMessage: (content: MessageContent[], role: 'user' | 'assistant') => {
        const { currentSessionId, sessions } = get();
        if (!currentSessionId) {
          throw new Error('No active session');
        }

        const message: Message = {
          id: nanoid(),
          role,
          content,
          status: 'completed',
          timestamp: Date.now(),
        };

        set((state) => {
          const session = state.sessions.get(currentSessionId);
          if (session) {
            session.messages.push(message);
            session.updatedAt = Date.now();
          }
        });

        get().saveToElectronStore();

        return message;
      },

      updateMessage: (messageId: string, updates: Partial<Message>) => {
        set((state) => {
          const session = state.sessions.get(state.currentSessionId!);
          if (session) {
            const message = session.messages.find(m => m.id === messageId);
            if (message) {
              Object.assign(message, updates);
              session.updatedAt = Date.now();
            }
          }
        });

        get().saveToElectronStore();
      },

      deleteMessage: (messageId: string) => {
        set((state) => {
          const session = state.sessions.get(state.currentSessionId!);
          if (session) {
            const index = session.messages.findIndex(m => m.id === messageId);
            if (index !== -1) {
              session.messages[index].deleted = true;
              session.updatedAt = Date.now();
            }
          }
        });

        get().saveToElectronStore();
      },

      appendToMessage: (messageId: string, content: MessageContent) => {
        set((state) => {
          const session = state.sessions.get(state.currentSessionId!);
          if (session) {
            const message = session.messages.find(m => m.id === messageId);
            if (message) {
              message.content.push(content);
              session.updatedAt = Date.now();
            }
          }
        });
      },

      // Streaming operations
      startStreaming: (messageId: string) => {
        set((state) => {
          state.streamingMessageId = messageId;
          state.isStreaming = true;

          const session = state.sessions.get(state.currentSessionId!);
          if (session) {
            const message = session.messages.find(m => m.id === messageId);
            if (message) {
              message.status = 'streaming';
            }
          }
        });
      },

      appendStreamingContent: (messageId: string, delta: string) => {
        set((state) => {
          const session = state.sessions.get(state.currentSessionId!);
          if (session) {
            const message = session.messages.find(m => m.id === messageId);
            if (message && message.content.length > 0) {
              const lastContent = message.content[message.content.length - 1];
              if (lastContent.type === 'text' && lastContent.text !== undefined) {
                lastContent.text += delta;
              }
            }
          }
        });
      },

      finishStreaming: (messageId: string) => {
        set((state) => {
          state.streamingMessageId = null;
          state.isStreaming = false;

          const session = state.sessions.get(state.currentSessionId!);
          if (session) {
            const message = session.messages.find(m => m.id === messageId);
            if (message) {
              message.status = 'completed';
            }
          }
        });

        get().saveToElectronStore();
      },

      // WebSocket state
      setWSConnected: (connected: boolean) => {
        set((state) => {
          state.wsConnected = connected;
          if (connected) {
            state.wsError = null;
          }
        });
      },

      setWSError: (error: string | null) => {
        set((state) => {
          state.wsError = error;
        });
      },

      // UI state
      selectMessage: (messageId: string | null) => {
        set((state) => {
          state.selectedMessageId = messageId;
        });
      },

      setSearchQuery: (query: string) => {
        set((state) => {
          state.searchQuery = query;
        });
      },

      // Persistence with Electron Store
      loadFromElectronStore: async () => {
        if (window.electronAPI) {
          try {
            const data = await window.electronAPI.getSetting('chatSessions');
            if (data) {
              set((state) => {
                // Convert plain object to Map
                const sessionsArray = data.sessions || [];
                state.sessions = new Map(
                  sessionsArray.map((s: ChatSession) => [s.id, s])
                );
                state.currentSessionId = data.currentSessionId || null;
              });
            }
          } catch (error) {
            console.error('Failed to load from electron-store:', error);
          }
        }
      },

      saveToElectronStore: async () => {
        if (window.electronAPI) {
          try {
            const { sessions, currentSessionId } = get();
            // Convert Map to array for serialization
            const sessionsArray = Array.from(sessions.values());
            await window.electronAPI.setSetting('chatSessions', {
              sessions: sessionsArray,
              currentSessionId,
            });
          } catch (error) {
            console.error('Failed to save to electron-store:', error);
          }
        }
      },
    }))
  )
);

// Auto-save on window close
if (typeof window !== 'undefined' && window.electronAPI) {
  window.addEventListener('beforeunload', () => {
    useChatStore.getState().saveToElectronStore();
  });
}
```

### 3️⃣ Electron Store IPC Handlers

**파일**: `src/main/handlers/store.ts`

```typescript
import { ipcMain } from 'electron';
import Store from 'electron-store';

// Define store schema
interface StoreSchema {
  chatSessions: {
    sessions: any[];
    currentSessionId: string | null;
  };
  settings: {
    theme: 'light' | 'dark' | 'system';
    apiKey?: string;
    model: string;
    temperature: number;
    maxTokens: number;
  };
  windowState: {
    width: number;
    height: number;
    x?: number;
    y?: number;
    isMaximized: boolean;
  };
}

const store = new Store<StoreSchema>({
  defaults: {
    chatSessions: {
      sessions: [],
      currentSessionId: null,
    },
    settings: {
      theme: 'system',
      model: 'claude-3-5-sonnet-20241022',
      temperature: 0.7,
      maxTokens: 4096,
    },
    windowState: {
      width: 1200,
      height: 800,
      isMaximized: false,
    },
  },
});

export function registerStoreHandlers() {
  // Get setting
  ipcMain.handle('store:get', (_event, key: string) => {
    return store.get(key as any);
  });

  // Set setting
  ipcMain.handle('store:set', (_event, key: string, value: any) => {
    store.set(key as any, value);
  });

  // Delete setting
  ipcMain.handle('store:delete', (_event, key: string) => {
    store.delete(key as any);
  });

  // Clear all
  ipcMain.handle('store:clear', () => {
    store.clear();
  });

  // Get entire store
  ipcMain.handle('store:getAll', () => {
    return store.store;
  });

  // Reset to defaults
  ipcMain.handle('store:reset', () => {
    store.clear();
  });
}

export { store };
```

**파일**: `src/main/handlers/index.ts`

```typescript
import { registerWindowHandlers } from './window';
import { registerStoreHandlers } from './store';
import { registerServerHandlers } from './server';

export function registerAllHandlers() {
  registerWindowHandlers();
  registerStoreHandlers();
  registerServerHandlers();
}
```

### 4️⃣ 타입 확장

**파일**: `src/preload/index.d.ts` (수정)

```typescript
export interface ElectronAPI {
  // ... 기존 메서드들 ...

  // Store methods
  getSetting: (key: string) => Promise<any>;
  setSetting: (key: string, value: any) => Promise<void>;
  deleteSetting: (key: string) => Promise<void>;
  clearSettings: () => Promise<void>;
  getAllSettings: () => Promise<any>;
  resetSettings: () => Promise<void>;
}
```

**파일**: `src/preload/index.ts` (수정)

```typescript
import { contextBridge, ipcRenderer } from 'electron';
import type { ElectronAPI } from './index.d';

const electronAPI: ElectronAPI = {
  // ... 기존 메서드들 ...

  // Store methods
  getSetting: (key: string) => ipcRenderer.invoke('store:get', key),
  setSetting: (key: string, value: any) => ipcRenderer.invoke('store:set', key, value),
  deleteSetting: (key: string) => ipcRenderer.invoke('store:delete', key),
  clearSettings: () => ipcRenderer.invoke('store:clear'),
  getAllSettings: () => ipcRenderer.invoke('store:getAll'),
  resetSettings: () => ipcRenderer.invoke('store:reset'),
};

contextBridge.exposeInMainWorld('electronAPI', electronAPI);
```

### ✅ 완료 기준

- [ ] 메시지 타입 완전히 정의됨
- [ ] Zustand store가 모든 채팅 상태를 관리함
- [ ] electron-store 통합 완료 및 자동 저장 작동
- [ ] IPC handlers가 설정 읽기/쓰기를 처리함
- [ ] TypeScript 타입 에러 없음

### 🧪 테스트

```typescript
// src/renderer/tests/useChatStore.test.ts
import { renderHook, act } from '@testing-library/react';
import { useChatStore } from '@/store/useChatStore';

describe('useChatStore', () => {
  it('should create a new session', () => {
    const { result } = renderHook(() => useChatStore());

    act(() => {
      const sessionId = result.current.createSession('Test Session');
      expect(sessionId).toBeTruthy();
      expect(result.current.currentSessionId).toBe(sessionId);
      expect(result.current.sessions.has(sessionId)).toBe(true);
    });
  });

  it('should add a message', () => {
    const { result } = renderHook(() => useChatStore());

    act(() => {
      result.current.createSession();
      const message = result.current.addMessage(
        [{ type: 'text', text: 'Hello' }],
        'user'
      );

      expect(message.role).toBe('user');
      expect(message.content[0].text).toBe('Hello');
    });
  });
});
```

### 📝 Commit Message

```
feat(store): add Zustand state management with electron-store integration

- Define comprehensive message and session types
- Implement Zustand chat store with immer middleware
- Integrate electron-store for persistence
- Add IPC handlers for settings management
- Support streaming state and WebSocket connection status
- Auto-save chat sessions on window close

Electron-specific:
- Use electron-store for durable persistence
- IPC communication for settings sync
- Main process handles all file I/O
```

---

## Commit 8: 채팅 UI 컴포넌트

### 📋 작업 내용

1. **MessageList 컴포넌트 (가상 스크롤)**
2. **MessageItem 컴포넌트**
3. **MessageInput 컴포넌트**
4. **CodeBlock 컴포넌트**
5. **TypingIndicator 컴포넌트**

### 📁 파일 구조

```
src/renderer/components/chat/
├── MessageList.tsx
├── MessageItem.tsx
├── MessageInput.tsx
├── CodeBlock.tsx
├── TypingIndicator.tsx
└── index.ts
```

### 1️⃣ MessageList 컴포넌트

**파일**: `src/renderer/components/chat/MessageList.tsx`

```typescript
import React, { useEffect, useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useChatStore } from '@/store/useChatStore';
import { MessageItem } from './MessageItem';
import { TypingIndicator } from './TypingIndicator';

export function MessageList() {
  const { currentSessionId, sessions, isStreaming } = useChatStore();
  const parentRef = useRef<HTMLDivElement>(null);

  const currentSession = currentSessionId
    ? sessions.get(currentSessionId)
    : null;

  const messages = currentSession?.messages.filter(m => !m.deleted) || [];

  // Virtual scrolling for performance
  const rowVirtualizer = useVirtualizer({
    count: messages.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 100, // Estimated message height
    overscan: 5,
  });

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    if (parentRef.current && messages.length > 0) {
      parentRef.current.scrollTop = parentRef.current.scrollHeight;
    }
  }, [messages.length]);

  if (!currentSession) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground">
        <div className="text-center">
          <h3 className="text-lg font-semibold mb-2">No session selected</h3>
          <p>Create a new chat to get started</p>
        </div>
      </div>
    );
  }

  if (messages.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground">
        <div className="text-center max-w-md">
          <h3 className="text-lg font-semibold mb-2">Start a conversation</h3>
          <p className="text-sm">
            Ask me anything about coding, debugging, or software development.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div
      ref={parentRef}
      className="flex-1 overflow-y-auto px-4 py-6"
      style={{ scrollBehavior: 'smooth' }}
    >
      <div
        style={{
          height: `${rowVirtualizer.getTotalSize()}px`,
          width: '100%',
          position: 'relative',
        }}
      >
        {rowVirtualizer.getVirtualItems().map((virtualItem) => {
          const message = messages[virtualItem.index];
          return (
            <div
              key={virtualItem.key}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                height: `${virtualItem.size}px`,
                transform: `translateY(${virtualItem.start}px)`,
              }}
            >
              <MessageItem message={message} />
            </div>
          );
        })}
      </div>

      {isStreaming && <TypingIndicator />}
    </div>
  );
}
```

### 2️⃣ MessageItem 컴포넌트

**파일**: `src/renderer/components/chat/MessageItem.tsx`

```typescript
import React from 'react';
import { format } from 'date-fns';
import { User, Bot, Copy, Edit2, Trash2, Check } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import type { Message } from '@/types/message';
import { CodeBlock } from './CodeBlock';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

interface MessageItemProps {
  message: Message;
}

export function MessageItem({ message }: MessageItemProps) {
  const [copied, setCopied] = React.useState(false);
  const isUser = message.role === 'user';

  const handleCopy = async () => {
    const textContent = message.content
      .filter(c => c.type === 'text')
      .map(c => c.text)
      .join('\n');

    await navigator.clipboard.writeText(textContent);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleEdit = () => {
    // TODO: Implement edit functionality
    console.log('Edit message:', message.id);
  };

  const handleDelete = () => {
    // TODO: Implement delete confirmation
    console.log('Delete message:', message.id);
  };

  return (
    <div
      className={cn(
        'group flex gap-4 p-6 hover:bg-accent/50 transition-colors',
        isUser && 'bg-muted/30'
      )}
    >
      {/* Avatar */}
      <div
        className={cn(
          'flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center',
          isUser
            ? 'bg-primary text-primary-foreground'
            : 'bg-secondary text-secondary-foreground'
        )}
      >
        {isUser ? <User className="w-4 h-4" /> : <Bot className="w-4 h-4" />}
      </div>

      {/* Content */}
      <div className="flex-1 min-w-0">
        {/* Header */}
        <div className="flex items-center gap-2 mb-2">
          <span className="font-semibold text-sm">
            {isUser ? 'You' : 'Codex'}
          </span>
          <span className="text-xs text-muted-foreground">
            {format(message.timestamp, 'HH:mm')}
          </span>
          {message.editedAt && (
            <span className="text-xs text-muted-foreground italic">
              (edited)
            </span>
          )}
        </div>

        {/* Message content */}
        <div className="prose prose-sm dark:prose-invert max-w-none">
          {message.content.map((content, index) => {
            if (content.type === 'text' && content.text) {
              return (
                <ReactMarkdown
                  key={index}
                  remarkPlugins={[remarkGfm]}
                  components={{
                    code({ node, inline, className, children, ...props }) {
                      const match = /language-(\w+)/.exec(className || '');
                      const language = match ? match[1] : '';

                      if (!inline && language) {
                        return (
                          <CodeBlock
                            language={language}
                            code={String(children).replace(/\n$/, '')}
                          />
                        );
                      }

                      return (
                        <code className={className} {...props}>
                          {children}
                        </code>
                      );
                    },
                  }}
                >
                  {content.text}
                </ReactMarkdown>
              );
            }

            if (content.type === 'code' && content.code) {
              return (
                <CodeBlock
                  key={index}
                  language={content.code.language}
                  code={content.code.content}
                />
              );
            }

            if (content.type === 'image' && content.image) {
              return (
                <img
                  key={index}
                  src={content.image.url}
                  alt={content.image.alt || 'Image'}
                  className="max-w-full rounded-lg"
                />
              );
            }

            return null;
          })}
        </div>

        {/* Metadata */}
        {message.metadata && (
          <div className="mt-2 text-xs text-muted-foreground space-y-1">
            {message.metadata.model && (
              <div>Model: {message.metadata.model}</div>
            )}
            {message.metadata.tokens && (
              <div>
                Tokens: {message.metadata.tokens.total.toLocaleString()}
              </div>
            )}
            {message.metadata.duration && (
              <div>
                Duration: {(message.metadata.duration / 1000).toFixed(2)}s
              </div>
            )}
          </div>
        )}
      </div>

      {/* Actions */}
      <div className="flex-shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
        <div className="flex gap-1">
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8"
            onClick={handleCopy}
          >
            {copied ? (
              <Check className="h-3 w-3" />
            ) : (
              <Copy className="h-3 w-3" />
            )}
          </Button>
          {isUser && (
            <>
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8"
                onClick={handleEdit}
              >
                <Edit2 className="h-3 w-3" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8 text-destructive hover:text-destructive"
                onClick={handleDelete}
              >
                <Trash2 className="h-3 w-3" />
              </Button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
```

### 3️⃣ CodeBlock 컴포넌트

**파일**: `src/renderer/components/chat/CodeBlock.tsx`

```typescript
import React, { useState } from 'react';
import { Light as SyntaxHighlighter } from 'react-syntax-highlighter';
import { atomOneDark, atomOneLight } from 'react-syntax-highlighter/dist/esm/styles/hljs';
import { Copy, Check } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useTheme } from '@/hooks/useTheme';

// Import only needed languages
import typescript from 'react-syntax-highlighter/dist/esm/languages/hljs/typescript';
import javascript from 'react-syntax-highlighter/dist/esm/languages/hljs/javascript';
import python from 'react-syntax-highlighter/dist/esm/languages/hljs/python';
import rust from 'react-syntax-highlighter/dist/esm/languages/hljs/rust';
import bash from 'react-syntax-highlighter/dist/esm/languages/hljs/bash';
import json from 'react-syntax-highlighter/dist/esm/languages/hljs/json';

SyntaxHighlighter.registerLanguage('typescript', typescript);
SyntaxHighlighter.registerLanguage('javascript', javascript);
SyntaxHighlighter.registerLanguage('python', python);
SyntaxHighlighter.registerLanguage('rust', rust);
SyntaxHighlighter.registerLanguage('bash', bash);
SyntaxHighlighter.registerLanguage('json', json);

interface CodeBlockProps {
  language: string;
  code: string;
  filename?: string;
}

export function CodeBlock({ language, code, filename }: CodeBlockProps) {
  const [copied, setCopied] = useState(false);
  const { theme } = useTheme();

  const handleCopy = async () => {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const isDark = theme === 'dark' ||
    (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);

  return (
    <div className="relative group my-4 rounded-lg overflow-hidden border">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 bg-muted border-b">
        <div className="flex items-center gap-2">
          {filename && (
            <span className="text-sm font-mono">{filename}</span>
          )}
          <span className="text-xs text-muted-foreground">{language}</span>
        </div>
        <Button
          variant="ghost"
          size="sm"
          className="h-6 w-6 p-0 opacity-0 group-hover:opacity-100 transition-opacity"
          onClick={handleCopy}
        >
          {copied ? (
            <Check className="h-3 w-3" />
          ) : (
            <Copy className="h-3 w-3" />
          )}
        </Button>
      </div>

      {/* Code */}
      <SyntaxHighlighter
        language={language}
        style={isDark ? atomOneDark : atomOneLight}
        customStyle={{
          margin: 0,
          padding: '1rem',
          fontSize: '0.875rem',
          lineHeight: '1.5',
        }}
        showLineNumbers
        wrapLines
      >
        {code}
      </SyntaxHighlighter>
    </div>
  );
}
```

### 4️⃣ MessageInput 컴포넌트

**파일**: `src/renderer/components/chat/MessageInput.tsx`

```typescript
import React, { useRef, useEffect } from 'react';
import { Send, Paperclip, Square } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import { useChatStore } from '@/store/useChatStore';

interface MessageInputProps {
  onSend: (message: string) => void;
  onStop?: () => void;
  disabled?: boolean;
}

export function MessageInput({ onSend, onStop, disabled }: MessageInputProps) {
  const [input, setInput] = React.useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { isStreaming } = useChatStore();

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = textareaRef.current.scrollHeight + 'px';
    }
  }, [input]);

  // Focus on mount
  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();

    const trimmed = input.trim();
    if (!trimmed || disabled || isStreaming) return;

    onSend(trimmed);
    setInput('');

    // Reset textarea height
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    // Submit on Enter (without Shift)
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit(e);
    }
  };

  const handleAttachment = async () => {
    if (!window.electronAPI) return;

    // Use native file picker
    const filePath = await window.electronAPI.selectFile();
    if (filePath) {
      console.log('Selected file:', filePath);
      // TODO: Handle file upload
    }
  };

  return (
    <div className="border-t bg-background p-4">
      <form onSubmit={handleSubmit} className="max-w-4xl mx-auto">
        <div className="relative flex items-end gap-2">
          {/* Attachment button */}
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="flex-shrink-0"
            onClick={handleAttachment}
            disabled={disabled || isStreaming}
          >
            <Paperclip className="h-4 w-4" />
          </Button>

          {/* Input */}
          <div className="flex-1 relative">
            <Textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Type your message... (Enter to send, Shift+Enter for new line)"
              className="min-h-[50px] max-h-[200px] resize-none pr-12"
              disabled={disabled || isStreaming}
            />
          </div>

          {/* Send/Stop button */}
          {isStreaming ? (
            <Button
              type="button"
              size="icon"
              variant="destructive"
              className="flex-shrink-0"
              onClick={onStop}
            >
              <Square className="h-4 w-4" />
            </Button>
          ) : (
            <Button
              type="submit"
              size="icon"
              className="flex-shrink-0"
              disabled={!input.trim() || disabled}
            >
              <Send className="h-4 w-4" />
            </Button>
          )}
        </div>

        {/* Hints */}
        <div className="mt-2 text-xs text-muted-foreground">
          <kbd className="px-1.5 py-0.5 rounded bg-muted">Enter</kbd> to send •{' '}
          <kbd className="px-1.5 py-0.5 rounded bg-muted">Shift</kbd> +{' '}
          <kbd className="px-1.5 py-0.5 rounded bg-muted">Enter</kbd> for new line
        </div>
      </form>
    </div>
  );
}
```

### 5️⃣ TypingIndicator 컴포넌트

**파일**: `src/renderer/components/chat/TypingIndicator.tsx`

```typescript
import React from 'react';
import { Bot } from 'lucide-react';

export function TypingIndicator() {
  return (
    <div className="flex gap-4 p-6">
      <div className="flex-shrink-0 w-8 h-8 rounded-full bg-secondary text-secondary-foreground flex items-center justify-center">
        <Bot className="w-4 h-4" />
      </div>
      <div className="flex items-center gap-1 mt-2">
        <div className="w-2 h-2 rounded-full bg-muted-foreground animate-bounce [animation-delay:-0.3s]" />
        <div className="w-2 h-2 rounded-full bg-muted-foreground animate-bounce [animation-delay:-0.15s]" />
        <div className="w-2 h-2 rounded-full bg-muted-foreground animate-bounce" />
      </div>
    </div>
  );
}
```

### 6️⃣ 필요한 IPC 추가

**파일**: `src/preload/index.d.ts` (확장)

```typescript
export interface ElectronAPI {
  // ... 기존 메서드들 ...

  // File selection
  selectFile: () => Promise<string | null>;
  selectFiles: () => Promise<string[] | null>;
}
```

**파일**: `src/main/handlers/dialog.ts` (신규)

```typescript
import { ipcMain, dialog } from 'electron';
import { BrowserWindow } from 'electron';

export function registerDialogHandlers() {
  ipcMain.handle('dialog:selectFile', async () => {
    const window = BrowserWindow.getFocusedWindow();
    if (!window) return null;

    const result = await dialog.showOpenDialog(window, {
      properties: ['openFile'],
      filters: [
        { name: 'All Files', extensions: ['*'] },
        { name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp'] },
        { name: 'Documents', extensions: ['pdf', 'doc', 'docx', 'txt'] },
      ],
    });

    return result.canceled ? null : result.filePaths[0];
  });

  ipcMain.handle('dialog:selectFiles', async () => {
    const window = BrowserWindow.getFocusedWindow();
    if (!window) return null;

    const result = await dialog.showOpenDialog(window, {
      properties: ['openFile', 'multiSelections'],
    });

    return result.canceled ? null : result.filePaths;
  });
}
```

### ✅ 완료 기준

- [ ] MessageList가 가상 스크롤로 대량 메시지 처리
- [ ] MessageItem이 모든 content 타입 렌더링
- [ ] CodeBlock이 syntax highlighting 지원
- [ ] MessageInput이 키보드 단축키 지원
- [ ] TypingIndicator 애니메이션 작동
- [ ] Native file picker 통합

### 📝 Commit Message

```
feat(ui): implement chat UI components with virtual scrolling

- Add MessageList with @tanstack/react-virtual
- Create MessageItem with markdown rendering
- Implement CodeBlock with syntax highlighting
- Build MessageInput with keyboard shortcuts
- Add TypingIndicator animation
- Integrate native file picker via IPC

Electron-specific:
- Use native dialog for file selection
- Support Cmd/Ctrl+Enter shortcuts
```

---

## Commit 9: WebSocket 통신 및 재연결 로직

### 📋 작업 내용

1. **WebSocket 클라이언트 구현**
2. **재연결 로직**
3. **서버 URL IPC 통합**
4. **연결 상태 관리**

### 📁 파일 구조

```
src/renderer/services/
├── websocket.ts          # WebSocket client
└── messageQueue.ts       # Offline message queue

src/main/handlers/
└── server.ts             # Server URL handler
```

### 1️⃣ WebSocket 클라이언트

**파일**: `src/renderer/services/websocket.ts`

```typescript
import { useChatStore } from '@/store/useChatStore';
import type { WSMessage } from '@/types/message';

export class WebSocketClient {
  private ws: WebSocket | null = null;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 1000;
  private pingInterval: ReturnType<typeof setInterval> | null = null;
  private serverUrl: string = '';

  constructor() {
    this.initServerUrl();
  }

  private async initServerUrl() {
    if (window.electronAPI) {
      this.serverUrl = await window.electronAPI.getServerUrl();
    } else {
      this.serverUrl = 'ws://localhost:8080';
    }
  }

  async connect(): Promise<void> {
    if (!this.serverUrl) {
      await this.initServerUrl();
    }

    const wsUrl = this.serverUrl.replace('http', 'ws') + '/ws';

    return new Promise((resolve, reject) => {
      try {
        this.ws = new WebSocket(wsUrl);

        this.ws.onopen = () => {
          console.log('WebSocket connected');
          this.reconnectAttempts = 0;
          useChatStore.getState().setWSConnected(true);
          this.startPing();
          resolve();
        };

        this.ws.onmessage = (event) => {
          this.handleMessage(event.data);
        };

        this.ws.onerror = (error) => {
          console.error('WebSocket error:', error);
          useChatStore.getState().setWSError('Connection error');
          reject(error);
        };

        this.ws.onclose = () => {
          console.log('WebSocket closed');
          useChatStore.getState().setWSConnected(false);
          this.stopPing();
          this.attemptReconnect();
        };
      } catch (error) {
        console.error('Failed to create WebSocket:', error);
        reject(error);
      }
    });
  }

  private handleMessage(data: string) {
    try {
      const message: WSMessage = JSON.parse(data);

      switch (message.type) {
        case 'message':
          // Full message received
          console.log('Received message:', message.data);
          break;

        case 'delta':
          // Streaming delta
          if (message.messageId) {
            useChatStore.getState().appendStreamingContent(
              message.messageId,
              message.data
            );
          }
          break;

        case 'tool_call':
          // Tool call received
          console.log('Tool call:', message.data);
          break;

        case 'done':
          // Streaming complete
          if (message.messageId) {
            useChatStore.getState().finishStreaming(message.messageId);
          }
          break;

        case 'error':
          // Error received
          console.error('Server error:', message.data);
          useChatStore.getState().setWSError(message.data);
          break;
      }
    } catch (error) {
      console.error('Failed to parse message:', error);
    }
  }

  send(data: any): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(data));
    } else {
      console.error('WebSocket not connected');
      throw new Error('WebSocket not connected');
    }
  }

  sendMessage(content: string): void {
    this.send({
      type: 'message',
      content,
      timestamp: Date.now(),
    });
  }

  stopStreaming(): void {
    this.send({
      type: 'stop',
    });
  }

  private startPing() {
    this.pingInterval = setInterval(() => {
      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({ type: 'ping' }));
      }
    }, 30000); // Ping every 30 seconds
  }

  private stopPing() {
    if (this.pingInterval) {
      clearInterval(this.pingInterval);
      this.pingInterval = null;
    }
  }

  private attemptReconnect() {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      console.error('Max reconnection attempts reached');
      useChatStore.getState().setWSError('Failed to reconnect');
      return;
    }

    this.reconnectAttempts++;
    const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);

    console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);

    setTimeout(() => {
      this.connect().catch((error) => {
        console.error('Reconnection failed:', error);
      });
    }, delay);
  }

  disconnect() {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.stopPing();
  }

  isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
  }
}

// Singleton instance
export const wsClient = new WebSocket Client();
```

### 2️⃣ Server URL Handler

**파일**: `src/main/handlers/server.ts`

```typescript
import { ipcMain } from 'electron';
import type { ServerManager } from '../server/ServerManager';

let serverManager: ServerManager;

export function registerServerHandlers(manager: ServerManager) {
  serverManager = manager;

  // Get server URL
  ipcMain.handle('server:getUrl', () => {
    return serverManager.getServerUrl();
  });

  // Get server status
  ipcMain.handle('server:getStatus', () => {
    return serverManager.getStatus();
  });

  // Restart server
  ipcMain.handle('server:restart', async () => {
    await serverManager.restart();
  });
}
```

**파일**: `src/main/server/ServerManager.ts` (확장)

```typescript
export class ServerManager {
  // ... 기존 메서드들 ...

  getServerUrl(): string {
    return `http://localhost:${this.serverPort}`;
  }

  getStatus(): {
    running: boolean;
    port: number;
    pid: number | null;
  } {
    return {
      running: this.serverProcess !== null,
      port: this.serverPort,
      pid: this.serverProcess?.pid || null,
    };
  }

  async restart(): Promise<void> {
    await this.stop();
    await this.start();
  }
}
```

### 3️⃣ IPC 타입 확장

**파일**: `src/preload/index.d.ts` (확장)

```typescript
export interface ElectronAPI {
  // ... 기존 메서드들 ...

  // Server methods
  getServerUrl: () => Promise<string>;
  getServerStatus: () => Promise<{ running: boolean; port: number; pid: number | null }>;
  restartServer: () => Promise<void>;
}
```

**파일**: `src/preload/index.ts` (확장)

```typescript
const electronAPI: ElectronAPI = {
  // ... 기존 메서드들 ...

  // Server methods
  getServerUrl: () => ipcRenderer.invoke('server:getUrl'),
  getServerStatus: () => ipcRenderer.invoke('server:getStatus'),
  restartServer: () => ipcRenderer.invoke('server:restart'),
};
```

### 4️⃣ WebSocket Hook

**파일**: `src/renderer/hooks/useWebSocket.ts`

```typescript
import { useEffect, useRef } from 'react';
import { wsClient } from '@/services/websocket';
import { useChatStore } from '@/store/useChatStore';
import { toast } from 'react-hot-toast';

export function useWebSocket() {
  const initialized = useRef(false);
  const { wsConnected, wsError } = useChatStore();

  useEffect(() => {
    if (initialized.current) return;
    initialized.current = true;

    // Connect on mount
    wsClient.connect().catch((error) => {
      console.error('Failed to connect:', error);
      toast.error('Failed to connect to server');
    });

    // Cleanup on unmount
    return () => {
      wsClient.disconnect();
    };
  }, []);

  // Show error toast
  useEffect(() => {
    if (wsError) {
      toast.error(wsError);
    }
  }, [wsError]);

  // Show reconnection toast
  useEffect(() => {
    if (wsConnected) {
      toast.success('Connected to server');
    } else {
      toast.error('Disconnected from server');
    }
  }, [wsConnected]);

  return {
    isConnected: wsConnected,
    error: wsError,
    client: wsClient,
  };
}
```

### ✅ 완료 기준

- [ ] WebSocket 연결 성공
- [ ] 재연결 로직 작동 (exponential backoff)
- [ ] Ping/pong으로 연결 유지
- [ ] Main Process에서 서버 URL 가져옴
- [ ] 연결 상태가 UI에 반영됨

### 🧪 테스트

수동 테스트:
1. 앱 시작 → WebSocket 연결 확인
2. 서버 중지 → 재연결 시도 확인
3. 서버 재시작 → 자동 연결 확인

### 📝 Commit Message

```
feat(websocket): implement WebSocket client with auto-reconnect

- Create WebSocket client with exponential backoff
- Add ping/pong for connection health check
- Integrate server URL from Main Process
- Implement connection state management
- Add error handling and user notifications

Electron-specific:
- Get server URL via IPC
- Support server restart command
- Handle offline/online transitions
```

---

## Commit 10: 스트리밍 응답 처리

### 📋 작업 내용

1. **SSE 기반 스트리밍**
2. **실시간 마크다운 렌더링**
3. **코드 하이라이팅 최적화**
4. **타이핑 애니메이션**

### 📁 파일 구조

```
src/renderer/components/chat/
└── StreamingMessage.tsx  # 스트리밍 메시지 전용 컴포넌트

src/renderer/services/
└── streaming.ts          # 스트리밍 핸들러
```

### 1️⃣ 스트리밍 서비스

**파일**: `src/renderer/services/streaming.ts`

```typescript
import { useChatStore } from '@/store/useChatStore';
import type { Message, MessageContent } from '@/types/message';
import { nanoid } from 'nanoid';

export class StreamingService {
  private controller: AbortController | null = null;

  async streamMessage(userMessage: string): Promise<void> {
    const store = useChatStore.getState();

    // Add user message
    store.addMessage([{ type: 'text', text: userMessage }], 'user');

    // Create assistant message placeholder
    const assistantMessage: Message = {
      id: nanoid(),
      role: 'assistant',
      content: [{ type: 'text', text: '' }],
      status: 'streaming',
      timestamp: Date.now(),
    };

    // Add to store
    const { currentSessionId, sessions } = store;
    if (currentSessionId) {
      const session = sessions.get(currentSessionId);
      if (session) {
        session.messages.push(assistantMessage);
        store.startStreaming(assistantMessage.id);
      }
    }

    // Start streaming
    this.controller = new AbortController();

    try {
      const serverUrl = window.electronAPI
        ? await window.electronAPI.getServerUrl()
        : 'http://localhost:8080';

      const response = await fetch(`${serverUrl}/api/chat/stream`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          message: userMessage,
          sessionId: currentSessionId,
        }),
        signal: this.controller.signal,
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const reader = response.body?.getReader();
      if (!reader) {
        throw new Error('No response body');
      }

      const decoder = new TextDecoder();
      let buffer = '';

      while (true) {
        const { done, value } = await reader.read();

        if (done) {
          store.finishStreaming(assistantMessage.id);
          break;
        }

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split('\n');
        buffer = lines.pop() || '';

        for (const line of lines) {
          if (line.startsWith('data: ')) {
            const data = line.slice(6);

            if (data === '[DONE]') {
              store.finishStreaming(assistantMessage.id);
              break;
            }

            try {
              const parsed = JSON.parse(data);

              if (parsed.delta) {
                store.appendStreamingContent(assistantMessage.id, parsed.delta);
              }

              if (parsed.metadata) {
                store.updateMessage(assistantMessage.id, {
                  metadata: parsed.metadata,
                });
              }
            } catch (e) {
              console.error('Failed to parse SSE data:', e);
            }
          }
        }
      }

      // Show native notification if window is not focused
      if (window.electronAPI && !document.hasFocus()) {
        const text = assistantMessage.content[0]?.text?.slice(0, 100) || 'New message';
        window.electronAPI.showNotification('Codex', text);
      }
    } catch (error: any) {
      if (error.name === 'AbortError') {
        console.log('Stream aborted');
        store.updateMessage(assistantMessage.id, {
          status: 'completed',
        });
      } else {
        console.error('Streaming error:', error);
        store.updateMessage(assistantMessage.id, {
          status: 'error',
        });
        store.setWSError(error.message);
      }
    } finally {
      this.controller = null;
    }
  }

  stopStreaming(): void {
    if (this.controller) {
      this.controller.abort();
      this.controller = null;
    }
  }
}

export const streamingService = new StreamingService();
```

### 2️⃣ Native Notification Handler

**파일**: `src/main/handlers/notification.ts`

```typescript
import { ipcMain, Notification } from 'electron';

export function registerNotificationHandlers() {
  ipcMain.handle('notification:show', (_event, title: string, body: string) => {
    if (Notification.isSupported()) {
      const notification = new Notification({
        title,
        body,
        silent: false,
      });

      notification.show();
    }
  });
}
```

### 3️⃣ Chat Page 통합

**파일**: `src/renderer/pages/Chat.tsx`

```typescript
import React, { useEffect } from 'react';
import { MessageList } from '@/components/chat/MessageList';
import { MessageInput } from '@/components/chat/MessageInput';
import { useChatStore } from '@/store/useChatStore';
import { streamingService } from '@/services/streaming';
import { useWebSocket } from '@/hooks/useWebSocket';

export function ChatPage() {
  const { currentSessionId, createSession, loadFromElectronStore } = useChatStore();
  const { isConnected } = useWebSocket();

  // Load saved sessions on mount
  useEffect(() => {
    loadFromElectronStore().then(() => {
      // Create default session if none exists
      if (!currentSessionId) {
        createSession();
      }
    });
  }, []);

  const handleSend = async (message: string) => {
    await streamingService.streamMessage(message);
  };

  const handleStop = () => {
    streamingService.stopStreaming();
  };

  return (
    <div className="flex flex-col h-screen">
      <MessageList />
      <MessageInput
        onSend={handleSend}
        onStop={handleStop}
        disabled={!isConnected}
      />
    </div>
  );
}
```

### 4️⃣ 타입 확장

**파일**: `src/preload/index.d.ts` (확장)

```typescript
export interface ElectronAPI {
  // ... 기존 메서드들 ...

  // Notification
  showNotification: (title: string, body: string) => Promise<void>;
}
```

### ✅ 완료 기준

- [ ] SSE 스트리밍 응답 처리
- [ ] 실시간 마크다운 렌더링
- [ ] 스트리밍 중단 기능
- [ ] Native notification 표시
- [ ] 메타데이터 (토큰, 비용) 업데이트

### 📝 Commit Message

```
feat(streaming): implement SSE-based streaming responses

- Add StreamingService with fetch-based SSE
- Support real-time markdown rendering
- Implement stream cancellation
- Add native notifications for background messages
- Track metadata (tokens, cost, duration)

Electron-specific:
- Show native notification when window unfocused
- Handle stream abort gracefully
```

---

## Commit 11: 메시지 기능 (복사, 편집, 삭제, 내보내기)

### 📋 작업 내용

1. **복사 기능**
2. **편집 기능**
3. **삭제 기능**
4. **검색 기능 (IPC)**
5. **내보내기 (Native dialog)**

### 📁 파일 구조

```
src/renderer/components/chat/
├── MessageActions.tsx    # 메시지 액션 버튼
└── MessageEditDialog.tsx # 편집 다이얼로그

src/main/handlers/
└── export.ts             # 내보내기 IPC
```

### 1️⃣ MessageActions 컴포넌트

**파일**: `src/renderer/components/chat/MessageActions.tsx`

```typescript
import React from 'react';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Button } from '@/components/ui/button';
import {
  MoreVertical,
  Copy,
  Edit2,
  Trash2,
  Download,
  Search,
} from 'lucide-react';
import type { Message } from '@/types/message';
import { useChatStore } from '@/store/useChatStore';
import { toast } from 'react-hot-toast';

interface MessageActionsProps {
  message: Message;
  onEdit?: () => void;
}

export function MessageActions({ message, onEdit }: MessageActionsProps) {
  const { deleteMessage } = useChatStore();

  const handleCopy = async () => {
    const text = message.content
      .filter(c => c.type === 'text')
      .map(c => c.text)
      .join('\n');

    await navigator.clipboard.writeText(text);
    toast.success('Copied to clipboard');
  };

  const handleDelete = () => {
    if (confirm('Are you sure you want to delete this message?')) {
      deleteMessage(message.id);
      toast.success('Message deleted');
    }
  };

  const handleExport = async () => {
    if (!window.electronAPI) {
      toast.error('Export is only available in desktop app');
      return;
    }

    const text = message.content
      .filter(c => c.type === 'text')
      .map(c => c.text)
      .join('\n');

    const filePath = await window.electronAPI.saveDialog({
      defaultPath: `message-${message.id}.md`,
      filters: [
        { name: 'Markdown', extensions: ['md'] },
        { name: 'Text', extensions: ['txt'] },
      ],
    });

    if (filePath) {
      await window.electronAPI.writeFile(filePath, text);
      toast.success('Message exported');
    }
  };

  const handleSearch = () => {
    // TODO: Implement search functionality
    toast('Search coming soon');
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" className="h-8 w-8">
          <MoreVertical className="h-4 w-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onClick={handleCopy}>
          <Copy className="h-4 w-4 mr-2" />
          Copy
        </DropdownMenuItem>

        {message.role === 'user' && onEdit && (
          <DropdownMenuItem onClick={onEdit}>
            <Edit2 className="h-4 w-4 mr-2" />
            Edit
          </DropdownMenuItem>
        )}

        <DropdownMenuItem onClick={handleSearch}>
          <Search className="h-4 w-4 mr-2" />
          Search similar
        </DropdownMenuItem>

        <DropdownMenuItem onClick={handleExport}>
          <Download className="h-4 w-4 mr-2" />
          Export
        </DropdownMenuItem>

        <DropdownMenuSeparator />

        <DropdownMenuItem
          onClick={handleDelete}
          className="text-destructive focus:text-destructive"
        >
          <Trash2 className="h-4 w-4 mr-2" />
          Delete
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
```

### 2️⃣ Export Handler

**파일**: `src/main/handlers/export.ts`

```typescript
import { ipcMain, dialog } from 'electron';
import { BrowserWindow } from 'electron';
import fs from 'fs/promises';
import path from 'path';

export function registerExportHandlers() {
  // Save dialog
  ipcMain.handle('dialog:save', async (_event, options) => {
    const window = BrowserWindow.getFocusedWindow();
    if (!window) return null;

    const result = await dialog.showSaveDialog(window, options);
    return result.canceled ? null : result.filePath;
  });

  // Write file
  ipcMain.handle('fs:writeFile', async (_event, filePath: string, content: string) => {
    await fs.writeFile(filePath, content, 'utf-8');
  });

  // Read file
  ipcMain.handle('fs:readFile', async (_event, filePath: string) => {
    return await fs.readFile(filePath, 'utf-8');
  });

  // Export session as markdown
  ipcMain.handle('export:markdown', async (_event, session: any) => {
    const window = BrowserWindow.getFocusedWindow();
    if (!window) return null;

    const result = await dialog.showSaveDialog(window, {
      defaultPath: `${session.title}.md`,
      filters: [{ name: 'Markdown', extensions: ['md'] }],
    });

    if (result.canceled || !result.filePath) return null;

    // Generate markdown
    let markdown = `# ${session.title}\n\n`;
    markdown += `Created: ${new Date(session.createdAt).toLocaleString()}\n\n`;
    markdown += `---\n\n`;

    for (const message of session.messages) {
      if (message.deleted) continue;

      markdown += `## ${message.role === 'user' ? 'You' : 'Codex'}\n\n`;

      for (const content of message.content) {
        if (content.type === 'text' && content.text) {
          markdown += content.text + '\n\n';
        } else if (content.type === 'code' && content.code) {
          markdown += `\`\`\`${content.code.language}\n${content.code.content}\n\`\`\`\n\n`;
        }
      }

      markdown += `---\n\n`;
    }

    await fs.writeFile(result.filePath, markdown, 'utf-8');
    return result.filePath;
  });
}
```

### 3️⃣ IPC 타입 확장

**파일**: `src/preload/index.d.ts` (확장)

```typescript
export interface ElectronAPI {
  // ... 기존 메서드들 ...

  // File operations
  saveDialog: (options: any) => Promise<string | null>;
  writeFile: (filePath: string, content: string) => Promise<void>;
  readFile: (filePath: string) => Promise<string>;

  // Export
  exportMarkdown: (session: any) => Promise<string | null>;
}
```

### ✅ 완료 기준

- [ ] 복사 기능 작동
- [ ] 편집 기능 구현
- [ ] 삭제 확인 다이얼로그
- [ ] Native save dialog로 내보내기
- [ ] Markdown 형식으로 세션 전체 내보내기

### 📝 Commit Message

```
feat(messages): add message actions (copy, edit, delete, export)

- Implement MessageActions dropdown menu
- Add copy to clipboard
- Support message editing for user messages
- Add delete with confirmation
- Implement export via native save dialog
- Support markdown export for full sessions

Electron-specific:
- Use native save dialog
- File I/O via Main Process
- Generate markdown from session data
```

---

## Commit 12: 에러 처리 및 알림

### 📋 작업 내용

1. **Toast 알림 시스템**
2. **에러 바운더리**
3. **Native notification 통합**
4. **재시도 로직**

### 📁 파일 구조

```
src/renderer/components/
├── ErrorBoundary.tsx     # React Error Boundary
└── Toaster.tsx           # Toast container

src/renderer/lib/
└── errorHandler.ts       # Global error handler
```

### 1️⃣ Error Boundary

**파일**: `src/renderer/components/ErrorBoundary.tsx`

```typescript
import React, { Component, ReactNode } from 'react';
import { AlertCircle, RefreshCw } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('Error Boundary caught:', error, errorInfo);

    // Log to file in Electron
    if (window.electronAPI) {
      window.electronAPI.logError({
        error: error.toString(),
        stack: error.stack,
        componentStack: errorInfo.componentStack,
        timestamp: Date.now(),
      });
    }
  }

  handleReset = () => {
    this.setState({ hasError: false, error: null });
  };

  handleReload = () => {
    window.location.reload();
  };

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div className="flex flex-col items-center justify-center h-screen p-8">
          <AlertCircle className="w-16 h-16 text-destructive mb-4" />
          <h1 className="text-2xl font-bold mb-2">Something went wrong</h1>
          <p className="text-muted-foreground mb-6 text-center max-w-md">
            {this.state.error?.message || 'An unexpected error occurred'}
          </p>
          <div className="flex gap-4">
            <Button onClick={this.handleReset} variant="outline">
              Try Again
            </Button>
            <Button onClick={this.handleReload}>
              <RefreshCw className="w-4 h-4 mr-2" />
              Reload App
            </Button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
```

### 2️⃣ Global Error Handler

**파일**: `src/renderer/lib/errorHandler.ts`

```typescript
import { toast } from 'react-hot-toast';

export class ErrorHandler {
  static initialize() {
    // Catch unhandled promise rejections
    window.addEventListener('unhandledrejection', (event) => {
      console.error('Unhandled promise rejection:', event.reason);

      ErrorHandler.handleError(event.reason);
      event.preventDefault();
    });

    // Catch global errors
    window.addEventListener('error', (event) => {
      console.error('Global error:', event.error);

      ErrorHandler.handleError(event.error);
      event.preventDefault();
    });
  }

  static handleError(error: any) {
    const message = error?.message || 'An unexpected error occurred';

    // Show toast
    toast.error(message, {
      duration: 5000,
      position: 'top-right',
    });

    // Log to Electron
    if (window.electronAPI) {
      window.electronAPI.logError({
        error: error?.toString(),
        stack: error?.stack,
        timestamp: Date.now(),
      });
    }

    // Show native notification for critical errors
    if (this.isCriticalError(error)) {
      if (window.electronAPI) {
        window.electronAPI.showNotification(
          'Error',
          'A critical error occurred. Please restart the app.'
        );
      }
    }
  }

  static isCriticalError(error: any): boolean {
    // Define critical error patterns
    const criticalPatterns = [
      /cannot read property/i,
      /undefined is not a function/i,
      /network error/i,
      /failed to fetch/i,
    ];

    const message = error?.message || error?.toString() || '';
    return criticalPatterns.some(pattern => pattern.test(message));
  }

  static async retry<T>(
    fn: () => Promise<T>,
    options: {
      maxAttempts?: number;
      delay?: number;
      backoff?: number;
    } = {}
  ): Promise<T> {
    const {
      maxAttempts = 3,
      delay = 1000,
      backoff = 2,
    } = options;

    let lastError: any;

    for (let attempt = 1; attempt <= maxAttempts; attempt++) {
      try {
        return await fn();
      } catch (error) {
        lastError = error;

        if (attempt < maxAttempts) {
          const waitTime = delay * Math.pow(backoff, attempt - 1);
          console.log(`Retry attempt ${attempt} failed, waiting ${waitTime}ms...`);
          await new Promise(resolve => setTimeout(resolve, waitTime));
        }
      }
    }

    throw lastError;
  }
}

// Initialize on module load
if (typeof window !== 'undefined') {
  ErrorHandler.initialize();
}
```

### 3️⃣ Error Logging Handler

**파일**: `src/main/handlers/logging.ts`

```typescript
import { ipcMain, app } from 'electron';
import fs from 'fs/promises';
import path from 'path';

const LOG_DIR = path.join(app.getPath('userData'), 'logs');
const ERROR_LOG = path.join(LOG_DIR, 'error.log');

async function ensureLogDir() {
  try {
    await fs.mkdir(LOG_DIR, { recursive: true });
  } catch (error) {
    console.error('Failed to create log directory:', error);
  }
}

export function registerLoggingHandlers() {
  ensureLogDir();

  ipcMain.handle('log:error', async (_event, errorData: any) => {
    const timestamp = new Date().toISOString();
    const logEntry = `\n[${timestamp}]\n${JSON.stringify(errorData, null, 2)}\n`;

    try {
      await fs.appendFile(ERROR_LOG, logEntry, 'utf-8');
    } catch (error) {
      console.error('Failed to write error log:', error);
    }
  });

  ipcMain.handle('log:getErrors', async () => {
    try {
      const content = await fs.readFile(ERROR_LOG, 'utf-8');
      return content;
    } catch (error) {
      return '';
    }
  });

  ipcMain.handle('log:clear', async () => {
    try {
      await fs.writeFile(ERROR_LOG, '', 'utf-8');
    } catch (error) {
      console.error('Failed to clear error log:', error);
    }
  });
}
```

### 4️⃣ Toaster 컴포넌트

**파일**: `src/renderer/components/Toaster.tsx`

```typescript
import { Toaster as HotToaster } from 'react-hot-toast';
import { useTheme } from '@/hooks/useTheme';

export function Toaster() {
  const { theme } = useTheme();

  return (
    <HotToaster
      position="top-right"
      toastOptions={{
        duration: 4000,
        style: {
          background: theme === 'dark' ? '#1f2937' : '#ffffff',
          color: theme === 'dark' ? '#f3f4f6' : '#111827',
          border: '1px solid',
          borderColor: theme === 'dark' ? '#374151' : '#e5e7eb',
        },
        success: {
          iconTheme: {
            primary: '#10b981',
            secondary: '#ffffff',
          },
        },
        error: {
          iconTheme: {
            primary: '#ef4444',
            secondary: '#ffffff',
          },
        },
      }}
    />
  );
}
```

### 5️⃣ App.tsx 통합

**파일**: `src/renderer/App.tsx` (수정)

```typescript
import React from 'react';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { Toaster } from '@/components/Toaster';
import { ChatPage } from '@/pages/Chat';
import '@/lib/errorHandler'; // Initialize global error handler

export function App() {
  return (
    <ErrorBoundary>
      <ChatPage />
      <Toaster />
    </ErrorBoundary>
  );
}
```

### ✅ 완료 기준

- [ ] Error Boundary가 React 에러를 캐치
- [ ] Global error handler가 작동
- [ ] Toast 알림이 에러를 표시
- [ ] Native notification이 critical 에러에 표시됨
- [ ] 에러 로그가 파일에 저장됨
- [ ] 재시도 로직 구현

### 📝 Commit Message

```
feat(errors): implement comprehensive error handling

- Add React Error Boundary with fallback UI
- Create global error handler for unhandled errors
- Integrate react-hot-toast for user notifications
- Implement error logging to file system
- Add retry logic with exponential backoff
- Show native notifications for critical errors

Electron-specific:
- Log errors to userData/logs/error.log
- Use native notifications for critical errors
- Provide app reload option in error UI
```

---

## 🎯 Day 2 완료 체크리스트

### 기능 완성도
- [ ] Zustand로 모든 채팅 상태 관리
- [ ] electron-store로 메시지 자동 저장
- [ ] 실시간 채팅 UI 완성
- [ ] WebSocket 연결 및 재연결
- [ ] SSE 스트리밍 응답 처리
- [ ] 메시지 복사/편집/삭제
- [ ] Native dialog로 내보내기
- [ ] 에러 처리 및 로깅

### Electron 통합
- [ ] electron-store로 영속화
- [ ] IPC로 파일 I/O
- [ ] Native notification 통합
- [ ] Native dialog 사용
- [ ] 에러 로그 파일 저장

### 코드 품질
- [ ] TypeScript 타입 에러 없음
- [ ] 모든 컴포넌트 작동
- [ ] Console에 에러 없음
- [ ] 빌드 성공

### 사용자 경험
- [ ] 메시지 전송 및 수신
- [ ] 스트리밍 응답이 부드럽게 표시
- [ ] 연결 끊김 시 자동 재연결
- [ ] 에러 발생 시 적절한 알림
- [ ] 백그라운드에서 알림

---

## 📦 설치된 Dependencies

### 신규 설치
```json
{
  "dependencies": {
    "zustand": "^4.4.7",
    "immer": "^10.0.3",
    "electron-store": "^8.1.0",
    "nanoid": "^5.0.4",
    "react-markdown": "^9.0.1",
    "remark-gfm": "^4.0.0",
    "react-syntax-highlighter": "^15.5.0",
    "react-hot-toast": "^2.4.1",
    "@tanstack/react-virtual": "^3.0.1",
    "date-fns": "^3.0.6"
  },
  "devDependencies": {
    "@types/react-syntax-highlighter": "^15.5.11"
  }
}
```

---

## 🔧 설정 파일

모든 설정은 Day 1에서 완료됨. 추가 설정 불필요.

---

## 📊 진행 상황

- **Day 1**: Electron + React 기본 구조 ✅
- **Day 2**: 실시간 채팅 및 상태 관리 ✅ (예정)
- **Day 3**: 파일 작업 및 도구 UI (다음)

---

**다음**: Day 3에서는 파일 탐색기, Monaco Editor, 파일 업로드/다운로드, 도구 호출 시각화를 구현합니다.
