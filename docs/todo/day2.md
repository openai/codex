# Day 2 TODO - 실시간 채팅 기능 구현

## 목표
Codex 에이전트와의 실시간 양방향 통신을 구현하고, 사용자 친화적인 채팅 인터페이스를 완성합니다.

---

## 1. 메시지 상태 관리 (Commit 7)

### 요구사항
- Zustand를 활용한 전역 상태 관리
- 메시지 데이터 구조 정의
- 메시지 CRUD 작업 지원

### 작업 내용

#### 메시지 타입 정의
- [ ] `src/types/message.ts` 파일 생성
  ```typescript
  export enum MessageRole {
    USER = 'user',
    ASSISTANT = 'assistant',
    SYSTEM = 'system',
  }

  export enum MessageStatus {
    PENDING = 'pending',
    STREAMING = 'streaming',
    COMPLETED = 'completed',
    ERROR = 'error',
  }

  export interface MessageContent {
    type: 'text' | 'code' | 'image' | 'file';
    content: string;
    language?: string; // 코드 블록용
    metadata?: Record<string, any>;
  }

  export interface ToolCall {
    id: string;
    name: string;
    arguments: Record<string, any>;
    status: 'pending' | 'running' | 'completed' | 'failed';
    result?: string;
    error?: string;
    timestamp: number;
  }

  export interface Message {
    id: string;
    role: MessageRole;
    content: MessageContent[];
    status: MessageStatus;
    toolCalls?: ToolCall[];
    timestamp: number;
    parentId?: string; // 스레드 지원용
    metadata?: {
      model?: string;
      tokens?: number;
      duration?: number;
    };
  }
  ```

- [ ] `src/types/index.ts`에서 export
  ```typescript
  export * from './message';
  export * from './session';
  export * from './settings';
  ```

#### Zustand 채팅 스토어 구현
- [ ] `src/store/chat-store.ts` 파일 생성
  ```typescript
  import { create } from 'zustand';
  import { persist } from 'zustand/middleware';
  import { Message, MessageRole, MessageStatus } from '@/types/message';

  interface ChatState {
    // State
    messages: Message[];
    isLoading: boolean;
    currentStreamingMessageId: string | null;

    // Actions
    addMessage: (message: Omit<Message, 'id' | 'timestamp'>) => Message;
    updateMessage: (id: string, updates: Partial<Message>) => void;
    deleteMessage: (id: string) => void;
    clearMessages: () => void;

    // Streaming
    startStreaming: (messageId: string) => void;
    appendToStreamingMessage: (content: string) => void;
    completeStreaming: () => void;

    // Tool calls
    addToolCall: (messageId: string, toolCall: ToolCall) => void;
    updateToolCall: (messageId: string, toolCallId: string, updates: Partial<ToolCall>) => void;
  }

  export const useChatStore = create<ChatState>()(
    persist(
      (set, get) => ({
        messages: [],
        isLoading: false,
        currentStreamingMessageId: null,

        addMessage: (message) => {
          const newMessage: Message = {
            ...message,
            id: crypto.randomUUID(),
            timestamp: Date.now(),
            status: message.status || MessageStatus.COMPLETED,
          };

          set((state) => ({
            messages: [...state.messages, newMessage],
          }));

          return newMessage;
        },

        updateMessage: (id, updates) => {
          set((state) => ({
            messages: state.messages.map((msg) =>
              msg.id === id ? { ...msg, ...updates } : msg
            ),
          }));
        },

        deleteMessage: (id) => {
          set((state) => ({
            messages: state.messages.filter((msg) => msg.id !== id),
          }));
        },

        clearMessages: () => {
          set({ messages: [] });
        },

        startStreaming: (messageId) => {
          set({ currentStreamingMessageId: messageId, isLoading: true });
          get().updateMessage(messageId, { status: MessageStatus.STREAMING });
        },

        appendToStreamingMessage: (content) => {
          const { currentStreamingMessageId } = get();
          if (!currentStreamingMessageId) return;

          set((state) => ({
            messages: state.messages.map((msg) => {
              if (msg.id === currentStreamingMessageId) {
                const lastContent = msg.content[msg.content.length - 1];
                if (lastContent?.type === 'text') {
                  return {
                    ...msg,
                    content: [
                      ...msg.content.slice(0, -1),
                      { ...lastContent, content: lastContent.content + content },
                    ],
                  };
                }
                return {
                  ...msg,
                  content: [...msg.content, { type: 'text', content }],
                };
              }
              return msg;
            }),
          }));
        },

        completeStreaming: () => {
          const { currentStreamingMessageId } = get();
          if (!currentStreamingMessageId) return;

          get().updateMessage(currentStreamingMessageId, {
            status: MessageStatus.COMPLETED,
          });
          set({ currentStreamingMessageId: null, isLoading: false });
        },

        addToolCall: (messageId, toolCall) => {
          set((state) => ({
            messages: state.messages.map((msg) =>
              msg.id === messageId
                ? {
                    ...msg,
                    toolCalls: [...(msg.toolCalls || []), toolCall],
                  }
                : msg
            ),
          }));
        },

        updateToolCall: (messageId, toolCallId, updates) => {
          set((state) => ({
            messages: state.messages.map((msg) =>
              msg.id === messageId
                ? {
                    ...msg,
                    toolCalls: msg.toolCalls?.map((tc) =>
                      tc.id === toolCallId ? { ...tc, ...updates } : tc
                    ),
                  }
                : msg
            ),
          }));
        },
      }),
      {
        name: 'chat-storage',
        partialize: (state) => ({ messages: state.messages }),
      }
    )
  );
  ```

