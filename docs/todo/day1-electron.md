# Day 1 TODO - Electron + React 프로젝트 초기 설정

## 목표
Electron과 React를 처음부터 통합한 프로젝트를 설정하고, Rust 서버를 번들링하여 완전한 standalone 데스크톱 앱의 기반을 구축합니다.

---

## 1. Electron 프로젝트 초기화 (Commit 1)

### 요구사항
- electron-vite 보일러플레이트 설정
- TypeScript 구성
- 개발 환경 설정
- Hot reload 확인

### 작업 내용

#### 프로젝트 생성
- [ ] electron-vite로 프로젝트 생성
  ```bash
  cd /home/user/codex-ui
  pnpm create @quick-start/electron codex-desktop --template react-ts
  cd codex-desktop
  pnpm install
  ```

#### 프로젝트 구조 확인
- [ ] 생성된 구조 검토
  ```
  codex-desktop/
  ├── electron/
  │   ├── main/              # Main Process (Node.js)
  │   │   └── index.ts
  │   └── preload/           # Preload Script
  │       └── index.ts
  ├── src/                   # Renderer Process (React)
  │   ├── App.tsx
  │   ├── main.tsx
  │   └── index.html
  ├── resources/             # 앱 아이콘, 리소스
  ├── electron.vite.config.ts
  ├── package.json
  └── tsconfig.json
  ```

#### 개발 서버 실행 테스트
- [ ] 앱 실행 확인
  ```bash
  pnpm dev
  ```
  - Electron 창이 열리는지 확인
  - React 앱이 로드되는지 확인
  - Hot reload가 작동하는지 확인

#### package.json 스크립트 정리
- [ ] 필요한 스크립트 추가
  ```json
  {
    "scripts": {
      "dev": "electron-vite dev",
      "build": "electron-vite build",
      "preview": "electron-vite preview",
      "package:mac": "pnpm build && electron-builder --mac",
      "package:win": "pnpm build && electron-builder --win",
      "package:linux": "pnpm build && electron-builder --linux"
    }
  }
  ```

### 예상 결과물
- 실행 가능한 Electron + React 앱
- TypeScript 설정 완료
- 개발 환경 준비 완료

### Commit 메시지
```
feat: initialize Electron + React project with electron-vite

- Create project with electron-vite template
- Setup TypeScript configuration
- Verify hot reload functionality
- Configure build scripts
```

---

## 2. Rust 서버 번들링 구조 설정 (Commit 2)

### 요구사항
- Rust 서버 빌드 자동화
- 번들 리소스 경로 설정
- Main Process에서 서버 시작
- 개발/프로덕션 환경 분리

### 작업 내용

#### Rust 서버 빌드 스크립트
- [ ] `scripts/build-server.js` 생성
  ```javascript
  const { execSync } = require('child_process');
  const path = require('path');
  const fs = require('fs');

  const serverDir = path.join(__dirname, '..', '..', 'codex-rs', 'app-server');
  const targetDir = path.join(__dirname, '..', 'resources', 'server');

  // Rust 서버 빌드
  console.log('Building Rust server...');
  execSync('cargo build --release', {
    cwd: serverDir,
    stdio: 'inherit'
  });

  // 빌드된 바이너리 복사
  console.log('Copying server binary...');
  if (!fs.existsSync(targetDir)) {
    fs.mkdirSync(targetDir, { recursive: true });
  }

  const binaryName = process.platform === 'win32' ? 'codex-server.exe' : 'codex-server';
  const sourcePath = path.join(serverDir, 'target', 'release', binaryName);
  const destPath = path.join(targetDir, binaryName);

  fs.copyFileSync(sourcePath, destPath);

  // 실행 권한 부여 (Unix)
  if (process.platform !== 'win32') {
    fs.chmodSync(destPath, 0o755);
  }

  console.log('Server build complete!');
  ```

- [ ] package.json에 스크립트 추가
  ```json
  {
    "scripts": {
      "build:server": "node scripts/build-server.js",
      "prebuild": "pnpm build:server"
    }
  }
  ```

