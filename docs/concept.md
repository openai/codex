# Codex UI - Project Concept Document

## 프로젝트 비전

**Codex UI**는 OpenAI Codex CLI의 강력한 기능을 누구나 쉽게 사용할 수 있는 현대적인 웹 인터페이스로 제공하는 프로젝트입니다. 터미널 환경에 익숙하지 않은 사용자도 직관적인 GUI를 통해 AI 코딩 어시스턴트의 모든 기능을 활용할 수 있도록 합니다.

### 핵심 목표

1. **접근성**: 터미널 CLI의 진입 장벽을 제거하고 모든 개발자가 쉽게 사용
2. **생산성**: 시각적 인터페이스를 통한 빠른 작업 흐름
3. **플랫폼 독립성**: 웹과 데스크톱 앱으로 어디서나 사용 가능
4. **확장성**: 플러그인과 커스터마이징을 통한 개인화

---

## 왜 Codex UI가 필요한가?

### Codex CLI의 한계

현재 Codex CLI는 강력하지만 다음과 같은 한계가 있습니다:

1. **터미널 의존성**
   - 터미널에 익숙하지 않은 사용자에게 어려움
   - 복잡한 명령어 구조
   - 시각적 피드백 부족

2. **파일 관리의 어려움**
   - 파일 탐색이 불편함
   - 변경사항을 시각적으로 확인하기 어려움
   - Diff 비교가 직관적이지 않음

3. **세션 관리 부족**
   - 여러 대화를 관리하기 어려움
   - 대화 기록 검색이 불편함
   - 작업 컨텍스트 유지 어려움

4. **설정의 복잡성**
   - TOML 파일 직접 편집 필요
   - 설정 검증 어려움
   - 실시간 피드백 부족

### Codex UI가 제공하는 가치

✅ **직관적인 인터페이스**: 드래그 앤 드롭, 클릭, 시각적 피드백
✅ **파일 시스템 통합**: Monaco Editor, 파일 탐색기, Diff 뷰어
✅ **세션 관리**: 무제한 대화 저장, 검색, 내보내기
✅ **실시간 피드백**: 스트리밍 응답, 진행률 표시, 상태 업데이트
✅ **커스터마이징**: 테마, 단축키, 설정 UI
✅ **접근성**: 키보드 네비게이션, 스크린 리더 지원

---

## 아키텍처 개요

### 전체 시스템 구조

```
┌─────────────────────────────────────────────────────────┐
│                     사용자 레이어                        │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  ┌─────────────────┐         ┌──────────────────┐       │
│  │   Web Browser   │         │  Electron App    │       │
│  │  (Chrome, etc)  │         │   (Desktop)      │       │
│  └────────┬────────┘         └────────┬─────────┘       │
│           │                           │                  │
│           └───────────┬───────────────┘                  │
│                       │                                  │
│                       ▼                                  │
│         ┌─────────────────────────────┐                 │
│         │    Codex Web UI (React)     │                 │
│         │  - Chat Interface           │                 │
│         │  - File Explorer            │                 │
│         │  - Settings                 │                 │
│         │  - Session Management       │                 │
│         └──────────┬──────────────────┘                 │
│                    │                                     │
├────────────────────┼─────────────────────────────────────┤
│                    │   API Layer                         │
│                    │                                     │
│                    ▼                                     │
│         ┌─────────────────────────────┐                 │
│         │  Codex App Server (Rust)    │                 │
│         │  - WebSocket Handler        │                 │
│         │  - REST API                 │                 │
│         │  - File Operations          │                 │
│         │  - Tool Execution           │                 │
│         └──────────┬──────────────────┘                 │
│                    │                                     │
├────────────────────┼─────────────────────────────────────┤
│                    │   Backend Services                  │
│                    │                                     │
│                    ▼                                     │
│         ┌─────────────────────────────┐                 │
│         │     Codex Core (Rust)       │                 │
│         │  - Agent Execution          │                 │
│         │  - LLM Communication        │                 │
│         │  - Sandbox Management       │                 │
│         │  - MCP Integration          │                 │
│         └──────────┬──────────────────┘                 │
│                    │                                     │
│                    ▼                                     │
│         ┌─────────────────────────────┐                 │
│         │   OpenAI API / LLM Provider │                 │
│         └─────────────────────────────┘                 │
│                                                           │
└─────────────────────────────────────────────────────────┘
```