#### 유틸리티 함수
- [ ] `src/lib/message-utils.ts` 생성
  ```typescript
  import { Message, MessageContent } from '@/types/message';

  export function parseMarkdown(content: string): MessageContent[] {
    // 마크다운 파싱 로직
    // 코드 블록, 이미지, 일반 텍스트 분리
  }

  export function formatTimestamp(timestamp: number): string {
    const date = new Date(timestamp);
    const now = new Date();
    const diff = now.getTime() - date.getTime();

    if (diff < 60000) return 'Just now';
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;

    return date.toLocaleDateString();
  }

  export function groupMessagesByDate(messages: Message[]) {
    // 날짜별로 메시지 그룹화
  }
  ```

### 예상 결과물
- 타입 안전한 메시지 데이터 구조
- 전역 상태 관리 스토어
- 스트리밍 메시지 지원
- 도구 호출 상태 추적

### 테스트
- [ ] 스토어 액션 단위 테스트 작성
  ```typescript
  // src/store/__tests__/chat-store.test.ts
  describe('ChatStore', () => {
    it('should add message', () => { ... });
    it('should handle streaming', () => { ... });
    it('should update tool calls', () => { ... });
  });
  ```

### Commit 메시지
```
feat(web-ui): implement message state management

- Define message types with role, status, and tool calls
- Create Zustand store for chat state
- Add streaming message support
- Implement tool call tracking
- Add message utility functions
```

---

## 2. 채팅 UI 컴포넌트 (Commit 8)

### 요구사항
- 터미널 스타일의 채팅 인터페이스
- 사용자/에이전트 메시지 구분
- 코드 블록 하이라이팅
- 자동 스크롤 및 스크롤 제어

### 작업 내용

#### MessageList 컴포넌트
- [ ] `src/components/chat/MessageList.tsx` 생성
  ```typescript
  import { useEffect, useRef } from 'react';
  import { useChatStore } from '@/store/chat-store';
  import { MessageItem } from './MessageItem';
  import { ScrollArea } from '@/components/ui/scroll-area';

  export function MessageList() {
    const messages = useChatStore((state) => state.messages);
    const scrollRef = useRef<HTMLDivElement>(null);
    const [autoScroll, setAutoScroll] = useState(true);

    useEffect(() => {
      if (autoScroll && scrollRef.current) {
        scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
      }
    }, [messages, autoScroll]);

    const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
      const { scrollTop, scrollHeight, clientHeight } = e.currentTarget;
      const isAtBottom = scrollHeight - scrollTop - clientHeight < 50;
      setAutoScroll(isAtBottom);
    };

    return (
      <ScrollArea ref={scrollRef} onScroll={handleScroll} className="flex-1">
        <div className="space-y-4 p-4">
          {messages.map((message) => (
            <MessageItem key={message.id} message={message} />
          ))}
          {!autoScroll && (
            <button
              onClick={() => setAutoScroll(true)}
              className="fixed bottom-20 right-8 p-2 rounded-full bg-primary text-white"
            >
              ↓ Scroll to bottom
            </button>
          )}
        </div>
      </ScrollArea>
    );
  }
  ```

#### MessageItem 컴포넌트
- [ ] `src/components/chat/MessageItem.tsx` 생성
  ```typescript
  import { Message, MessageRole } from '@/types/message';
  import { Avatar } from '@/components/ui/avatar';
  import { CodeBlock } from './CodeBlock';
  import { ToolCallDisplay } from './ToolCallDisplay';
  import { formatTimestamp } from '@/lib/message-utils';
  import { User, Bot } from 'lucide-react';

  interface MessageItemProps {
    message: Message;
  }

  export function MessageItem({ message }: MessageItemProps) {
    const isUser = message.role === MessageRole.USER;

    return (
      <div className={cn(
        'flex gap-3 group',
        isUser ? 'justify-end' : 'justify-start'
      )}>
        {!isUser && (
          <Avatar className="w-8 h-8 bg-primary">
            <Bot className="w-5 h-5" />
          </Avatar>
        )}

        <div className={cn(
          'flex flex-col max-w-[80%]',
          isUser && 'items-end'
        )}>
          <div className={cn(
            'rounded-lg px-4 py-2',
            isUser
              ? 'bg-primary text-primary-foreground'
              : 'bg-muted'
          )}>
            {message.content.map((content, idx) => (
              <div key={idx}>
                {content.type === 'text' && (
                  <div className="prose dark:prose-invert">
                    {content.content}
                  </div>
                )}
                {content.type === 'code' && (
                  <CodeBlock
                    code={content.content}
                    language={content.language || 'plaintext'}
                  />
                )}
              </div>
            ))}
          </div>

          {message.toolCalls && message.toolCalls.length > 0 && (
            <div className="mt-2 space-y-2">
              {message.toolCalls.map((toolCall) => (
                <ToolCallDisplay key={toolCall.id} toolCall={toolCall} />
              ))}
            </div>
          )}

          <div className="flex items-center gap-2 mt-1 text-xs text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity">
            <span>{formatTimestamp(message.timestamp)}</span>
            {message.status === 'streaming' && (
              <span className="flex items-center gap-1">
                <span className="animate-pulse">●</span> Typing...
              </span>
            )}
          </div>
        </div>

        {isUser && (
          <Avatar className="w-8 h-8 bg-secondary">
            <User className="w-5 h-5" />
          </Avatar>
        )}
      </div>
    );
  }
  ```