#### 서버 관리 모듈 생성
- [ ] `electron/main/server-manager.ts` 생성
  ```typescript
  import { ChildProcess, spawn } from 'child_process';
  import path from 'path';
  import { app } from 'electron';

  export class ServerManager {
    private serverProcess: ChildProcess | null = null;
    private serverPort = 8080;

    async start(): Promise<void> {
      const serverPath = this.getServerPath();

      console.log('Starting Codex server from:', serverPath);

      this.serverProcess = spawn(serverPath, ['--port', String(this.serverPort)], {
        stdio: 'pipe',
      });

      this.serverProcess.stdout?.on('data', (data) => {
        console.log(`[Server] ${data}`);
      });

      this.serverProcess.stderr?.on('data', (data) => {
        console.error(`[Server Error] ${data}`);
      });

      this.serverProcess.on('error', (error) => {
        console.error('Failed to start server:', error);
      });

      // 서버 준비 대기
      await this.waitForServerReady();
    }

    private getServerPath(): string {
      const isDev = !app.isPackaged;

      if (isDev) {
        // 개발 환경: resources/server에서 로드
        const binaryName = process.platform === 'win32' ? 'codex-server.exe' : 'codex-server';
        return path.join(__dirname, '..', '..', 'resources', 'server', binaryName);
      } else {
        // 프로덕션: 패키지된 리소스에서 로드
        const binaryName = process.platform === 'win32' ? 'codex-server.exe' : 'codex-server';
        return path.join(process.resourcesPath, 'server', binaryName);
      }
    }

    private async waitForServerReady(timeout = 30000): Promise<void> {
      const startTime = Date.now();

      while (Date.now() - startTime < timeout) {
        try {
          const response = await fetch(`http://localhost:${this.serverPort}/health`);
          if (response.ok) {
            console.log('Server is ready!');
            return;
          }
        } catch (e) {
          // 서버가 아직 준비되지 않음
        }
        await new Promise(resolve => setTimeout(resolve, 500));
      }

      throw new Error('Server failed to start within timeout');
    }

    async stop(): Promise<void> {
      if (this.serverProcess) {
        console.log('Stopping Codex server...');
        this.serverProcess.kill();
        this.serverProcess = null;
      }
    }

    getServerUrl(): string {
      return `http://localhost:${this.serverPort}`;
    }
  }
  ```

#### Main Process 수정
- [ ] `electron/main/index.ts` 업데이트
  ```typescript
  import { app, BrowserWindow } from 'electron';
  import path from 'path';
  import { ServerManager } from './server-manager';

  const serverManager = new ServerManager();
  let mainWindow: BrowserWindow | null = null;

  async function createWindow() {
    // 서버 시작
    await serverManager.start();

    mainWindow = new BrowserWindow({
      width: 1400,
      height: 900,
      webPreferences: {
        preload: path.join(__dirname, '../preload/index.js'),
        contextIsolation: true,
        nodeIntegration: false,
      },
    });

    // 개발 환경
    if (!app.isPackaged) {
      mainWindow.loadURL('http://localhost:5173');
      mainWindow.webContents.openDevTools();
    } else {
      // 프로덕션
      mainWindow.loadFile(path.join(__dirname, '../renderer/index.html'));
    }
  }

  app.whenReady().then(createWindow);

  app.on('window-all-closed', () => {
    serverManager.stop();
    if (process.platform !== 'darwin') {
      app.quit();
    }
  });

  app.on('before-quit', () => {
    serverManager.stop();
  });

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
  ```

#### electron-builder 설정
- [ ] `electron-builder.yml` 생성
  ```yaml
  appId: com.openai.codex-ui
  productName: Codex UI

  directories:
    output: dist-electron

  files:
    - out/**/*

  extraResources:
    - from: resources/server
      to: server

  mac:
    category: public.app-category.developer-tools
    target:
      - dmg
      - zip
    icon: resources/icon.icns

  win:
    target:
      - nsis
      - portable
    icon: resources/icon.ico

  linux:
    target:
      - AppImage
      - deb
    category: Development
    icon: resources/icon.png
  ```

### 예상 결과물
- Rust 서버 자동 빌드
- Main Process에서 서버 시작/종료
- 개발/프로덕션 환경 분리
- 번들링 준비 완료

### Commit 메시지
```
feat: setup Rust server bundling structure