### 배포 아키텍처

#### 1. 웹 애플리케이션 (Web Deployment)

```
User Browser
     ↓
  Nginx (Static Files + Proxy)
     ↓
Codex App Server (localhost:8080)
     ↓
Codex Core → OpenAI API
```

**특징:**
- 브라우저에서 바로 접속
- 서버 설치 필요
- 팀 협업에 적합

#### 2. Electron 데스크톱 앱 (Standalone Desktop App)

```
┌────────────────────────────────────────┐
│         Electron Application           │
│                                         │
│  ┌──────────────────────────────────┐  │
│  │   Renderer Process (Web UI)      │  │
│  │   - React App                    │  │
│  │   - All UI Components            │  │
│  └────────────┬─────────────────────┘  │
│               │ IPC                     │
│  ┌────────────▼─────────────────────┐  │
│  │   Main Process                   │  │
│  │   - Window Management            │  │
│  │   - Native Menu                  │  │
│  │   - Auto Update                  │  │
│  │   - System Tray                  │  │
│  └────────────┬─────────────────────┘  │
│               │                         │
└───────────────┼─────────────────────────┘
                │
                ▼
     Bundled Codex App Server (Child Process)
                │
                ▼
          Codex Core → OpenAI API
```

**특징:**
- 완전히 독립적인 데스크톱 앱
- 설치만으로 즉시 사용 가능
- 네이티브 OS 통합 (메뉴, 단축키, 트레이)
- 자동 업데이트 지원
- 오프라인 기능 (로컬 모델 사용 시)

---

## Electron 통합 전략

### 핵심 개념: Standalone Desktop Application

**Codex UI는 단순한 웹 앱이 아니라, Electron을 통해 완전한 데스크톱 애플리케이션으로 패키징됩니다.**

### 아키텍처 구성

```typescript
// 프로젝트 구조
codex-ui/
├── codex-web-ui/          # React 웹 앱
│   ├── src/               # UI 소스 코드
│   └── dist/              # 빌드된 정적 파일
│
├── codex-electron/        # Electron 래퍼
│   ├── main/              # Main Process
│   │   ├── main.ts        # 앱 진입점
│   │   ├── window.ts      # 윈도우 관리
│   │   ├── server.ts      # 내장 서버 관리
│   │   ├── menu.ts        # 네이티브 메뉴
│   │   └── updater.ts     # 자동 업데이트
│   │
│   ├── preload/           # Preload Scripts
│   │   └── preload.ts     # IPC 브릿지
│   │
│   └── resources/         # 번들 리소스
│       └── codex-server/  # 번들된 Rust 바이너리
│
└── codex-rs/              # Rust 백엔드
    └── app-server/        # HTTP/WebSocket 서버
```

### Main Process 역할

```typescript
// codex-electron/main/main.ts
import { app, BrowserWindow } from 'electron';
import { startCodexServer } from './server';
import { createMenu } from './menu';
import { setupAutoUpdater } from './updater';

class CodexApp {
  private mainWindow: BrowserWindow | null = null;
  private serverProcess: ChildProcess | null = null;

  async init() {
    // 1. Bundled Codex Server 시작
    this.serverProcess = await startCodexServer();

    // 2. Main Window 생성
    this.mainWindow = new BrowserWindow({
      width: 1400,
      height: 900,
      webPreferences: {
        preload: path.join(__dirname, 'preload.js'),
        contextIsolation: true,
        nodeIntegration: false,
      },
    });

    // 3. React App 로드 (빌드된 정적 파일)
    await this.mainWindow.loadFile('dist/index.html');

    // 4. 네이티브 메뉴 설정
    createMenu(this.mainWindow);

    // 5. 자동 업데이트 설정
    setupAutoUpdater();
  }

  async cleanup() {
    // Codex Server 프로세스 종료
    if (this.serverProcess) {
      this.serverProcess.kill();
    }
  }
}
```

### 내장 서버 관리