#### CodeBlock 컴포넌트
- [ ] `src/components/chat/CodeBlock.tsx` 생성
  ```typescript
  import { useState } from 'react';
  import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
  import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism';
  import { Button } from '@/components/ui/button';
  import { Copy, Check } from 'lucide-react';

  interface CodeBlockProps {
    code: string;
    language: string;
  }

  export function CodeBlock({ code, language }: CodeBlockProps) {
    const [copied, setCopied] = useState(false);

    const handleCopy = async () => {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    };

    return (
      <div className="relative group my-2">
        <div className="flex items-center justify-between px-4 py-2 bg-zinc-800 rounded-t-lg">
          <span className="text-xs text-zinc-400">{language}</span>
          <Button
            size="sm"
            variant="ghost"
            onClick={handleCopy}
            className="h-6 text-xs"
          >
            {copied ? (
              <>
                <Check className="w-3 h-3 mr-1" />
                Copied
              </>
            ) : (
              <>
                <Copy className="w-3 h-3 mr-1" />
                Copy
              </>
            )}
          </Button>
        </div>
        <SyntaxHighlighter
          language={language}
          style={oneDark}
          customStyle={{
            margin: 0,
            borderTopLeftRadius: 0,
            borderTopRightRadius: 0,
          }}
        >
          {code}
        </SyntaxHighlighter>
      </div>
    );
  }
  ```

#### MessageInput 컴포넌트
- [ ] `src/components/chat/MessageInput.tsx` 생성
  ```typescript
  import { useState, useRef } from 'react';
  import { Textarea } from '@/components/ui/textarea';
  import { Button } from '@/components/ui/button';
  import { Send, Paperclip, Square } from 'lucide-react';
  import { useChatStore } from '@/store/chat-store';

  export function MessageInput() {
    const [input, setInput] = useState('');
    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const isLoading = useChatStore((state) => state.isLoading);

    const handleSubmit = () => {
      if (!input.trim() || isLoading) return;

      // 메시지 전송 로직 (다음 커밋에서 구현)
      console.log('Sending message:', input);

      setInput('');
      textareaRef.current?.focus();
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSubmit();
      }
    };

    return (
      <div className="border-t bg-background p-4">
        <div className="flex items-end gap-2">
          <Button
            size="icon"
            variant="ghost"
            className="mb-2"
          >
            <Paperclip className="w-5 h-5" />
          </Button>

          <div className="flex-1">
            <Textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Type a message... (Enter to send, Shift+Enter for new line)"
              className="min-h-[60px] max-h-[200px] resize-none"
              disabled={isLoading}
            />
          </div>

          <Button
            size="icon"
            onClick={handleSubmit}
            disabled={!input.trim() || isLoading}
            className="mb-2"
          >
            {isLoading ? (
              <Square className="w-5 h-5" />
            ) : (
              <Send className="w-5 h-5" />
            )}
          </Button>
        </div>

        <div className="flex items-center justify-between mt-2 text-xs text-muted-foreground">
          <div className="flex items-center gap-2">
            <kbd className="px-2 py-1 bg-muted rounded">Enter</kbd>
            <span>to send</span>
            <kbd className="px-2 py-1 bg-muted rounded">Shift+Enter</kbd>
            <span>for new line</span>
          </div>
          <span>{input.length} / 4000</span>
        </div>
      </div>
    );
  }
  ```

#### TypingIndicator 컴포넌트
- [ ] `src/components/chat/TypingIndicator.tsx` 생성
  ```typescript
  export function TypingIndicator() {
    return (
      <div className="flex items-center gap-2 p-4">
        <div className="flex gap-1">
          <span className="w-2 h-2 bg-primary rounded-full animate-bounce [animation-delay:-0.3s]" />
          <span className="w-2 h-2 bg-primary rounded-full animate-bounce [animation-delay:-0.15s]" />
          <span className="w-2 h-2 bg-primary rounded-full animate-bounce" />
        </div>
        <span className="text-sm text-muted-foreground">
          Codex is thinking...
        </span>
      </div>
    );
  }
  ```

#### 필수 의존성 설치
- [ ] 코드 하이라이팅 라이브러리 설치
  ```bash
  pnpm add react-syntax-highlighter
  pnpm add -D @types/react-syntax-highlighter
  ```

- [ ] 마크다운 렌더링 라이브러리 설치 (다음 커밋용)
  ```bash
  pnpm add react-markdown remark-gfm rehype-raw
  ```

### 예상 결과물
- 완전히 작동하는 채팅 인터페이스
- 코드 블록 복사 기능
- 자동 스크롤 및 수동 제어
- 타이핑 인디케이터

### Commit 메시지
```
feat(web-ui): create chat UI components

- Implement MessageList with auto-scroll
- Create MessageItem with user/assistant styles
- Add CodeBlock with syntax highlighting and copy button
- Build MessageInput with keyboard shortcuts
- Add TypingIndicator component
- Install react-syntax-highlighter
```

---

## 3. WebSocket 통신 구현 (Commit 9)

### 요구사항
- app-server와의 실시간 양방향 통신
- 자동 재연결 로직
- 연결 상태 관리
- 에러 처리

### 작업 내용