- Add server build script
- Create ServerManager class
- Integrate server lifecycle with Electron
- Configure electron-builder for bundling
- Support dev/production environments
```

---

## 3. IPC 통신 구조 설정 (Commit 3)

### 요구사항
- Preload script에 안전한 API 노출
- Main-Renderer 간 양방향 통신
- TypeScript 타입 정의
- React에서 사용할 수 있는 훅

### 작업 내용

#### IPC 타입 정의
- [ ] `electron/shared/types.ts` 생성
  ```typescript
  export interface ElectronAPI {
    // System
    platform: NodeJS.Platform;

    // Window
    minimizeWindow: () => void;
    maximizeWindow: () => void;
    closeWindow: () => void;

    // File System
    selectDirectory: () => Promise<string | null>;
    selectFile: (filters?: FileFilter[]) => Promise<string | null>;

    // Server
    getServerUrl: () => Promise<string>;

    // Settings
    getSetting: (key: string) => Promise<any>;
    setSetting: (key: string, value: any) => Promise<void>;

    // Events
    onUpdateAvailable: (callback: () => void) => void;
    offUpdateAvailable: (callback: () => void) => void;
  }

  export interface FileFilter {
    name: string;
    extensions: string[];
  }

  declare global {
    interface Window {
      electronAPI: ElectronAPI;
    }
  }
  ```

#### Preload Script 구현
- [ ] `electron/preload/index.ts` 업데이트
  ```typescript
  import { contextBridge, ipcRenderer } from 'electron';
  import { ElectronAPI } from '../shared/types';

  const electronAPI: ElectronAPI = {
    platform: process.platform,

    // Window controls
    minimizeWindow: () => ipcRenderer.send('window-minimize'),
    maximizeWindow: () => ipcRenderer.send('window-maximize'),
    closeWindow: () => ipcRenderer.send('window-close'),

    // File system
    selectDirectory: () => ipcRenderer.invoke('dialog-select-directory'),
    selectFile: (filters) => ipcRenderer.invoke('dialog-select-file', filters),

    // Server
    getServerUrl: () => ipcRenderer.invoke('server-get-url'),

    // Settings
    getSetting: (key) => ipcRenderer.invoke('settings-get', key),
    setSetting: (key, value) => ipcRenderer.invoke('settings-set', key, value),

    // Events
    onUpdateAvailable: (callback) => {
      ipcRenderer.on('update-available', callback);
    },
    offUpdateAvailable: (callback) => {
      ipcRenderer.removeListener('update-available', callback);
    },
  };

  contextBridge.exposeInMainWorld('electronAPI', electronAPI);
  ```

#### Main Process IPC 핸들러
- [ ] `electron/main/ipc-handlers.ts` 생성
  ```typescript
  import { ipcMain, dialog, BrowserWindow } from 'electron';
  import Store from 'electron-store';
  import { ServerManager } from './server-manager';

  const store = new Store();

  export function setupIpcHandlers(
    serverManager: ServerManager,
    getMainWindow: () => BrowserWindow | null
  ) {
    // Window controls
    ipcMain.on('window-minimize', () => {
      getMainWindow()?.minimize();
    });

    ipcMain.on('window-maximize', () => {
      const win = getMainWindow();
      if (win?.isMaximized()) {
        win.unmaximize();
      } else {
        win?.maximize();
      }
    });

    ipcMain.on('window-close', () => {
      getMainWindow()?.close();
    });

    // File system
    ipcMain.handle('dialog-select-directory', async () => {
      const result = await dialog.showOpenDialog({
        properties: ['openDirectory'],
      });
      return result.canceled ? null : result.filePaths[0];
    });

    ipcMain.handle('dialog-select-file', async (_, filters) => {
      const result = await dialog.showOpenDialog({
        properties: ['openFile'],
        filters,
      });
      return result.canceled ? null : result.filePaths[0];
    });

    // Server
    ipcMain.handle('server-get-url', async () => {
      return serverManager.getServerUrl();
    });

    // Settings
    ipcMain.handle('settings-get', async (_, key) => {
      return store.get(key);
    });

    ipcMain.handle('settings-set', async (_, key, value) => {
      store.set(key, value);
    });
  }
  ```

- [ ] electron-store 설치
  ```bash
  pnpm add electron-store
  ```

- [ ] `electron/main/index.ts`에 핸들러 추가
  ```typescript
  import { setupIpcHandlers } from './ipc-handlers';

  function createWindow() {
    // ... existing code ...

    setupIpcHandlers(serverManager, () => mainWindow);
  }
  ```

#### React 훅 생성
- [ ] `src/hooks/useElectron.ts` 생성
  ```typescript
  export function useElectron() {
    const isElectron = typeof window !== 'undefined' && window.electronAPI !== undefined;

    return {
      isElectron,
      api: isElectron ? window.electronAPI : null,
    };
  }

  // 편의 훅들
  export function useServerUrl() {
    const { isElectron, api } = useElectron();
    const [serverUrl, setServerUrl] = useState<string>('http://localhost:8080');

    useEffect(() => {
      if (isElectron && api) {
        api.getServerUrl().then(setServerUrl);
      }
    }, [isElectron, api]);

    return serverUrl;
  }

  export function useSettings() {
    const { api } = useElectron();

    const getSetting = async (key: string) => {
      return api?.getSetting(key);
    };

    const setSetting = async (key: string, value: any) => {
      return api?.setSetting(key, value);
    };

    return { getSetting, setSetting };
  }
  ```

### 예상 결과물
- 타입 안전한 IPC 통신
- React에서 Electron API 사용 가능
- 설정 저장/로드 가능
- 파일 다이얼로그 사용 가능

### Commit 메시지
```
feat: setup IPC communication structure