```typescript
// codex-electron/main/server.ts
import { spawn } from 'child_process';
import path from 'path';

export async function startCodexServer(): Promise<ChildProcess> {
  // 번들된 Codex 서버 바이너리 경로
  const serverPath = path.join(
    process.resourcesPath,
    'codex-server',
    process.platform === 'win32' ? 'codex-server.exe' : 'codex-server'
  );

  // 서버 시작
  const serverProcess = spawn(serverPath, ['--port', '8080'], {
    stdio: 'pipe',
  });

  // 서버 준비 대기
  await waitForServerReady('http://localhost:8080/health');

  return serverProcess;
}

async function waitForServerReady(url: string): Promise<void> {
  // 서버가 준비될 때까지 대기
  for (let i = 0; i < 30; i++) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch (e) {
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }
  throw new Error('Server failed to start');
}
```

### IPC 통신

```typescript
// codex-electron/preload/preload.ts
import { contextBridge, ipcRenderer } from 'electron';

// React 앱에서 사용할 수 있는 안전한 API 노출
contextBridge.exposeInMainWorld('electronAPI', {
  // 윈도우 제어
  minimizeWindow: () => ipcRenderer.send('window-minimize'),
  maximizeWindow: () => ipcRenderer.send('window-maximize'),
  closeWindow: () => ipcRenderer.send('window-close'),

  // 파일 시스템
  selectDirectory: () => ipcRenderer.invoke('dialog-select-directory'),
  openExternal: (url: string) => ipcRenderer.invoke('shell-open-external', url),

  // 시스템 정보
  getPlatform: () => process.platform,
  getVersion: () => ipcRenderer.invoke('app-get-version'),

  // 설정
  getSetting: (key: string) => ipcRenderer.invoke('settings-get', key),
  setSetting: (key: string, value: any) => ipcRenderer.invoke('settings-set', key, value),

  // 업데이트
  onUpdateAvailable: (callback: () => void) => {
    ipcRenderer.on('update-available', callback);
  },
  checkForUpdates: () => ipcRenderer.send('check-for-updates'),
});
```

```typescript
// React 앱에서 사용
// src/lib/electron.ts
export const isElectron = () => {
  return typeof window !== 'undefined' && window.electronAPI !== undefined;
};

export const electron = window.electronAPI;

// 사용 예시
if (isElectron()) {
  const platform = electron.getPlatform();
  const version = await electron.getVersion();
}
```

### 패키징 및 배포

```json
// package.json
{
  "name": "codex-ui",
  "version": "1.0.0",
  "main": "dist-electron/main.js",
  "scripts": {
    "build:web": "cd codex-web-ui && pnpm build",
    "build:electron": "cd codex-electron && pnpm build",
    "build:server": "cd codex-rs && cargo build --release",
    "package:mac": "electron-builder --mac",
    "package:win": "electron-builder --win",
    "package:linux": "electron-builder --linux",
    "package:all": "electron-builder -mwl"
  },
  "build": {
    "appId": "com.openai.codex-ui",
    "productName": "Codex UI",
    "files": [
      "dist-electron/**/*",
      "dist/**/*",
      "resources/**/*"
    ],
    "extraResources": [
      {
        "from": "codex-rs/target/release/codex-server",
        "to": "codex-server/"
      }
    ],
    "mac": {
      "category": "public.app-category.developer-tools",
      "target": ["dmg", "zip"],
      "icon": "build/icon.icns"
    },
    "win": {
      "target": ["nsis", "portable"],
      "icon": "build/icon.ico"
    },
    "linux": {
      "target": ["AppImage", "deb", "rpm"],
      "category": "Development"
    }
  }
}
```

### 최종 배포물

#### macOS
```
Codex UI.app/
├── Contents/
│   ├── MacOS/
│   │   └── Codex UI           # Electron 실행 파일
│   ├── Resources/
│   │   ├── app.asar           # 패키징된 앱 (React + Electron)
│   │   └── codex-server/      # 번들된 Rust 서버
│   │       └── codex-server   # Rust 바이너리
│   └── Info.plist
```

#### Windows
```
Codex UI/
├── Codex UI.exe               # Electron 실행 파일
├── resources/
│   ├── app.asar              # 패키징된 앱
│   └── codex-server/
│       └── codex-server.exe  # Rust 바이너리
└── ...
```

#### Linux
```
codex-ui/
├── codex-ui                   # Electron 실행 파일
├── resources/
│   ├── app.asar
│   └── codex-server/
│       └── codex-server       # Rust 바이너리
└── ...
```