#### WebSocket 클라이언트 구현
- [ ] `src/lib/websocket-client.ts` 생성
  ```typescript
  import { EventEmitter } from 'events';

  export enum WebSocketEvent {
    OPEN = 'open',
    CLOSE = 'close',
    ERROR = 'error',
    MESSAGE = 'message',
    RECONNECTING = 'reconnecting',
  }

  export enum ConnectionStatus {
    CONNECTING = 'connecting',
    CONNECTED = 'connected',
    DISCONNECTED = 'disconnected',
    RECONNECTING = 'reconnecting',
    ERROR = 'error',
  }

  interface WebSocketOptions {
    url: string;
    reconnect?: boolean;
    reconnectInterval?: number;
    maxReconnectAttempts?: number;
    heartbeatInterval?: number;
  }

  export class CodexWebSocket extends EventEmitter {
    private ws: WebSocket | null = null;
    private url: string;
    private reconnect: boolean;
    private reconnectInterval: number;
    private maxReconnectAttempts: number;
    private reconnectAttempts = 0;
    private heartbeatInterval: number;
    private heartbeatTimer: NodeJS.Timeout | null = null;
    private status: ConnectionStatus = ConnectionStatus.DISCONNECTED;

    constructor(options: WebSocketOptions) {
      super();
      this.url = options.url;
      this.reconnect = options.reconnect ?? true;
      this.reconnectInterval = options.reconnectInterval ?? 3000;
      this.maxReconnectAttempts = options.maxReconnectAttempts ?? 5;
      this.heartbeatInterval = options.heartbeatInterval ?? 30000;
    }

    connect() {
      if (this.ws?.readyState === WebSocket.OPEN) {
        console.warn('WebSocket is already connected');
        return;
      }

      this.setStatus(ConnectionStatus.CONNECTING);

      try {
        this.ws = new WebSocket(this.url);
        this.setupEventHandlers();
      } catch (error) {
        this.handleError(error as Error);
      }
    }

    private setupEventHandlers() {
      if (!this.ws) return;

      this.ws.onopen = () => {
        console.log('WebSocket connected');
        this.setStatus(ConnectionStatus.CONNECTED);
        this.reconnectAttempts = 0;
        this.startHeartbeat();
        this.emit(WebSocketEvent.OPEN);
      };

      this.ws.onclose = (event) => {
        console.log('WebSocket closed', event.code, event.reason);
        this.setStatus(ConnectionStatus.DISCONNECTED);
        this.stopHeartbeat();
        this.emit(WebSocketEvent.CLOSE, event);

        if (this.reconnect && this.reconnectAttempts < this.maxReconnectAttempts) {
          this.attemptReconnect();
        }
      };

      this.ws.onerror = (error) => {
        console.error('WebSocket error', error);
        this.handleError(new Error('WebSocket error'));
      };

      this.ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          this.emit(WebSocketEvent.MESSAGE, data);
        } catch (error) {
          console.error('Failed to parse message', error);
        }
      };
    }

    private attemptReconnect() {
      this.reconnectAttempts++;
      this.setStatus(ConnectionStatus.RECONNECTING);
      this.emit(WebSocketEvent.RECONNECTING, this.reconnectAttempts);

      console.log(
        `Reconnecting... Attempt ${this.reconnectAttempts}/${this.maxReconnectAttempts}`
      );

      setTimeout(() => {
        this.connect();
      }, this.reconnectInterval * this.reconnectAttempts);
    }

    private startHeartbeat() {
      this.stopHeartbeat();
      this.heartbeatTimer = setInterval(() => {
        if (this.ws?.readyState === WebSocket.OPEN) {
          this.send({ type: 'ping' });
        }
      }, this.heartbeatInterval);
    }

    private stopHeartbeat() {
      if (this.heartbeatTimer) {
        clearInterval(this.heartbeatTimer);
        this.heartbeatTimer = null;
      }
    }

    send(data: any) {
      if (this.ws?.readyState !== WebSocket.OPEN) {
        console.error('WebSocket is not connected');
        return false;
      }

      try {
        this.ws.send(JSON.stringify(data));
        return true;
      } catch (error) {
        console.error('Failed to send message', error);
        return false;
      }
    }

    disconnect() {
      this.reconnect = false;
      this.stopHeartbeat();

      if (this.ws) {
        this.ws.close();
        this.ws = null;
      }
    }

    private handleError(error: Error) {
      this.setStatus(ConnectionStatus.ERROR);
      this.emit(WebSocketEvent.ERROR, error);
    }

    private setStatus(status: ConnectionStatus) {
      this.status = status;
    }

    getStatus(): ConnectionStatus {
      return this.status;
    }

    isConnected(): boolean {
      return this.status === ConnectionStatus.CONNECTED;
    }
  }
  ```

#### WebSocket 훅 구현
- [ ] `src/hooks/useWebSocket.ts` 생성
  ```typescript
  import { useEffect, useState, useCallback } from 'react';
  import { CodexWebSocket, WebSocketEvent, ConnectionStatus } from '@/lib/websocket-client';

  let wsInstance: CodexWebSocket | null = null;

  export function useWebSocket(url: string) {
    const [status, setStatus] = useState<ConnectionStatus>(ConnectionStatus.DISCONNECTED);
    const [lastMessage, setLastMessage] = useState<any>(null);

    useEffect(() => {
      // 싱글톤 패턴으로 하나의 WebSocket 인스턴스만 유지
      if (!wsInstance) {
        wsInstance = new CodexWebSocket({
          url,
          reconnect: true,
          maxReconnectAttempts: 5,
        });
      }

      const ws = wsInstance;

      const handleOpen = () => setStatus(ConnectionStatus.CONNECTED);
      const handleClose = () => setStatus(ConnectionStatus.DISCONNECTED);
      const handleError = () => setStatus(ConnectionStatus.ERROR);
      const handleReconnecting = () => setStatus(ConnectionStatus.RECONNECTING);
      const handleMessage = (data: any) => setLastMessage(data);

      ws.on(WebSocketEvent.OPEN, handleOpen);
      ws.on(WebSocketEvent.CLOSE, handleClose);
      ws.on(WebSocketEvent.ERROR, handleError);
      ws.on(WebSocketEvent.RECONNECTING, handleReconnecting);
      ws.on(WebSocketEvent.MESSAGE, handleMessage);

      ws.connect();

      return () => {
        ws.off(WebSocketEvent.OPEN, handleOpen);
        ws.off(WebSocketEvent.CLOSE, handleClose);
        ws.off(WebSocketEvent.ERROR, handleError);
        ws.off(WebSocketEvent.RECONNECTING, handleReconnecting);
        ws.off(WebSocketEvent.MESSAGE, handleMessage);
      };
    }, [url]);

    const sendMessage = useCallback((data: any) => {
      return wsInstance?.send(data) ?? false;
    }, []);

    const disconnect = useCallback(() => {
      wsInstance?.disconnect();
      wsInstance = null;
    }, []);

    return {
      status,
      lastMessage,
      sendMessage,
      disconnect,
      isConnected: status === ConnectionStatus.CONNECTED,
    };
  }
  ```

