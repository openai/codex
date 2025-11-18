# Codex Electron UI - 아키텍처 설계

> Codex CLI의 샌드박스 보안을 유지하면서 향상된 데스크톱 UI 제공

## 목차

- [개요](#개요)
- [현재 아키텍처 분석](#현재-아키텍처-분석)
- [제안하는 아키텍처](#제안하는-아키텍처)
- [주요 컴포넌트](#주요-컴포넌트)
- [통신 프로토콜](#통신-프로토콜)
- [보안 및 샌드박싱](#보안-및-샌드박싱)
- [디렉토리 구조](#디렉토리-구조)
- [기술 스택](#기술-스택)
- [데이터 흐름](#데이터-흐름)
- [마이그레이션 전략](#마이그레이션-전략)
- [확장성 고려사항](#확장성-고려사항)

---

## 개요

### 목표

Codex CLI의 강력한 샌드박스 보안과 Rust 기반 백엔드를 유지하면서, Electron을 활용한 현대적인 데스크톱 UI를 제공합니다.

### 핵심 원칙

1. **보안 우선**: 플랫폼 네이티브 샌드박싱 유지
2. **백엔드 재사용**: 기존 Rust 코드베이스 최대한 활용
3. **프로토콜 기반**: app-server-protocol을 통한 명확한 프론트엔드/백엔드 분리
4. **점진적 마이그레이션**: TUI와 Electron UI 공존 가능
5. **성능**: 네이티브 성능 유지

---

## 현재 아키텍처 분석

### 기존 컴포넌트

```
codex-rs/
├── core/                    # 핵심 에이전트 로직
│   ├── Tool orchestration
│   ├── Model communication
│   └── State management
├── app-server/              # IDE 통합용 백엔드 서버
│   ├── JSON-RPC over stdio
│   ├── Message processing
│   └── Conversation management
├── app-server-protocol/     # 프로토콜 정의
│   ├── v1 (legacy)
│   ├── v2 (current)
│   └── TypeScript 타입 생성
├── protocol/                # 이벤트 프로토콜
│   └── Stream-based events
├── tui/                     # 터미널 UI (Ratatui)
│   ├── Interactive TUI
│   └── Event handling
└── [sandbox components]/    # 플랫폼별 샌드박싱
    ├── linux-sandbox
    ├── process-hardening
    └── windows-sandbox-rs
```

### 현재 통신 방식

**TUI Mode:**
```
User Input → TUI → Core → Model → Core → TUI → Terminal Output
```

**App-Server Mode (IDE):**
```
IDE Extension → JSON-RPC (stdio) → app-server → Core → Model
                                        ↓
                                    Events Stream
                                        ↓
                              JSON-RPC Response → IDE
```

### 재사용 가능한 컴포넌트

✅ **core** - 전체 에이전트 로직
✅ **app-server** - 프로세스 간 통신 백엔드
✅ **app-server-protocol** - 프론트엔드/백엔드 인터페이스
✅ **protocol** - 이벤트 스트리밍
✅ **모든 샌드박싱 컴포넌트**

---

## 제안하는 아키텍처

### 전체 시스템 아키텍처

```
┌─────────────────────────────────────────────────────────┐
│                    Electron Main Process                 │
│  ┌────────────────────────────────────────────────────┐ │
│  │  - Window management                               │ │
│  │  - Menu / system tray                              │ │
│  │  - IPC router                                      │ │
│  │  - Process lifecycle management                    │ │
│  └────────────────────────────────────────────────────┘ │
└───────────────────┬─────────────────────────────────────┘
                    │ IPC (contextBridge)
                    ↓
┌─────────────────────────────────────────────────────────┐
│                  Electron Renderer Process               │
│  ┌────────────────────────────────────────────────────┐ │
│  │  React + TypeScript UI                             │ │
│  │  ┌──────────────────────────────────────────────┐  │ │
│  │  │  - Chat Interface                            │  │ │
│  │  │  - Code Editor (Monaco)                      │  │ │
│  │  │  - File Browser                              │  │ │
│  │  │  - Tool Approval UI                          │  │ │
│  │  │  - Settings Panel                            │  │ │
│  │  │  - Sandbox Status Indicator                  │  │ │
│  │  └──────────────────────────────────────────────┘  │ │
│  └────────────────────────────────────────────────────┘ │
└───────────────────┬─────────────────────────────────────┘
                    │ IPC Messages
                    ↓
┌─────────────────────────────────────────────────────────┐
│              Rust Backend Process (Child)                │
│  ┌────────────────────────────────────────────────────┐ │
│  │  codex-app-server (재사용)                         │ │
│  │  ┌──────────────────────────────────────────────┐  │ │
│  │  │  JSON-RPC Server                             │  │ │
│  │  │  ├─ stdin/stdout communication                │  │ │
│  │  │  ├─ Message processing                        │  │ │
│  │  │  └─ Event streaming                           │  │ │
│  │  └──────────────────────────────────────────────┘  │ │
│  │                      ↓                              │ │
│  │  ┌──────────────────────────────────────────────┐  │ │
│  │  │  codex-core                                   │  │ │
│  │  │  ├─ Tool orchestration                        │  │ │
│  │  │  ├─ Model communication                       │  │ │
│  │  │  ├─ Conversation management                   │  │ │
│  │  │  └─ State management                          │  │ │
│  │  └──────────────────────────────────────────────┘  │ │
│  │                      ↓                              │ │
│  │  ┌──────────────────────────────────────────────┐  │ │
│  │  │  Platform Sandboxing                          │  │ │
│  │  │  ├─ Seatbelt (macOS)                          │  │ │
│  │  │  ├─ Landlock (Linux)                          │  │ │
│  │  │  └─ Restricted Tokens (Windows)               │  │ │
│  │  └──────────────────────────────────────────────┘  │ │
│  └────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────────────┐
│                  External Services                       │
│  - OpenAI API / Azure / Gemini / etc.                   │
│  - MCP Servers                                           │
│  - File System (sandboxed)                               │
└─────────────────────────────────────────────────────────┘
```

### 프로세스 모델

**1. Main Process (Electron)**
- 역할: 애플리케이션 생명주기 관리
- 책임:
  - 창 생성/관리
  - 시스템 메뉴/트레이
  - Rust 백엔드 프로세스 생성 및 감시
  - Renderer ↔ Backend IPC 라우팅
  - 로컬 설정 파일 관리

**2. Renderer Process (Chromium + React)**
- 역할: 사용자 인터페이스
- 책임:
  - 채팅 인터페이스 렌더링
  - 코드 편집기 (Monaco Editor)
  - 파일 브라우저
  - 도구 승인 UI
  - 샌드박스 상태 표시
  - 로컬 상태 관리 (React State/Context)

**3. Backend Process (Rust - app-server)**
- 역할: 핵심 로직 실행
- 책임:
  - AI 모델 통신
  - 도구 실행
  - 샌드박싱
  - 파일 시스템 접근
  - Git 작업
  - MCP 서버 통신

---

## 주요 컴포넌트

### 1. Electron Frontend (`electron-ui/`)

#### 디렉토리 구조

```
electron-ui/
├── package.json
├── tsconfig.json
├── electron-builder.yml
├── main/                           # Main Process
│   ├── index.ts                    # Entry point
│   ├── window.ts                   # Window management
│   ├── backend-manager.ts          # Rust process lifecycle
│   ├── ipc-handlers.ts             # IPC routing
│   ├── menu.ts                     # App menu
│   └── tray.ts                     # System tray
├── preload/                        # Preload Scripts
│   └── index.ts                    # contextBridge API
├── renderer/                       # Renderer Process
│   ├── public/
│   │   └── index.html
│   ├── src/
│   │   ├── main.tsx                # React entry
│   │   ├── App.tsx                 # Root component
│   │   ├── components/             # UI components
│   │   │   ├── chat/
│   │   │   │   ├── ChatContainer.tsx
│   │   │   │   ├── MessageList.tsx
│   │   │   │   ├── MessageItem.tsx
│   │   │   │   ├── InputArea.tsx
│   │   │   │   └── TypingIndicator.tsx
│   │   │   ├── editor/
│   │   │   │   ├── CodeEditor.tsx  # Monaco editor
│   │   │   │   ├── DiffViewer.tsx
│   │   │   │   └── FilePreview.tsx
│   │   │   ├── sidebar/
│   │   │   │   ├── Sidebar.tsx
│   │   │   │   ├── ConversationList.tsx
│   │   │   │   ├── FileExplorer.tsx
│   │   │   │   └── ToolHistory.tsx
│   │   │   ├── approval/
│   │   │   │   ├── ApprovalDialog.tsx
│   │   │   │   ├── ToolPreview.tsx
│   │   │   │   └── DiffPreview.tsx
│   │   │   ├── settings/
│   │   │   │   ├── SettingsPanel.tsx
│   │   │   │   ├── ModelConfig.tsx
│   │   │   │   ├── SandboxConfig.tsx
│   │   │   │   └── KeyboardShortcuts.tsx
│   │   │   └── status/
│   │   │       ├── StatusBar.tsx
│   │   │       ├── SandboxIndicator.tsx
│   │   │       └── ConnectionStatus.tsx
│   │   ├── hooks/                  # Custom hooks
│   │   │   ├── useBackend.ts       # Backend communication
│   │   │   ├── useConversation.ts  # Conversation state
│   │   │   ├── useApproval.ts      # Tool approval
│   │   │   └── useFileSystem.ts    # File operations
│   │   ├── services/               # Business logic
│   │   │   ├── backend.ts          # Backend API client
│   │   │   ├── protocol.ts         # Protocol types
│   │   │   └── events.ts           # Event handling
│   │   ├── store/                  # State management
│   │   │   ├── store.ts            # Redux/Zustand store
│   │   │   ├── slices/
│   │   │   │   ├── conversation.ts
│   │   │   │   ├── files.ts
│   │   │   │   └── settings.ts
│   │   │   └── types.ts
│   │   ├── types/                  # TypeScript types
│   │   │   ├── protocol.ts         # Generated from Rust
│   │   │   └── app.ts              # App-specific types
│   │   ├── utils/                  # Utilities
│   │   │   ├── format.ts
│   │   │   ├── markdown.ts
│   │   │   └── syntax-highlight.ts
│   │   └── styles/                 # Styling
│   │       ├── globals.css
│   │       └── themes/
│   └── vite.config.ts              # Vite config
└── scripts/
    ├── build.js                    # Build scripts
    ├── generate-protocol-types.js  # Type generation
    └── dev.js                      # Development mode
```

#### 핵심 구현

**Backend Manager (`main/backend-manager.ts`)**

```typescript
import { spawn, ChildProcess } from 'child_process';
import { EventEmitter } from 'events';

export class BackendManager extends EventEmitter {
  private process: ChildProcess | null = null;
  private messageQueue: any[] = [];

  async start(): Promise<void> {
    const binaryPath = this.getBackendBinaryPath();

    this.process = spawn(binaryPath, ['app-server'], {
      stdio: ['pipe', 'pipe', 'pipe'],
      env: {
        ...process.env,
        CODEX_MODE: 'electron',
      }
    });

    this.setupStreamHandlers();
    await this.waitForReady();
  }

  sendMessage(message: JSONRPCRequest): Promise<JSONRPCResponse> {
    return new Promise((resolve, reject) => {
      const id = generateId();
      const jsonrpc = { jsonrpc: '2.0', id, ...message };

      this.process!.stdin!.write(JSON.stringify(jsonrpc) + '\n');

      // Handle response...
    });
  }

  private setupStreamHandlers(): void {
    let buffer = '';

    this.process!.stdout!.on('data', (data) => {
      buffer += data.toString();
      const lines = buffer.split('\n');
      buffer = lines.pop() || '';

      for (const line of lines) {
        if (line.trim()) {
          this.handleMessage(JSON.parse(line));
        }
      }
    });

    this.process!.stderr!.on('data', (data) => {
      console.error('Backend error:', data.toString());
    });
  }

  private getBackendBinaryPath(): string {
    // Development vs Production path resolution
    if (process.env.NODE_ENV === 'development') {
      return path.join(__dirname, '../../codex-rs/target/debug/codex-app-server');
    }
    return path.join(process.resourcesPath, 'bin', 'codex-app-server');
  }
}
```

**Backend Hook (`renderer/src/hooks/useBackend.ts`)**

```typescript
import { useState, useEffect, useCallback } from 'react';
import { backendService } from '../services/backend';

export function useBackend() {
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    backendService.on('connected', () => setConnected(true));
    backendService.on('disconnected', () => setConnected(false));
    backendService.on('error', (err) => setError(err));

    return () => {
      backendService.removeAllListeners();
    };
  }, []);

  const sendMessage = useCallback(async (
    conversationId: string,
    message: string,
    onEvent?: (event: Event) => void
  ) => {
    return await backendService.sendMessage({
      method: 'turn/start',
      params: {
        conversation_id: conversationId,
        message,
        stream: true,
      }
    }, onEvent);
  }, []);

  const approveToolUse = useCallback(async (
    conversationId: string,
    approved: boolean
  ) => {
    return await backendService.sendMessage({
      method: 'turn/respond',
      params: {
        conversation_id: conversationId,
        decision: approved ? 'approved' : 'rejected',
      }
    });
  }, []);

  return {
    connected,
    error,
    sendMessage,
    approveToolUse,
  };
}
```

### 2. Backend Integration

기존 `codex-app-server`를 그대로 사용하되, Electron 특화 기능 추가:

**새 crate: `electron-bridge/`**

```rust
// electron-bridge/src/lib.rs
use codex_app_server_protocol::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct ElectronBridge {
    stdin: BufReader<tokio::io::Stdin>,
    stdout: tokio::io::Stdout,
}

impl ElectronBridge {
    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut line = String::new();

        loop {
            line.clear();
            let n = self.stdin.read_line(&mut line).await?;

            if n == 0 {
                break; // EOF
            }

            let request: JSONRPCRequest = serde_json::from_str(&line)?;
            let response = self.handle_request(request).await?;

            let json = serde_json::to_string(&response)?;
            self.stdout.write_all(json.as_bytes()).await?;
            self.stdout.write_all(b"\n").await?;
            self.stdout.flush().await?;
        }

        Ok(())
    }

    async fn handle_request(&mut self, request: JSONRPCRequest)
        -> anyhow::Result<JSONRPCResponse> {
        // Delegate to existing app-server logic
        // Add Electron-specific handlers if needed
    }
}
```

---

## 통신 프로토콜

### IPC 레이어 (Electron Main ↔ Renderer)

**Preload API (`preload/index.ts`)**

```typescript
import { contextBridge, ipcRenderer } from 'electron';

contextBridge.exposeInMainWorld('codexAPI', {
  // Backend communication
  sendMessage: (message: any) => ipcRenderer.invoke('backend:send', message),
  onEvent: (callback: (event: any) => void) => {
    ipcRenderer.on('backend:event', (_, event) => callback(event));
  },

  // File operations
  selectDirectory: () => ipcRenderer.invoke('file:select-directory'),
  readFile: (path: string) => ipcRenderer.invoke('file:read', path),

  // Settings
  getConfig: () => ipcRenderer.invoke('config:get'),
  setConfig: (config: any) => ipcRenderer.invoke('config:set', config),

  // Window operations
  minimize: () => ipcRenderer.send('window:minimize'),
  maximize: () => ipcRenderer.send('window:maximize'),
  close: () => ipcRenderer.send('window:close'),
});
```

### JSON-RPC 프로토콜 (Main ↔ Rust Backend)

기존 `app-server-protocol`을 그대로 사용:

**Request Format:**
```json
{
  "jsonrpc": "2.0",
  "id": "uuid-v4",
  "method": "turn/start",
  "params": {
    "conversation_id": "conv-123",
    "message": "Add authentication to the API",
    "stream": true
  }
}
```

**Response Format:**
```json
{
  "jsonrpc": "2.0",
  "id": "uuid-v4",
  "result": {
    "turn_id": "turn-456",
    "status": "running"
  }
}
```

**Event Stream (Notification):**
```json
{
  "jsonrpc": "2.0",
  "method": "turn/event",
  "params": {
    "conversation_id": "conv-123",
    "turn_id": "turn-456",
    "event": {
      "type": "thinking",
      "content": "I'll add authentication..."
    }
  }
}
```

### 프로토콜 v2 주요 메서드

기존 app-server-protocol v2를 활용:

| 메서드 | 설명 | 필요한 UI 컴포넌트 |
|--------|------|-------------------|
| `thread/start` | 새 대화 시작 | ConversationList |
| `thread/list` | 대화 목록 조회 | Sidebar |
| `thread/resume` | 대화 재개 | ConversationList |
| `thread/archive` | 대화 아카이브 | ConversationList |
| `turn/start` | 메시지 전송 | ChatContainer |
| `turn/interrupt` | 실행 중단 | StatusBar |
| `turn/respond` | 도구 승인/거부 | ApprovalDialog |
| `model/list` | 사용 가능 모델 조회 | SettingsPanel |
| `account/info` | 사용자 정보 | StatusBar |
| `file/search` | 파일 검색 | FileExplorer |

---

## 보안 및 샌드박싱

### 보안 경계

```
┌─────────────────────────────────────────────────┐
│  Electron Renderer (Chromium Sandbox)           │ ← 제한된 권한
│  - No Node.js access                            │
│  - contextBridge만 사용 가능                     │
└────────────────┬────────────────────────────────┘
                 │ IPC (contextBridge API)
┌────────────────┴────────────────────────────────┐
│  Electron Main Process                          │ ← 파일 시스템 접근
│  - Limited file system access                   │
│  - Config file R/W only                         │
│  - Process management                           │
└────────────────┬────────────────────────────────┘
                 │ stdio (JSON-RPC)
┌────────────────┴────────────────────────────────┐
│  Rust Backend (Sandboxed Child Process)         │ ← 플랫폼 샌드박싱
│  ┌──────────────────────────────────────────┐   │
│  │  Platform Sandbox (Seatbelt/Landlock)   │   │
│  │  ├─ Workspace directory: read/write     │   │
│  │  ├─ Home directory: read-only           │   │
│  │  ├─ Network: disabled (optional)        │   │
│  │  └─ System: read-only                   │   │
│  └──────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

### 샌드박스 모드별 동작

**1. Suggest Mode (기본)**
- 모든 도구 사용 시 Electron UI에 ApprovalDialog 표시
- 사용자 명시적 승인 후 실행
- 파일 변경 diff 미리보기 제공

**2. Auto Edit Mode**
- 파일 패치 자동 적용
- 쉘 명령 실행 시에만 승인 UI 표시
- 백그라운드에서 변경사항 알림

**3. Full Auto Mode**
- 모든 작업 자동 실행
- 실시간 진행 상황 StatusBar에 표시
- 완료 후 요약 제공

### Electron 특화 보안

```typescript
// main/index.ts
const mainWindow = new BrowserWindow({
  webPreferences: {
    nodeIntegration: false,      // Node.js 비활성화
    contextIsolation: true,       // Context 격리
    sandbox: true,                // Chromium 샌드박스
    preload: path.join(__dirname, 'preload.js'),
  }
});

// CSP 헤더 설정
mainWindow.webContents.session.webRequest.onHeadersReceived((details, callback) => {
  callback({
    responseHeaders: {
      ...details.responseHeaders,
      'Content-Security-Policy': [
        "default-src 'self'; " +
        "script-src 'self'; " +
        "style-src 'self' 'unsafe-inline'; " +
        "img-src 'self' data: https:; " +
        "connect-src 'self' https://api.openai.com;"
      ]
    }
  });
});
```

---

## 디렉토리 구조

### 전체 프로젝트 구조

```
codex-ui/
├── codex-rs/                       # 기존 Rust 백엔드 (유지)
│   ├── core/
│   ├── app-server/
│   ├── app-server-protocol/
│   ├── electron-bridge/            # ✨ 새로 추가
│   └── [other crates]/
├── electron-ui/                    # ✨ 새로 추가 - Electron 프론트엔드
│   ├── main/
│   ├── preload/
│   ├── renderer/
│   └── package.json
├── codex-cli/                      # 기존 TypeScript CLI (유지)
├── sdk/                            # 기존 SDK (유지)
├── docs/
│   ├── electron-ui-architecture.md # 이 문서
│   └── electron-ui-requirements.md
└── package.json                    # Root workspace
```

---

## 기술 스택

### Frontend (Electron UI)

| 레이어 | 기술 | 목적 |
|--------|------|------|
| **Framework** | Electron 28+ | 크로스플랫폼 데스크톱 |
| **UI Library** | React 18+ | 컴포넌트 기반 UI |
| **Language** | TypeScript 5+ | 타입 안전성 |
| **Build Tool** | Vite 5+ | 빠른 개발/빌드 |
| **State Management** | Zustand | 경량 상태 관리 |
| **Code Editor** | Monaco Editor | VS Code 엔진 |
| **Markdown** | react-markdown | 마크다운 렌더링 |
| **Syntax Highlighting** | Prism.js | 코드 하이라이팅 |
| **Diff Viewer** | react-diff-viewer | 파일 변경 비교 |
| **Styling** | Tailwind CSS | 유틸리티 CSS |
| **Icons** | Lucide React | 아이콘 세트 |
| **Packaging** | electron-builder | 배포 패키징 |

### Backend (Rust - 재사용)

기존 스택 그대로 사용:
- **Rust 2024**
- **Tokio** - 비동기 런타임
- **Axum** (app-server에서 사용하지 않지만 향후 WebSocket 지원 시 고려)
- **Serde** - JSON 직렬화
- 모든 기존 샌드박싱 컴포넌트

---

## 데이터 흐름

### 1. 메시지 전송 플로우

```
User types message
        ↓
InputArea (React Component)
        ↓
useBackend.sendMessage()
        ↓
window.codexAPI.sendMessage() [Preload API]
        ↓
ipcRenderer.invoke('backend:send')
        ↓
[IPC Channel]
        ↓
Main Process: ipcMain.handle('backend:send')
        ↓
BackendManager.sendMessage()
        ↓
process.stdin.write(JSON-RPC)
        ↓
[stdio pipe]
        ↓
Rust app-server
        ↓
codex-core (Tool execution)
        ↓
Events streamed back ←─┐
        ↓               │
[stdio pipe]           │
        ↓               │
BackendManager         │
        ↓               │
ipcRenderer.send('backend:event') ─┘
        ↓
[IPC Channel]
        ↓
Renderer: codexAPI.onEvent()
        ↓
Event handler updates React state
        ↓
UI re-renders (MessageList, StatusBar, etc.)
```

### 2. 도구 승인 플로우

```
Backend requests approval
        ↓
Event: { type: 'approval_needed', tool: {...} }
        ↓
[Event flow same as above]
        ↓
ApprovalDialog.tsx opens
        ↓
User reviews tool parameters
        ↓
User clicks "Approve" or "Reject"
        ↓
useBackend.approveToolUse(approved)
        ↓
[Request flow same as message]
        ↓
Backend continues or cancels execution
```

### 3. 파일 브라우저 플로우

```
User clicks "Open Workspace"
        ↓
window.codexAPI.selectDirectory()
        ↓
Main Process: dialog.showOpenDialog()
        ↓
Returns selected path
        ↓
Renderer updates workspace path
        ↓
Calls window.codexAPI.sendMessage({ method: 'file/search' })
        ↓
Backend performs file search (codex-file-search)
        ↓
Results streamed back
        ↓
FileExplorer.tsx displays file tree
```

---

## 마이그레이션 전략

### Phase 1: 기반 구축 (4-6주)

**Week 1-2: 프로젝트 셋업**
- [ ] Electron 프로젝트 초기화
- [ ] Build 시스템 구축 (Vite + electron-builder)
- [ ] TypeScript 설정 및 타입 생성 파이프라인
- [ ] 기본 IPC 통신 구조

**Week 3-4: Backend 통합**
- [ ] BackendManager 구현
- [ ] JSON-RPC 클라이언트 구현
- [ ] 이벤트 스트리밍 핸들러
- [ ] 프로세스 생명주기 관리

**Week 5-6: 기본 UI**
- [ ] 채팅 인터페이스 (메시지 전송/수신)
- [ ] 기본 레이아웃 (사이드바, 메인 영역, 상태바)
- [ ] 대화 목록 표시

### Phase 2: 핵심 기능 (6-8주)

**Week 7-10: 도구 승인 UI**
- [ ] ApprovalDialog 컴포넌트
- [ ] Diff 뷰어 통합
- [ ] 코드 미리보기 (Monaco Editor)
- [ ] 승인 모드 전환 (Suggest/Auto/Full)

**Week 11-14: 고급 기능**
- [ ] 파일 브라우저/탐색기
- [ ] 설정 패널
- [ ] 멀티 대화 관리
- [ ] 키보드 단축키

### Phase 3: 개선 및 최적화 (4-6주)

**Week 15-17: UX 개선**
- [ ] 마크다운 렌더링 개선
- [ ] 구문 하이라이팅
- [ ] 이미지 뷰어 (멀티모달)
- [ ] 다크 모드 / 테마

**Week 18-20: 성능 & 안정성**
- [ ] 메모리 최적화
- [ ] 에러 핸들링 강화
- [ ] 로깅 시스템
- [ ] 자동 업데이트 (electron-updater)

### Phase 4: 배포 준비 (2-4주)

**Week 21-24:**
- [ ] 패키징 및 코드 서명
- [ ] 설치 프로그램 생성 (Windows: NSIS, macOS: DMG, Linux: AppImage)
- [ ] 자동 업데이트 서버 구축
- [ ] 문서 작성 및 릴리스 노트

---

## 확장성 고려사항

### 1. 멀티 윈도우 지원

향후 여러 대화를 별도 윈도우로 열 수 있도록:

```typescript
// main/window-manager.ts
class WindowManager {
  private windows: Map<string, BrowserWindow> = new Map();

  createConversationWindow(conversationId: string): BrowserWindow {
    const window = new BrowserWindow({...});
    this.windows.set(conversationId, window);
    return window;
  }
}
```

### 2. 플러그인 시스템

MCP 서버 UI 통합:

```typescript
interface Plugin {
  id: string;
  name: string;
  renderUI: (container: HTMLElement, context: PluginContext) => void;
}

class PluginManager {
  loadPlugin(plugin: Plugin): void {
    // MCP 서버가 제공하는 UI 컴포넌트 로드
  }
}
```

### 3. WebSocket 지원 (선택적)

고성능 스트리밍이 필요한 경우, stdio 대신 WebSocket 고려:

```rust
// app-server에 WebSocket 엔드포인트 추가
#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/ws", get(websocket_handler));

    axum::Server::bind(&"127.0.0.1:0".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
```

### 4. 원격 Backend 지원

향후 클라우드 Backend 연결 지원:

```typescript
class RemoteBackendManager {
  async connect(url: string, token: string): Promise<void> {
    // WebSocket 또는 HTTP/2 연결
  }
}
```

---

## 모니터링 및 디버깅

### 개발 모드

```typescript
// main/index.ts
if (process.env.NODE_ENV === 'development') {
  // React DevTools
  require('electron-devtools-installer').default(
    require('electron-devtools-installer').REACT_DEVELOPER_TOOLS
  );

  // Backend stdout/stderr를 파일로 저장
  backendManager.on('stdout', (data) => {
    fs.appendFileSync('/tmp/codex-backend.log', data);
  });

  // Hot reload
  mainWindow.webContents.openDevTools();
}
```

### 프로덕션 로깅

```typescript
import log from 'electron-log';

log.transports.file.level = 'info';
log.transports.file.resolvePath = () =>
  path.join(app.getPath('userData'), 'logs', 'main.log');

log.info('Application started');
```

### 크래시 리포팅

```typescript
import { crashReporter } from 'electron';

crashReporter.start({
  productName: 'Codex UI',
  companyName: 'OpenAI',
  submitURL: 'https://your-crash-server.com/report',
  uploadToServer: true,
});
```

---

## 성능 최적화

### 1. Lazy Loading

```typescript
// 대화 내용 가상화
import { FixedSizeList } from 'react-window';

function MessageList({ messages }: { messages: Message[] }) {
  return (
    <FixedSizeList
      height={600}
      itemCount={messages.length}
      itemSize={80}
    >
      {({ index, style }) => (
        <div style={style}>
          <MessageItem message={messages[index]} />
        </div>
      )}
    </FixedSizeList>
  );
}
```

### 2. 이벤트 버퍼링

```typescript
class EventBuffer {
  private buffer: Event[] = [];
  private flushInterval: NodeJS.Timeout;

  constructor(private onFlush: (events: Event[]) => void) {
    this.flushInterval = setInterval(() => this.flush(), 100);
  }

  add(event: Event): void {
    this.buffer.push(event);
    if (this.buffer.length >= 50) {
      this.flush();
    }
  }

  private flush(): void {
    if (this.buffer.length > 0) {
      this.onFlush(this.buffer);
      this.buffer = [];
    }
  }
}
```

### 3. 메모리 관리

```typescript
// 오래된 메시지 정리
function useMessagePruning(messages: Message[], maxMessages = 1000) {
  useEffect(() => {
    if (messages.length > maxMessages) {
      // 오래된 메시지 아카이브
      archiveOldMessages(messages.slice(0, messages.length - maxMessages));
    }
  }, [messages.length]);
}
```

---

## 테스트 전략

### Unit Tests

```typescript
// __tests__/backend-manager.test.ts
describe('BackendManager', () => {
  it('should start backend process', async () => {
    const manager = new BackendManager();
    await manager.start();
    expect(manager.isRunning()).toBe(true);
  });

  it('should send JSON-RPC messages', async () => {
    const manager = new BackendManager();
    const response = await manager.sendMessage({
      method: 'thread/list',
      params: {}
    });
    expect(response).toHaveProperty('result');
  });
});
```

### Integration Tests

```typescript
// __tests__/e2e/conversation.test.ts
describe('Conversation Flow', () => {
  it('should create new conversation and send message', async () => {
    const app = await launchApp();

    await app.click('[data-testid="new-conversation"]');
    await app.type('[data-testid="message-input"]', 'Hello Codex');
    await app.click('[data-testid="send-button"]');

    await app.waitFor('[data-testid="message-item"]');
    expect(await app.getText('[data-testid="message-item"]')).toContain('Hello Codex');
  });
});
```

### E2E Tests (Playwright)

```typescript
import { test, expect, _electron as electron } from '@playwright/test';

test('full conversation workflow', async () => {
  const app = await electron.launch({ args: ['.'] });
  const window = await app.firstWindow();

  // Test complete workflow
});
```

---

## 배포 및 릴리스

### Electron Builder 설정

```yaml
# electron-builder.yml
appId: com.openai.codex
productName: Codex
copyright: Copyright © 2025 OpenAI

directories:
  buildResources: build
  output: dist

files:
  - main/**/*
  - preload/**/*
  - renderer/dist/**/*
  - package.json

extraResources:
  - from: ../codex-rs/target/release/codex-app-server
    to: bin/codex-app-server
  - from: ../codex-rs/target/release/codex-linux-sandbox-exe
    to: bin/codex-linux-sandbox-exe

mac:
  category: public.app-category.developer-tools
  target:
    - dmg
    - zip
  icon: build/icon.icns
  hardenedRuntime: true
  gatekeeperAssess: false
  entitlements: build/entitlements.mac.plist

win:
  target:
    - nsis
    - portable
  icon: build/icon.ico

linux:
  target:
    - AppImage
    - deb
    - rpm
  category: Development
  icon: build/icon.png
```

### CI/CD (GitHub Actions)

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    strategy:
      matrix:
        os: [macos-latest, ubuntu-latest, windows-latest]

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v3

      # Build Rust backend
      - uses: actions-rs/toolchain@v1
      - run: cargo build --release --manifest-path codex-rs/Cargo.toml

      # Build Electron UI
      - uses: actions/setup-node@v3
      - run: cd electron-ui && npm ci
      - run: cd electron-ui && npm run build

      # Package
      - run: cd electron-ui && npm run package

      # Upload artifacts
      - uses: actions/upload-artifact@v3
        with:
          name: ${{ matrix.os }}
          path: electron-ui/dist/*
```

---

## 결론

이 아키텍처는 다음을 달성합니다:

✅ **보안 유지**: 플랫폼 네이티브 샌드박싱 완전히 보존
✅ **코드 재사용**: 기존 Rust 백엔드 100% 활용
✅ **명확한 분리**: 프로토콜 기반 프론트엔드/백엔드 경계
✅ **확장성**: 플러그인, 멀티 윈도우 등 향후 확장 가능
✅ **점진적 도입**: TUI와 Electron UI 공존 가능
✅ **크로스 플랫폼**: macOS, Windows, Linux 모두 지원

다음 단계: [요구사항 문서](./electron-ui-requirements.md) 참조