### 사용자 경험

1. **설치**
   - 사용자가 `.dmg`, `.exe`, `.AppImage` 다운로드
   - 한 번의 클릭으로 설치 완료

2. **실행**
   - 아이콘 더블클릭
   - Electron이 자동으로 내장 서버 시작
   - React UI 로드
   - 즉시 사용 가능

3. **업데이트**
   - 백그라운드에서 자동으로 업데이트 확인
   - 새 버전 다운로드
   - 재시작 시 업데이트 적용

---

## 기술 스택

### Frontend (React App)

```
Core:
├── React 18              # UI 프레임워크
├── TypeScript           # 타입 안전성
├── Vite                 # 빌드 도구
└── React Router         # 라우팅

Styling:
├── Tailwind CSS         # 유틸리티 CSS
├── shadcn/ui            # UI 컴포넌트
└── Radix UI             # Headless 컴포넌트

State Management:
├── Zustand              # 전역 상태
├── TanStack Query       # 서버 상태
└── IndexedDB            # 로컬 저장소

UI Components:
├── Monaco Editor        # 코드 에디터
├── react-markdown       # 마크다운 렌더링
├── react-syntax-highlighter # 코드 하이라이팅
├── react-window         # 가상 스크롤
└── react-diff-viewer    # Diff 뷰어

Communication:
├── Axios                # HTTP 클라이언트
├── WebSocket API        # 실시간 통신
└── idb                  # IndexedDB 래퍼
```

### Backend (Rust Server)

```
Core:
├── Tokio                # 비동기 런타임
├── Axum                 # HTTP 프레임워크
└── Tower                # 미들웨어

Communication:
├── WebSocket            # 실시간 통신
├── SSE                  # 서버 센트 이벤트
└── REST API             # HTTP 엔드포인트

Integrations:
├── MCP                  # Model Context Protocol
├── Sandbox              # 코드 실행 격리
└── File System          # 파일 작업
```

### Electron Desktop

```
Core:
├── Electron 28+         # 데스크톱 프레임워크
├── electron-builder     # 패키징 도구
└── electron-updater     # 자동 업데이트

IPC:
├── contextBridge        # 안전한 통신
└── ipcMain/ipcRenderer  # 프로세스 간 통신

Native:
├── Node.js APIs         # 파일 시스템 등
└── Child Process        # 서버 프로세스 관리
```

---

## 개발 철학

### 1. 사용자 중심 설계

- **간단함이 우선**: 복잡한 기능도 직관적인 UI로
- **피드백 제공**: 모든 액션에 즉각적인 시각적 피드백
- **에러 친화적**: 명확한 에러 메시지와 복구 옵션

### 2. 성능 최우선

- **빠른 초기 로딩**: 코드 스플리팅, 지연 로딩
- **부드러운 인터랙션**: 가상 스크롤, 메모이제이션
- **효율적인 데이터**: IndexedDB, 캐싱, 압축

### 3. 접근성

- **키보드 우선**: 모든 기능을 키보드로 접근
- **스크린 리더**: ARIA 레이블, 시맨틱 HTML
- **커스터마이징**: 테마, 폰트 크기, 레이아웃

### 4. 플랫폼 네이티브

- **OS 통합**: 시스템 메뉴, 단축키, 알림
- **성능**: 네이티브 바이너리 사용
- **오프라인**: 로컬 스토리지, 캐싱

---

## 배포 전략

### 3가지 배포 방식

#### 1. Self-Hosted Web App
```bash
# Docker Compose로 배포
docker-compose up -d

# 또는 수동 배포
cd codex-rs && cargo build --release
cd codex-web-ui && pnpm build
nginx -c nginx.conf
```

**사용 사례:**
- 팀 협업 환경
- 클라우드 서버
- 내부 개발 도구

#### 2. Electron Desktop App (권장)
```bash
# 빌드
pnpm build:all

# 패키징
pnpm package:mac   # macOS
pnpm package:win   # Windows
pnpm package:linux # Linux

# 배포
# - GitHub Releases
# - 직접 다운로드
# - 앱 스토어 (선택사항)
```

**사용 사례:**
- 개인 사용자
- 오프라인 환경
- 빠른 시작 필요