#### 연결 상태 표시 UI
- [ ] `src/components/chat/ConnectionStatus.tsx` 생성
  ```typescript
  import { ConnectionStatus } from '@/lib/websocket-client';
  import { Wifi, WifiOff, Loader2 } from 'lucide-react';
  import { cn } from '@/lib/utils';

  interface ConnectionStatusProps {
    status: ConnectionStatus;
  }

  export function ConnectionStatusIndicator({ status }: ConnectionStatusProps) {
    const statusConfig = {
      [ConnectionStatus.CONNECTED]: {
        icon: Wifi,
        color: 'text-green-500',
        label: 'Connected',
      },
      [ConnectionStatus.CONNECTING]: {
        icon: Loader2,
        color: 'text-yellow-500',
        label: 'Connecting...',
        spin: true,
      },
      [ConnectionStatus.DISCONNECTED]: {
        icon: WifiOff,
        color: 'text-red-500',
        label: 'Disconnected',
      },
      [ConnectionStatus.RECONNECTING]: {
        icon: Loader2,
        color: 'text-orange-500',
        label: 'Reconnecting...',
        spin: true,
      },
      [ConnectionStatus.ERROR]: {
        icon: WifiOff,
        color: 'text-red-600',
        label: 'Connection Error',
      },
    };

    const config = statusConfig[status];
    const Icon = config.icon;

    return (
      <div className="flex items-center gap-2 px-3 py-1 rounded-full bg-muted">
        <Icon
          className={cn('w-4 h-4', config.color, config.spin && 'animate-spin')}
        />
        <span className="text-xs">{config.label}</span>
      </div>
    );
  }
  ```

#### 메시지 전송 로직 통합
- [ ] `src/components/chat/MessageInput.tsx` 업데이트
  ```typescript
  // handleSubmit 함수 수정
  const { sendMessage, isConnected } = useWebSocket(WS_URL);

  const handleSubmit = () => {
    if (!input.trim() || isLoading || !isConnected) return;

    const userMessage = addMessage({
      role: MessageRole.USER,
      content: [{ type: 'text', content: input }],
      status: MessageStatus.COMPLETED,
    });

    sendMessage({
      type: 'user_message',
      content: input,
      messageId: userMessage.id,
    });

    setInput('');
  };
  ```

### 예상 결과물
- 안정적인 WebSocket 연결
- 자동 재연결 기능
- 실시간 연결 상태 표시
- 메시지 송수신 가능

### Commit 메시지
```
feat(web-ui): implement WebSocket communication

- Create CodexWebSocket client with auto-reconnect
- Implement useWebSocket hook
- Add connection status indicator
- Integrate message sending via WebSocket
- Handle connection errors and reconnection
```

---

## 4. 스트리밍 응답 처리 (Commit 10)

### 요구사항
- 실시간 스트리밍 응답 표시
- 마크다운 렌더링
- 코드 블록 실시간 하이라이팅
- 부드러운 타이핑 애니메이션

### 작업 내용

#### 스트리밍 메시지 핸들러
- [ ] `src/hooks/useChatStream.ts` 생성
  ```typescript
  import { useEffect } from 'use';
  import { useWebSocket } from './useWebSocket';
  import { useChatStore } from '@/store/chat-store';
  import { MessageRole, MessageStatus } from '@/types/message';

  export function useChatStream() {
    const { lastMessage } = useWebSocket(WS_URL);
    const {
      addMessage,
      startStreaming,
      appendToStreamingMessage,
      completeStreaming,
      addToolCall,
      updateToolCall,
    } = useChatStore();

    useEffect(() => {
      if (!lastMessage) return;

      const { type, data } = lastMessage;

      switch (type) {
        case 'response_start':
          // 새 에이전트 메시지 시작
          const assistantMessage = addMessage({
            role: MessageRole.ASSISTANT,
            content: [{ type: 'text', content: '' }],
            status: MessageStatus.STREAMING,
          });
          startStreaming(assistantMessage.id);
          break;

        case 'response_chunk':
          // 스트리밍 청크 추가
          appendToStreamingMessage(data.content);
          break;

        case 'response_end':
          // 스트리밍 완료
          completeStreaming();
          break;

        case 'tool_call_start':
          // 도구 호출 시작
          addToolCall(data.messageId, {
            id: data.toolCallId,
            name: data.toolName,
            arguments: data.arguments,
            status: 'running',
            timestamp: Date.now(),
          });
          break;

        case 'tool_call_result':
          // 도구 호출 결과
          updateToolCall(data.messageId, data.toolCallId, {
            status: 'completed',
            result: data.result,
          });
          break;

        case 'tool_call_error':
          // 도구 호출 에러
          updateToolCall(data.messageId, data.toolCallId, {
            status: 'failed',
            error: data.error,
          });
          break;

        case 'error':
          // 에러 처리
          console.error('Stream error:', data);
          completeStreaming();
          break;
      }
    }, [lastMessage]);
  }
  ```