- Define ElectronAPI types
- Implement secure preload script
- Add IPC handlers in main process
- Create React hooks for Electron APIs
- Install electron-store for settings
```

---

## 4. 기본 UI 구조 및 디자인 시스템 (Commit 4)

### 요구사항
- Tailwind CSS 설정
- shadcn/ui 초기화
- 기본 레이아웃 구성
- 커스텀 타이틀바 (프레임리스 윈도우)

### 작업 내용

#### Tailwind CSS 설정
- [ ] Tailwind 및 관련 패키지 설치
  ```bash
  pnpm add -D tailwindcss postcss autoprefixer
  pnpm add class-variance-authority clsx tailwind-merge
  npx tailwindcss init -p
  ```

- [ ] `tailwind.config.js` 설정
  ```javascript
  /** @type {import('tailwindcss').Config} */
  export default {
    darkMode: ['class'],
    content: [
      './src/**/*.{js,jsx,ts,tsx}',
    ],
    theme: {
      extend: {
        colors: {
          border: 'hsl(var(--border))',
          input: 'hsl(var(--input))',
          ring: 'hsl(var(--ring))',
          background: 'hsl(var(--background))',
          foreground: 'hsl(var(--foreground))',
          primary: {
            DEFAULT: 'hsl(var(--primary))',
            foreground: 'hsl(var(--primary-foreground))',
          },
          secondary: {
            DEFAULT: 'hsl(var(--secondary))',
            foreground: 'hsl(var(--secondary-foreground))',
          },
          muted: {
            DEFAULT: 'hsl(var(--muted))',
            foreground: 'hsl(var(--muted-foreground))',
          },
          accent: {
            DEFAULT: 'hsl(var(--accent))',
            foreground: 'hsl(var(--accent-foreground))',
          },
        },
      },
    },
    plugins: [],
  }
  ```

- [ ] `src/index.css` 업데이트
  ```css
  @tailwind base;
  @tailwind components;
  @tailwind utilities;

  @layer base {
    :root {
      --background: 0 0% 100%;
      --foreground: 222.2 84% 4.9%;
      --primary: 221.2 83.2% 53.3%;
      --primary-foreground: 210 40% 98%;
      --secondary: 210 40% 96.1%;
      --secondary-foreground: 222.2 47.4% 11.2%;
      --muted: 210 40% 96.1%;
      --muted-foreground: 215.4 16.3% 46.9%;
      --accent: 210 40% 96.1%;
      --accent-foreground: 222.2 47.4% 11.2%;
      --border: 214.3 31.8% 91.4%;
      --input: 214.3 31.8% 91.4%;
      --ring: 221.2 83.2% 53.3%;
    }

    .dark {
      --background: 222.2 84% 4.9%;
      --foreground: 210 40% 98%;
      --primary: 217.2 91.2% 59.8%;
      --primary-foreground: 222.2 47.4% 11.2%;
      --secondary: 217.2 32.6% 17.5%;
      --secondary-foreground: 210 40% 98%;
      --muted: 217.2 32.6% 17.5%;
      --muted-foreground: 215 20.2% 65.1%;
      --accent: 217.2 32.6% 17.5%;
      --accent-foreground: 210 40% 98%;
      --border: 217.2 32.6% 17.5%;
      --input: 217.2 32.6% 17.5%;
      --ring: 224.3 76.3% 48%;
    }
  }

  * {
    @apply border-border;
  }

  body {
    @apply bg-background text-foreground;
  }

  /* 커스텀 스크롤바 */
  ::-webkit-scrollbar {
    width: 10px;
  }

  ::-webkit-scrollbar-track {
    @apply bg-muted;
  }

  ::-webkit-scrollbar-thumb {
    @apply bg-muted-foreground/20 rounded;
  }

  ::-webkit-scrollbar-thumb:hover {
    @apply bg-muted-foreground/30;
  }
  ```

#### shadcn/ui 초기화
- [ ] shadcn/ui CLI 실행
  ```bash
  npx shadcn@latest init
  ```

  설정:
  - TypeScript: Yes
  - Style: Default
  - Base color: Slate
  - CSS variables: Yes

- [ ] 기본 컴포넌트 설치
  ```bash
  npx shadcn@latest add button
  npx shadcn@latest add input
  npx shadcn@latest add card
  ```

#### 커스텀 타이틀바
- [ ] `electron/main/index.ts`에서 프레임리스 윈도우 설정
  ```typescript
  mainWindow = new BrowserWindow({
    width: 1400,
    height: 900,
    frame: false,  // 타이틀바 제거
    titleBarStyle: 'hidden',
    webPreferences: {
      preload: path.join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  ```

- [ ] `src/components/TitleBar.tsx` 생성
  ```typescript
  import { useElectron } from '@/hooks/useElectron';
  import { Minus, Square, X } from 'lucide-react';
  import { Button } from './ui/button';

  export function TitleBar() {
    const { isElectron, api } = useElectron();

    if (!isElectron) return null;

    return (
      <div className="h-8 bg-background border-b flex items-center justify-between px-2 select-none drag">
        <div className="flex items-center gap-2">
          <img src="/icon.png" alt="Codex" className="w-4 h-4" />
          <span className="text-sm font-semibold">Codex UI</span>
        </div>

        <div className="flex items-center gap-1 no-drag">
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6 hover:bg-muted"
            onClick={() => api?.minimizeWindow()}
          >
            <Minus className="h-3 w-3" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6 hover:bg-muted"
            onClick={() => api?.maximizeWindow()}
          >
            <Square className="h-3 w-3" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6 hover:bg-destructive hover:text-destructive-foreground"
            onClick={() => api?.closeWindow()}
          >
            <X className="h-3 w-3" />
          </Button>
        </div>
      </div>
    );
  }
  ```

- [ ] CSS에 드래그 영역 추가 (`src/index.css`)
  ```css
  .drag {
    -webkit-app-region: drag;
  }

  .no-drag {
    -webkit-app-region: no-drag;
  }
  ```

#### 기본 레이아웃
- [ ] `src/components/AppLayout.tsx` 생성
  ```typescript
  import { TitleBar } from './TitleBar';

  export function AppLayout({ children }: { children: React.ReactNode }) {
    return (
      <div className="h-screen flex flex-col overflow-hidden">
        <TitleBar />
        <div className="flex-1 flex overflow-hidden">
          {children}
        </div>
      </div>
    );
  }
  ```

- [ ] `src/App.tsx` 업데이트
  ```typescript
  import { AppLayout } from './components/AppLayout';

  function App() {
    return (
      <AppLayout>
        <div className="flex-1 flex items-center justify-center">
          <h1 className="text-4xl font-bold">Codex UI</h1>
        </div>
      </AppLayout>
    );
  }

  export default App;
  ```

### 예상 결과물
- Tailwind CSS 설정 완료
- shadcn/ui 설치
- 커스텀 타이틀바
- 기본 레이아웃 구조

### Commit 메시지
```
feat: setup UI foundation with Tailwind and shadcn/ui

- Install and configure Tailwind CSS
- Initialize shadcn/ui
- Create custom titlebar for frameless window
- Build basic app layout structure
- Add window control buttons
```

---

## 5. React Router 및 기본 페이지 (Commit 5)

### 요구사항
- React Router 설정
- 네비게이션 구조
- 기본 페이지 생성
- 사이드바 네비게이션

### 작업 내용

#### React Router 설치 및 설정
- [ ] 패키지 설치
  ```bash
  pnpm add react-router-dom
  ```

- [ ] `src/main.tsx` 업데이트
  ```typescript
  import React from 'react';
  import ReactDOM from 'react-dom/client';
  import { BrowserRouter } from 'react-router-dom';
  import App from './App';
  import './index.css';

  ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </React.StrictMode>,
  );
  ```

#### 페이지 컴포넌트
- [ ] `src/pages/HomePage.tsx` 생성
  ```typescript
  export function HomePage() {
    return (
      <div className="flex flex-col items-center justify-center h-full p-8">
        <h1 className="text-4xl font-bold mb-4">Welcome to Codex UI</h1>
        <p className="text-muted-foreground text-center max-w-md">
          A modern desktop interface for OpenAI Codex CLI
        </p>
      </div>
    );
  }
  ```

- [ ] `src/pages/ChatPage.tsx` 생성
  ```typescript
  export function ChatPage() {
    return (
      <div className="flex flex-col h-full">
        <div className="border-b p-4">
          <h2 className="text-lg font-semibold">Chat</h2>
        </div>
        <div className="flex-1 p-4">
          <p className="text-muted-foreground">Chat interface will go here</p>
        </div>
      </div>
    );
  }
  ```

- [ ] `src/pages/SettingsPage.tsx` 생성
  ```typescript
  export function SettingsPage() {
    return (
      <div className="flex flex-col h-full">
        <div className="border-b p-4">
          <h2 className="text-lg font-semibold">Settings</h2>
        </div>
        <div className="flex-1 p-4">
          <p className="text-muted-foreground">Settings will go here</p>
        </div>
      </div>
    );
  }
  ```

#### 사이드바 네비게이션
- [ ] `src/components/Sidebar.tsx` 생성
  ```typescript
  import { NavLink } from 'react-router-dom';
  import { Home, MessageSquare, Settings } from 'lucide-react';
  import { cn } from '@/lib/utils';

  const navItems = [
    { to: '/', icon: Home, label: 'Home' },
    { to: '/chat', icon: MessageSquare, label: 'Chat' },
    { to: '/settings', icon: Settings, label: 'Settings' },
  ];

  export function Sidebar() {
    return (
      <div className="w-64 border-r bg-muted/20 flex flex-col">
        <div className="p-4 border-b">
          <h2 className="font-semibold">Navigation</h2>
        </div>
        <nav className="flex-1 p-2">
          {navItems.map((item) => {
            const Icon = item.icon;
            return (
              <NavLink
                key={item.to}
                to={item.to}
                className={({ isActive }) =>
                  cn(
                    'flex items-center gap-3 px-3 py-2 rounded-lg mb-1 transition-colors',
                    isActive
                      ? 'bg-primary text-primary-foreground'
                      : 'hover:bg-muted'
                  )
                }
              >
                <Icon className="w-4 h-4" />
                <span>{item.label}</span>
              </NavLink>
            );
          })}
        </nav>
      </div>
    );
  }
  ```

#### 라우팅 설정
- [ ] `src/App.tsx` 완성
  ```typescript
  import { Routes, Route } from 'react-router-dom';
  import { AppLayout } from './components/AppLayout';
  import { Sidebar } from './components/Sidebar';
  import { HomePage } from './pages/HomePage';
  import { ChatPage } from './pages/ChatPage';
  import { SettingsPage } from './pages/SettingsPage';

  function App() {
    return (
      <AppLayout>
        <Sidebar />
        <main className="flex-1 overflow-auto">
          <Routes>
            <Route path="/" element={<HomePage />} />
            <Route path="/chat" element={<ChatPage />} />
            <Route path="/settings" element={<SettingsPage />} />
          </Routes>
        </main>
      </AppLayout>
    );
  }

  export default App;
  ```

#### 유틸리티 함수
- [ ] `src/lib/utils.ts` 생성
  ```typescript
  import { type ClassValue, clsx } from 'clsx';
  import { twMerge } from 'tailwind-merge';

  export function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
  }
  ```

### 예상 결과물
- React Router 동작
- 페이지 간 네비게이션
- 사이드바 UI
- 기본 페이지 구조

### Commit 메시지
```
feat: implement routing and basic pages

- Setup React Router
- Create HomePage, ChatPage, SettingsPage
- Build Sidebar navigation component
- Implement route structure
- Add utility functions
```

---

## 6. 개발 환경 최적화 및 테스트 (Commit 6)

### 요구사항
- ESLint/Prettier 설정
- 환경 변수 관리
- 개발 도구 설정
- 빌드 테스트

### 작업 내용

#### ESLint 설정
- [ ] `.eslintrc.json` 생성
  ```json
  {
    "extends": [
      "eslint:recommended",
      "plugin:@typescript-eslint/recommended",
      "plugin:react-hooks/recommended"
    ],
    "parser": "@typescript-eslint/parser",
    "plugins": ["@typescript-eslint", "react-refresh"],
    "rules": {
      "react-refresh/only-export-components": "warn",
      "@typescript-eslint/no-explicit-any": "warn",
      "no-console": ["warn", { "allow": ["error", "warn"] }]
    }
  }
  ```

#### Prettier 설정
- [ ] `.prettierrc` 생성
  ```json
  {
    "semi": true,
    "trailingComma": "es5",
    "singleQuote": true,
    "printWidth": 100,
    "tabWidth": 2
  }
  ```

- [ ] `.prettierignore` 생성
  ```
  node_modules
  dist
  out
  dist-electron
  resources/server
  ```

#### 환경 변수
- [ ] `.env.development` 생성
  ```env
  VITE_SERVER_PORT=8080
  VITE_APP_NAME=Codex UI
  ```

- [ ] `.env.production` 생성
  ```env
  VITE_SERVER_PORT=8080
  VITE_APP_NAME=Codex UI
  ```

#### package.json 스크립트 정리
- [ ] 린트 및 포맷 스크립트 추가
  ```json
  {
    "scripts": {
      "dev": "electron-vite dev",
      "build": "electron-vite build",
      "build:server": "node scripts/build-server.js",
      "prebuild": "pnpm build:server",
      "preview": "electron-vite preview",
      "lint": "eslint src --ext ts,tsx",
      "lint:fix": "eslint src --ext ts,tsx --fix",
      "format": "prettier --write \"src/**/*.{ts,tsx}\"",
      "package:mac": "pnpm build && electron-builder --mac",
      "package:win": "pnpm build && electron-builder --win",
      "package:linux": "pnpm build && electron-builder --linux"
    }
  }
  ```

#### 빌드 테스트
- [ ] 개발 모드 테스트
  ```bash
  pnpm dev
  ```
  확인 사항:
  - Electron 창이 열리는지
  - React 앱이 로드되는지
  - Hot reload가 작동하는지
  - Rust 서버가 시작되는지

- [ ] 프로덕션 빌드 테스트
  ```bash
  pnpm build
  pnpm preview
  ```
  확인 사항:
  - 빌드가 성공하는지
  - 앱이 정상 실행되는지

#### .gitignore 업데이트
- [ ] `.gitignore` 확인 및 추가
  ```
  # Electron
  dist-electron
  out

  # Build
  dist

  # Server binary
  resources/server

  # Dependencies
  node_modules

  # Env
  .env.local

  # OS
  .DS_Store
  Thumbs.db
  ```

### 예상 결과물
- 코드 품질 도구 설정
- 환경 변수 관리
- 개발/프로덕션 빌드 검증
- 깔끔한 프로젝트 구조

### Commit 메시지
```
chore: optimize development environment

- Configure ESLint and Prettier
- Setup environment variables
- Add lint and format scripts
- Test development and production builds
- Update .gitignore
```

---

## Day 1 완료 체크리스트

- [ ] Electron + React 프로젝트 초기화 (electron-vite)
- [ ] Rust 서버 번들링 구조 설정
- [ ] IPC 통신 구조 (Preload, Types, Handlers)
- [ ] UI 기반 (Tailwind, shadcn/ui, 커스텀 타이틀바)
- [ ] 라우팅 및 기본 페이지
- [ ] 개발 환경 최적화 (ESLint, Prettier, 빌드 테스트)

---

## 다음 단계 (Day 2 예고)

1. 상태 관리 (Zustand)
2. 서버 통신 (API Client, WebSocket)
3. 채팅 인터페이스
4. 메시지 상태 관리
5. 실시간 스트리밍
6. 에러 처리

---

## 중요 포인트

### Electron First의 장점 확인
✅ 처음부터 Native API 활용
✅ IPC 통신 기반 구축
✅ 번들된 서버 관리
✅ 타이틀바 커스터마이징
✅ 설정 저장 (electron-store)

### 주의사항
⚠️ Preload script에서만 Node.js API 접근
⚠️ Renderer에서는 IPC를 통해서만 통신
⚠️ contextIsolation 반드시 true 유지
⚠️ 보안을 위해 nodeIntegration false 유지

---

**Last Updated**: 2025-11-20
**Version**: 2.0 (Electron First)
**Day**: 1 / 7