#### 3. Hybrid (Web + Desktop)
```bash
# 웹 버전으로 시작
# 필요시 데스크톱 앱으로 전환
```

**사용 사례:**
- 유연성 필요
- 평가 후 선택

---

## 로드맵

### Phase 1: MVP (Week 1-2)
- ✅ 기본 채팅 인터페이스
- ✅ 파일 탐색 및 뷰어
- ✅ 세션 관리
- ✅ 기본 설정

### Phase 2: Desktop App (Week 3)
- 🔄 Electron 통합
- 🔄 Native menu 구현
- 🔄 Auto-update 설정
- 🔄 Cross-platform 빌드

### Phase 3: Advanced Features (Week 4)
- 📋 플러그인 시스템
- 📋 테마 마켓플레이스
- 📋 협업 기능
- 📋 Git 통합

### Phase 4: Polish & Launch (Week 5-6)
- 📋 성능 최적화
- 📋 보안 강화
- 📋 문서화
- 📋 공개 출시

---

## 비교: Web vs Desktop

| 기능 | Web App | Electron Desktop |
|------|---------|------------------|
| 설치 | 서버 필요 | 클릭 한 번 |
| 접근성 | 브라우저 필요 | 독립 실행 |
| 업데이트 | 서버 업데이트 | 자동 업데이트 |
| 성능 | 네트워크 의존 | 로컬 실행 |
| 오프라인 | 제한적 | 완전 지원 |
| OS 통합 | 없음 | 네이티브 |
| 배포 | 복잡 | 간단 |
| 사용자 경험 | 브라우저 제약 | 네이티브 앱 |

**결론**: 대부분의 사용자에게 **Electron Desktop App**이 최적의 경험을 제공합니다.

---

## 차별화 요소

### vs Code Editor Extensions (Cursor, Copilot)
- ✅ **독립 실행**: 에디터에 종속되지 않음
- ✅ **전용 UI**: 채팅에 최적화된 인터페이스
- ✅ **세션 관리**: 무제한 대화 저장 및 검색

### vs Web-based AI Tools (ChatGPT, Claude)
- ✅ **로컬 실행**: 코드가 서버로 전송되지 않음
- ✅ **파일 시스템 통합**: 직접 파일 조작
- ✅ **도구 실행**: 실제 코드 실행 및 테스트

### vs Terminal CLI
- ✅ **시각적 인터페이스**: 직관적인 UI
- ✅ **멀티미디어**: 이미지, 차트, Diff 뷰
- ✅ **접근성**: 모든 수준의 사용자

---

## 성공 지표

### 기술적 목표
- ⚡ 초기 로딩 < 2초
- ⚡ 페이지 전환 < 100ms
- ⚡ 메모리 사용 < 300MB
- ⚡ 번들 크기 < 5MB (gzip)

### 사용자 목표
- 👥 월간 활성 사용자 10,000+
- ⭐ GitHub Stars 1,000+
- 📝 긍정적 리뷰 90%+
- 🔄 재방문율 70%+

### 비즈니스 목표
- 💼 오픈소스 커뮤니티 구축
- 🎯 엔터프라이즈 버전 론칭
- 🌍 다국어 지원 확대

---

## 라이선스 및 오픈소스

### 라이선스
- **Codex UI**: Apache-2.0 (OpenAI Codex CLI와 동일)
- **오픈소스**: GitHub에서 공개
- **기여 환영**: Community-driven development

### 기여 방법
1. Fork & Clone
2. Feature Branch 생성
3. Pull Request 제출
4. Code Review
5. Merge

---

## 결론

**Codex UI**는 단순한 웹 인터페이스를 넘어, **Electron을 통해 완전히 독립적인 데스크톱 애플리케이션**으로 제공됩니다.

사용자는 복잡한 설정 없이 **단 한 번의 클릭**으로 강력한 AI 코딩 어시스턴트를 사용할 수 있으며, 브라우저나 서버 없이도 **완전한 기능**을 경험할 수 있습니다.

이는 Codex CLI의 모든 기능을 유지하면서도, **누구나 쉽게 접근할 수 있는 현대적인 개발 도구**를 만드는 것이 우리의 비전입니다.

---

**문서 버전**: 1.0
**최종 업데이트**: 2025-11-20
**작성자**: Claude Code Assistant
**리뷰 필요**: Architecture Team