#### 마크다운 렌더링 컴포넌트
- [ ] `src/components/chat/MarkdownContent.tsx` 생성
  ```typescript
  import ReactMarkdown from 'react-markdown';
  import remarkGfm from 'remark-gfm';
  import rehypeRaw from 'rehype-raw';
  import { CodeBlock } from './CodeBlock';

  interface MarkdownContentProps {
    content: string;
  }

  export function MarkdownContent({ content }: MarkdownContentProps) {
    return (
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeRaw]}
        className="prose dark:prose-invert max-w-none"
        components={{
          code({ node, inline, className, children, ...props }) {
            const match = /language-(\w+)/.exec(className || '');
            const language = match ? match[1] : 'plaintext';
            const code = String(children).replace(/\n$/, '');

            return !inline ? (
              <CodeBlock code={code} language={language} />
            ) : (
              <code className="px-1.5 py-0.5 rounded bg-muted font-mono text-sm" {...props}>
                {children}
              </code>
            );
          },
          a({ href, children }) {
            return (
              <a
                href={href}
                target="_blank"
                rel="noopener noreferrer"
                className="text-primary underline hover:no-underline"
              >
                {children}
              </a>
            );
          },
          table({ children }) {
            return (
              <div className="overflow-x-auto my-4">
                <table className="w-full border-collapse">{children}</table>
              </div>
            );
          },
        }}
      >
        {content}
      </ReactMarkdown>
    );
  }
  ```

#### MessageItem 컴포넌트 업데이트
- [ ] `src/components/chat/MessageItem.tsx` 수정
  ```typescript
  // content 렌더링 부분 수정
  {content.type === 'text' && (
    <MarkdownContent content={content.content} />
  )}
  ```

#### 스트리밍 애니메이션
- [ ] `src/components/chat/StreamingCursor.tsx` 생성
  ```typescript
  export function StreamingCursor() {
    return (
      <span className="inline-block w-1 h-4 ml-1 bg-current animate-pulse" />
    );
  }
  ```

- [ ] MessageItem에 커서 추가
  ```typescript
  {message.status === MessageStatus.STREAMING && <StreamingCursor />}
  ```

#### ChatPage 통합
- [ ] `src/pages/ChatPage.tsx` 업데이트
  ```typescript
  import { MessageList } from '@/components/chat/MessageList';
  import { MessageInput } from '@/components/chat/MessageInput';
  import { ConnectionStatusIndicator } from '@/components/chat/ConnectionStatus';
  import { useChatStream } from '@/hooks/useChatStream';
  import { useWebSocket } from '@/hooks/useWebSocket';

  export function ChatPage() {
    const { status } = useWebSocket(WS_URL);
    useChatStream(); // 스트리밍 처리 활성화

    return (
      <div className="flex flex-col h-screen">
        <header className="flex items-center justify-between p-4 border-b">
          <h1 className="text-xl font-bold">Codex Chat</h1>
          <ConnectionStatusIndicator status={status} />
        </header>

        <MessageList />
        <MessageInput />
      </div>
    );
  }
  ```

### 예상 결과물
- 실시간 스트리밍 응답 표시
- 마크다운 포맷팅
- 코드 블록 하이라이팅
- 부드러운 타이핑 효과

### Commit 메시지
```
feat(web-ui): add streaming response handling

- Implement useChatStream hook for real-time updates
- Create MarkdownContent component with syntax highlighting
- Add streaming cursor animation
- Integrate markdown rendering in MessageItem
- Handle tool calls in streaming context
```

---

## 5. 메시지 기능 개선 (Commit 11)

### 요구사항
- 메시지 편집 및 삭제
- 메시지 검색
- 메시지 내보내기
- 메시지 재전송

### 작업 내용

#### 메시지 액션 메뉴
- [ ] `src/components/chat/MessageActions.tsx` 생성
  ```typescript
  import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
  } from '@/components/ui/dropdown-menu';
  import { Button } from '@/components/ui/button';
  import { MoreVertical, Copy, Edit, Trash, RotateCcw } from 'lucide-react';

  interface MessageActionsProps {
    messageId: string;
    content: string;
    onEdit?: () => void;
    onDelete?: () => void;
    onRetry?: () => void;
  }

  export function MessageActions({
    messageId,
    content,
    onEdit,
    onDelete,
    onRetry,
  }: MessageActionsProps) {
    const handleCopy = async () => {
      await navigator.clipboard.writeText(content);
      toast({ title: 'Copied to clipboard' });
    };

    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            size="icon"
            variant="ghost"
            className="h-6 w-6 opacity-0 group-hover:opacity-100"
          >
            <MoreVertical className="h-4 w-4" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem onClick={handleCopy}>
            <Copy className="h-4 w-4 mr-2" />
            Copy
          </DropdownMenuItem>
          {onEdit && (
            <DropdownMenuItem onClick={onEdit}>
              <Edit className="h-4 w-4 mr-2" />
              Edit
            </DropdownMenuItem>
          )}
          {onRetry && (
            <DropdownMenuItem onClick={onRetry}>
              <RotateCcw className="h-4 w-4 mr-2" />
              Retry
            </DropdownMenuItem>
          )}
          {onDelete && (
            <DropdownMenuItem onClick={onDelete} className="text-destructive">
              <Trash className="h-4 w-4 mr-2" />
              Delete
            </DropdownMenuItem>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
    );
  }
  ```

#### 메시지 검색 기능
- [ ] `src/components/chat/MessageSearch.tsx` 생성
  ```typescript
  import { useState } from 'react';
  import { Input } from '@/components/ui/input';
  import { Button } from '@/components/ui/button';
  import { Search, X } from 'lucide-react';
  import { useChatStore } from '@/store/chat-store';

  export function MessageSearch() {
    const [query, setQuery] = useState('');
    const [isOpen, setIsOpen] = useState(false);
    const messages = useChatStore((state) => state.messages);

    const results = messages.filter((msg) =>
      msg.content.some((c) =>
        c.content.toLowerCase().includes(query.toLowerCase())
      )
    );

    if (!isOpen) {
      return (
        <Button
          size="icon"
          variant="ghost"
          onClick={() => setIsOpen(true)}
        >
          <Search className="h-5 w-5" />
        </Button>
      );
    }

    return (
      <div className="flex items-center gap-2 px-4 py-2 border-b">
        <Search className="h-4 w-4 text-muted-foreground" />
        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search messages..."
          className="border-0 focus-visible:ring-0"
          autoFocus
        />
        <span className="text-sm text-muted-foreground whitespace-nowrap">
          {results.length} results
        </span>
        <Button
          size="icon"
          variant="ghost"
          onClick={() => {
            setIsOpen(false);
            setQuery('');
          }}
        >
          <X className="h-4 w-4" />
        </Button>
      </div>
    );
  }
  ```

#### 메시지 내보내기
- [ ] `src/lib/export-utils.ts` 생성
  ```typescript
  import { Message, MessageRole } from '@/types/message';

  export function exportMessagesToMarkdown(messages: Message[]): string {
    let markdown = '# Chat Export\n\n';
    markdown += `Generated on ${new Date().toLocaleString()}\n\n`;
    markdown += '---\n\n';

    messages.forEach((msg) => {
      const role = msg.role === MessageRole.USER ? '**You**' : '**Codex**';
      markdown += `## ${role}\n\n`;

      msg.content.forEach((content) => {
        if (content.type === 'text') {
          markdown += `${content.content}\n\n`;
        } else if (content.type === 'code') {
          markdown += `\`\`\`${content.language || ''}\n${content.content}\n\`\`\`\n\n`;
        }
      });

      if (msg.toolCalls && msg.toolCalls.length > 0) {
        markdown += '### Tool Calls\n\n';
        msg.toolCalls.forEach((tc) => {
          markdown += `- **${tc.name}**: ${tc.status}\n`;
          if (tc.result) {
            markdown += `  \`\`\`\n  ${tc.result}\n  \`\`\`\n`;
          }
        });
        markdown += '\n';
      }

      markdown += '---\n\n';
    });

    return markdown;
  }

  export function downloadAsFile(content: string, filename: string) {
    const blob = new Blob([content], { type: 'text/markdown' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
  }
  ```

- [ ] 내보내기 버튼 추가
  ```typescript
  const handleExport = () => {
    const markdown = exportMessagesToMarkdown(messages);
    downloadAsFile(markdown, `chat-${Date.now()}.md`);
  };
  ```

### 예상 결과물
- 메시지 컨텍스트 메뉴
- 검색 기능
- 마크다운 내보내기

### Commit 메시지
```
feat(web-ui): enhance message functionality

- Add MessageActions dropdown menu
- Implement message search with real-time filtering
- Create export to Markdown functionality
- Add message copy, edit, delete, retry actions
```

---

## 6. 에러 처리 및 알림 (Commit 12)

### 요구사항
- 사용자 친화적인 에러 메시지
- Toast 알림 시스템
- 네트워크 에러 처리
- 재시도 메커니즘

### 작업 내용

#### Toast 알림 시스템
- [ ] shadcn/ui toast 설치
  ```bash
  npx shadcn@latest add toast
  npx shadcn@latest add sonner
  ```

- [ ] `src/lib/toast.ts` 생성
  ```typescript
  import { toast as sonnerToast } from 'sonner';

  export const toast = {
    success: (message: string, description?: string) => {
      sonnerToast.success(message, { description });
    },
    error: (message: string, description?: string) => {
      sonnerToast.error(message, { description });
    },
    info: (message: string, description?: string) => {
      sonnerToast.info(message, { description });
    },
    warning: (message: string, description?: string) => {
      sonnerToast.warning(message, { description });
    },
  };
  ```

#### 에러 타입 정의
- [ ] `src/types/error.ts` 생성
  ```typescript
  export enum ErrorCode {
    NETWORK_ERROR = 'NETWORK_ERROR',
    AUTHENTICATION_ERROR = 'AUTHENTICATION_ERROR',
    RATE_LIMIT_ERROR = 'RATE_LIMIT_ERROR',
    SERVER_ERROR = 'SERVER_ERROR',
    VALIDATION_ERROR = 'VALIDATION_ERROR',
    UNKNOWN_ERROR = 'UNKNOWN_ERROR',
  }

  export interface CodexError {
    code: ErrorCode;
    message: string;
    details?: any;
    retryable?: boolean;
  }

  export function createError(
    code: ErrorCode,
    message: string,
    details?: any
  ): CodexError {
    return {
      code,
      message,
      details,
      retryable: [
        ErrorCode.NETWORK_ERROR,
        ErrorCode.SERVER_ERROR,
      ].includes(code),
    };
  }
  ```

#### 에러 핸들러
- [ ] `src/lib/error-handler.ts` 생성
  ```typescript
  import { CodexError, ErrorCode, createError } from '@/types/error';
  import { toast } from './toast';

  export function handleError(error: unknown): CodexError {
    let codexError: CodexError;

    if (error instanceof Error) {
      if (error.message.includes('network')) {
        codexError = createError(
          ErrorCode.NETWORK_ERROR,
          'Network connection failed. Please check your internet connection.',
          error
        );
      } else if (error.message.includes('401') || error.message.includes('403')) {
        codexError = createError(
          ErrorCode.AUTHENTICATION_ERROR,
          'Authentication failed. Please log in again.',
          error
        );
      } else if (error.message.includes('429')) {
        codexError = createError(
          ErrorCode.RATE_LIMIT_ERROR,
          'Rate limit exceeded. Please try again later.',
          error
        );
      } else if (error.message.includes('500') || error.message.includes('502')) {
        codexError = createError(
          ErrorCode.SERVER_ERROR,
          'Server error occurred. Please try again.',
          error
        );
      } else {
        codexError = createError(
          ErrorCode.UNKNOWN_ERROR,
          error.message || 'An unknown error occurred',
          error
        );
      }
    } else {
      codexError = createError(
        ErrorCode.UNKNOWN_ERROR,
        'An unexpected error occurred',
        error
      );
    }

    // 에러 로깅
    console.error('[Codex Error]', codexError);

    // Toast 표시
    toast.error(codexError.message);

    return codexError;
  }

  export async function withErrorHandling<T>(
    fn: () => Promise<T>,
    options?: {
      onError?: (error: CodexError) => void;
      silent?: boolean;
    }
  ): Promise<T | null> {
    try {
      return await fn();
    } catch (error) {
      const codexError = handleError(error);
      options?.onError?.(codexError);
      if (!options?.silent) {
        toast.error(codexError.message);
      }
      return null;
    }
  }
  ```

#### 에러 바운더리
- [ ] `src/components/ErrorBoundary.tsx` 생성
  ```typescript
  import { Component, ReactNode } from 'react';
  import { AlertTriangle } from 'lucide-react';
  import { Button } from './ui/button';

  interface Props {
    children: ReactNode;
    fallback?: ReactNode;
  }

  interface State {
    hasError: boolean;
    error?: Error;
  }

  export class ErrorBoundary extends Component<Props, State> {
    constructor(props: Props) {
      super(props);
      this.state = { hasError: false };
    }

    static getDerivedStateFromError(error: Error): State {
      return { hasError: true, error };
    }

    componentDidCatch(error: Error, errorInfo: any) {
      console.error('ErrorBoundary caught:', error, errorInfo);
    }

    render() {
      if (this.state.hasError) {
        return (
          this.props.fallback || (
            <div className="flex flex-col items-center justify-center h-screen p-8">
              <AlertTriangle className="w-16 h-16 text-destructive mb-4" />
              <h1 className="text-2xl font-bold mb-2">Something went wrong</h1>
              <p className="text-muted-foreground mb-4 text-center max-w-md">
                {this.state.error?.message || 'An unexpected error occurred'}
              </p>
              <Button onClick={() => window.location.reload()}>
                Reload Page
              </Button>
            </div>
          )
        );
      }

      return this.props.children;
    }
  }
  ```

#### 재시도 로직
- [ ] `src/hooks/useRetry.ts` 생성
  ```typescript
  import { useState } from 'react';

  interface RetryOptions {
    maxAttempts?: number;
    delay?: number;
    backoff?: boolean;
  }

  export function useRetry() {
    const [isRetrying, setIsRetrying] = useState(false);
    const [attempts, setAttempts] = useState(0);

    const retry = async <T>(
      fn: () => Promise<T>,
      options: RetryOptions = {}
    ): Promise<T> => {
      const { maxAttempts = 3, delay = 1000, backoff = true } = options;

      setIsRetrying(true);
      let lastError: any;

      for (let i = 0; i < maxAttempts; i++) {
        try {
          setAttempts(i + 1);
          const result = await fn();
          setIsRetrying(false);
          setAttempts(0);
          return result;
        } catch (error) {
          lastError = error;
          if (i < maxAttempts - 1) {
            const waitTime = backoff ? delay * Math.pow(2, i) : delay;
            await new Promise((resolve) => setTimeout(resolve, waitTime));
          }
        }
      }

      setIsRetrying(false);
      setAttempts(0);
      throw lastError;
    };

    return { retry, isRetrying, attempts };
  }
  ```

#### App.tsx에 통합
- [ ] `src/App.tsx` 업데이트
  ```typescript
  import { ErrorBoundary } from '@/components/ErrorBoundary';
  import { Toaster } from 'sonner';

  function App() {
    return (
      <ErrorBoundary>
        <BrowserRouter>
          {/* routes */}
        </BrowserRouter>
        <Toaster position="top-right" />
      </ErrorBoundary>
    );
  }
  ```

### 예상 결과물
- 통합된 에러 처리 시스템
- Toast 알림
- 에러 바운더리
- 재시도 메커니즘

### Commit 메시지
```
feat(web-ui): implement error handling and notifications

- Add toast notification system with Sonner
- Create error types and error handler
- Implement ErrorBoundary component
- Add retry mechanism with exponential backoff
- Integrate error handling across the app
```

---

## Day 2 완료 체크리스트

- [ ] Zustand 메시지 상태 관리 구현
- [ ] 채팅 UI 컴포넌트 (MessageList, MessageItem, MessageInput)
- [ ] WebSocket 클라이언트 및 연결 관리
- [ ] 실시간 스트리밍 응답 처리
- [ ] 마크다운 렌더링 및 코드 하이라이팅
- [ ] 메시지 검색, 복사, 내보내기 기능
- [ ] 에러 처리 및 Toast 알림 시스템
- [ ] 모든 커밋 메시지 명확하게 작성
- [ ] 기능 테스트 및 검증

---

## 다음 단계 (Day 3 예고)

1. 파일 탐색기 UI 구현
2. 파일 뷰어 및 코드 에디터 통합
3. 파일 업로드/다운로드
4. 도구 호출 시각화 컴포넌트
5. 파일 diff 뷰어
6. 승인 플로우 UI

---

## 참고 자료

- [Zustand 문서](https://docs.pmnd.rs/zustand)
- [React Markdown](https://github.com/remarkjs/react-markdown)
- [React Syntax Highlighter](https://github.com/react-syntax-highlighter/react-syntax-highlighter)
- [WebSocket API MDN](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket)
- [Sonner Toast](https://sonner.emilkowal.ski/)

---

**Last Updated**: 2025-11-20
**Version**: 1.0
**Day**: 2 / 7
