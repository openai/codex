# Day 6 TODO - 성능 및 UX 개선 (Electron)

> **목표**: 키보드 단축키, 명령 팔레트, 성능 최적화, Native 통합 완성

## 전체 개요

Day 6는 사용자 경험과 성능을 최적화합니다:
- Global/Local shortcuts
- 명령 팔레트 (Cmd/Ctrl+K)
- React 성능 최적화
- System tray icon
- Badge count (Dock/Taskbar)
- Progress bar
- 접근성
- Window state 관리

**Electron 특화:**
- Global shortcuts registration
- System tray with context menu
- Dock/Taskbar badges
- Window state persistence
- Multi-window support
- Native progress indicators

---

## Commit 31: 키보드 단축키 시스템

### 📋 작업 내용

1. **Global shortcuts (앱 전역)**
2. **Local shortcuts (앱 내)**
3. **Shortcuts 도움말 (Cmd+/)**
4. **Menu accelerators**

### 📁 파일 구조

```
src/main/
└── shortcuts.ts          # Global shortcuts

src/renderer/hooks/
└── useKeyboard.ts        # Local shortcuts hook

src/renderer/components/
└── ShortcutsDialog.tsx   # 단축키 도움말
```

### 1️⃣ Global Shortcuts (확장)

**파일**: `src/main/shortcuts.ts`

```typescript
import { app, globalShortcut, BrowserWindow } from 'electron';

interface Shortcut {
  key: string;
  description: string;
  handler: (window: BrowserWindow) => void;
}

const shortcuts: Shortcut[] = [
  {
    key: 'CommandOrControl+N',
    description: 'New Session',
    handler: (window) => {
      window.webContents.send('shortcut:new-session');
    },
  },
  {
    key: 'CommandOrControl+K',
    description: 'Command Palette',
    handler: (window) => {
      window.webContents.send('shortcut:command-palette');
    },
  },
  {
    key: 'CommandOrControl+/',
    description: 'Show Shortcuts',
    handler: (window) => {
      window.webContents.send('shortcut:show-help');
    },
  },
  {
    key: 'CommandOrControl+,',
    description: 'Settings',
    handler: (window) => {
      window.webContents.send('shortcut:settings');
    },
  },
  {
    key: 'CommandOrControl+F',
    description: 'Search',
    handler: (window) => {
      window.webContents.send('shortcut:search');
    },
  },
  {
    key: 'CommandOrControl+]',
    description: 'Next Session',
    handler: (window) => {
      window.webContents.send('shortcut:next-session');
    },
  },
  {
    key: 'CommandOrControl+[',
    description: 'Previous Session',
    handler: (window) => {
      window.webContents.send('shortcut:prev-session');
    },
  },
  {
    key: 'CommandOrControl+W',
    description: 'Close Session',
    handler: (window) => {
      window.webContents.send('shortcut:close-session');
    },
  },
  {
    key: 'F12',
    description: 'Toggle DevTools',
    handler: (window) => {
      window.webContents.toggleDevTools();
    },
  },
];

export function registerGlobalShortcuts(window: BrowserWindow) {
  shortcuts.forEach(({ key, handler }) => {
    const success = globalShortcut.register(key, () => handler(window));
    if (!success) {
      console.error(`Failed to register shortcut: ${key}`);
    }
  });

  console.log(`Registered ${shortcuts.length} global shortcuts`);
}

export function unregisterGlobalShortcuts() {
  globalShortcut.unregisterAll();
}

export function getShortcuts() {
  return shortcuts.map(({ key, description }) => ({ key, description }));
}

app.on('will-quit', unregisterGlobalShortcuts);
```

### 2️⃣ Local Shortcuts Hook

**파일**: `src/renderer/hooks/useKeyboard.ts`

```typescript
import { useEffect } from 'react';

type KeyHandler = (event: KeyboardEvent) => void;

interface ShortcutConfig {
  key: string;
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  meta?: boolean;
  handler: () => void;
  description?: string;
}

export function useKeyboard(shortcuts: ShortcutConfig[]) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      for (const shortcut of shortcuts) {
        const { key, ctrl, shift, alt, meta, handler } = shortcut;

        const matchesKey = event.key.toLowerCase() === key.toLowerCase();
        const matchesCtrl = ctrl === undefined || event.ctrlKey === ctrl;
        const matchesShift = shift === undefined || event.shiftKey === shift;
        const matchesAlt = alt === undefined || event.altKey === alt;
        const matchesMeta = meta === undefined || event.metaKey === meta;

        if (matchesKey && matchesCtrl && matchesShift && matchesAlt && matchesMeta) {
          event.preventDefault();
          handler();
          break;
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [shortcuts]);
}

// Usage example
export function useCommonShortcuts() {
  const { createSession, deleteSession, nextSession, prevSession } =
    useSessionStore();
  const { setSearchQuery } = useChatStore();

  useKeyboard([
    {
      key: 'n',
      ctrl: true,
      handler: createSession,
      description: 'New Session',
    },
    {
      key: 'w',
      ctrl: true,
      handler: deleteSession,
      description: 'Close Session',
    },
    {
      key: ']',
      ctrl: true,
      handler: () => {
        const next = nextSession();
        if (next) switchSession(next);
      },
      description: 'Next Session',
    },
    {
      key: '[',
      ctrl: true,
      handler: () => {
        const prev = prevSession();
        if (prev) switchSession(prev);
      },
      description: 'Previous Session',
    },
    {
      key: 'f',
      ctrl: true,
      handler: () => {
        setSearchQuery('');
        // Focus search input
      },
      description: 'Search',
    },
  ]);
}
```

### 3️⃣ Shortcuts Dialog

**파일**: `src/renderer/components/ShortcutsDialog.tsx`

```typescript
import React from 'react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { ScrollArea } from '@/components/ui/scroll-area';

interface ShortcutsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function ShortcutsDialog({ open, onOpenChange }: ShortcutsDialogProps) {
  const isMac = navigator.platform.includes('Mac');

  const shortcutGroups = [
    {
      title: 'General',
      shortcuts: [
        { key: isMac ? '⌘N' : 'Ctrl+N', description: 'New Session' },
        { key: isMac ? '⌘K' : 'Ctrl+K', description: 'Command Palette' },
        { key: isMac ? '⌘,' : 'Ctrl+,', description: 'Settings' },
        { key: isMac ? '⌘/' : 'Ctrl+/', description: 'Show Shortcuts' },
      ],
    },
    {
      title: 'Navigation',
      shortcuts: [
        { key: isMac ? '⌘]' : 'Ctrl+]', description: 'Next Session' },
        { key: isMac ? '⌘[' : 'Ctrl+[', description: 'Previous Session' },
        { key: isMac ? '⌘W' : 'Ctrl+W', description: 'Close Session' },
      ],
    },
    {
      title: 'Editing',
      shortcuts: [
        { key: isMac ? '⌘S' : 'Ctrl+S', description: 'Save File' },
        { key: isMac ? '⌘F' : 'Ctrl+F', description: 'Search' },
        { key: isMac ? '⌘Z' : 'Ctrl+Z', description: 'Undo' },
        { key: isMac ? '⌘⇧Z' : 'Ctrl+Shift+Z', description: 'Redo' },
      ],
    },
    {
      title: 'Messages',
      shortcuts: [
        { key: 'Enter', description: 'Send Message' },
        { key: 'Shift+Enter', description: 'New Line' },
        { key: 'Esc', description: 'Stop Streaming' },
      ],
    },
  ];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Keyboard Shortcuts</DialogTitle>
        </DialogHeader>
        <ScrollArea className="h-[400px] pr-4">
          <div className="space-y-6">
            {shortcutGroups.map((group) => (
              <div key={group.title}>
                <h3 className="font-semibold mb-3 text-sm">{group.title}</h3>
                <div className="space-y-2">
                  {group.shortcuts.map((shortcut) => (
                    <div
                      key={shortcut.key}
                      className="flex items-center justify-between py-2"
                    >
                      <span className="text-sm text-muted-foreground">
                        {shortcut.description}
                      </span>
                      <kbd className="px-2 py-1 rounded bg-muted text-xs font-mono">
                        {shortcut.key}
                      </kbd>
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </ScrollArea>
      </DialogContent>
    </Dialog>
  );
}
```

### ✅ 완료 기준

- [ ] Global shortcuts 등록
- [ ] Local shortcuts hook
- [ ] 단축키 도움말 다이얼로그
- [ ] Menu accelerators 작동

### 📝 Commit Message

```
feat(shortcuts): implement comprehensive keyboard shortcuts

- Register global shortcuts (Cmd+N, Cmd+K, etc.)
- Add local shortcuts hook for in-app use
- Create shortcuts help dialog (Cmd+/)
- Support menu accelerators
- Handle platform-specific modifiers

Electron-specific:
- globalShortcut API
- Platform-specific key handling
```

---

## Commit 32: 명령 팔레트

### 📋 작업 내용

1. **Cmd/Ctrl+K 명령 팔레트**
2. **Fuzzy search (fuse.js)**
3. **최근 명령어 추적**
4. **IPC 액션 실행**

### 1️⃣ Command Palette

**파일**: `src/renderer/components/CommandPalette.tsx`

```typescript
import React, { useState, useEffect } from 'react';
import { Command } from 'cmdk';
import { Search, Settings, Plus, Folder, FileText } from 'lucide-react';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { useSessionStore } from '@/store/useSessionStore';
import { useFileStore } from '@/store/useFileStore';

interface CommandPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CommandPalette({ open, onOpenChange }: CommandPaletteProps) {
  const [search, setSearch] = useState('');
  const { createSession, sessions } = useSessionStore();
  const { setWorkspace } = useFileStore();

  const commands = [
    {
      id: 'new-session',
      title: 'New Session',
      icon: Plus,
      action: () => {
        createSession();
        onOpenChange(false);
      },
    },
    {
      id: 'open-folder',
      title: 'Open Folder',
      icon: Folder,
      action: async () => {
        if (window.electronAPI) {
          const path = await window.electronAPI.selectDirectory();
          if (path) {
            setWorkspace(path);
          }
        }
        onOpenChange(false);
      },
    },
    {
      id: 'settings',
      title: 'Open Settings',
      icon: Settings,
      action: () => {
        // Navigate to settings
        onOpenChange(false);
      },
    },
  ];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="p-0 max-w-2xl">
        <Command className="rounded-lg border shadow-md">
          <div className="flex items-center border-b px-3">
            <Search className="mr-2 h-4 w-4 shrink-0 opacity-50" />
            <Command.Input
              value={search}
              onValueChange={setSearch}
              placeholder="Type a command or search..."
              className="flex h-11 w-full rounded-md bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50"
            />
          </div>
          <Command.List className="max-h-[300px] overflow-y-auto p-2">
            <Command.Empty className="py-6 text-center text-sm">
              No results found.
            </Command.Empty>

            <Command.Group heading="Actions">
              {commands.map((command) => {
                const Icon = command.icon;
                return (
                  <Command.Item
                    key={command.id}
                    onSelect={command.action}
                    className="flex items-center gap-2 px-2 py-2 cursor-pointer rounded hover:bg-accent"
                  >
                    <Icon className="h-4 w-4" />
                    <span>{command.title}</span>
                  </Command.Item>
                );
              })}
            </Command.Group>

            <Command.Group heading="Recent Sessions">
              {Array.from(sessions.values())
                .slice(0, 5)
                .map((session) => (
                  <Command.Item
                    key={session.id}
                    onSelect={() => {
                      // Switch to session
                      onOpenChange(false);
                    }}
                    className="flex items-center gap-2 px-2 py-2 cursor-pointer rounded hover:bg-accent"
                  >
                    <FileText className="h-4 w-4" />
                    <span>{session.title}</span>
                  </Command.Item>
                ))}
            </Command.Group>
          </Command.List>
        </Command>
      </DialogContent>
    </Dialog>
  );
}
```

### ✅ 완료 기준

- [ ] Cmd/Ctrl+K 열기
- [ ] Fuzzy search 작동
- [ ] 명령어 실행
- [ ] 최근 세션 표시

### 📝 Commit Message

```
feat(commands): add command palette with fuzzy search

- Implement command palette (Cmd+K)
- Add fuzzy search for commands
- Show recent sessions
- Execute IPC actions
- Track command history

Uses cmdk library for command palette UI
```

---

## Commit 33: 성능 최적화

### 📋 작업 내용

1. **React.memo**
2. **가상 스크롤 (react-window)**
3. **Code splitting**
4. **Lazy loading**
5. **Preload optimization**

### 핵심 최적화

```typescript
// 1. React.memo for expensive components
export const MessageItem = React.memo(({ message }: MessageItemProps) => {
  // ...
}, (prev, next) => {
  return prev.message.id === next.message.id &&
         prev.message.status === next.message.status;
});

// 2. Virtual scrolling (already implemented in Day 2)
import { useVirtualizer } from '@tanstack/react-virtual';

// 3. Code splitting
const EditorPage = lazy(() => import('@/pages/Editor'));
const SettingsPage = lazy(() => import('@/pages/Settings'));

// 4. Lazy load Monaco Editor
const MonacoEditor = lazy(() => import('@monaco-editor/react'));

// 5. Optimize preload script
// Keep preload.ts minimal, only expose necessary APIs
```

### ✅ 완료 기준

- [ ] React.memo 적용
- [ ] 가상 스크롤 작동
- [ ] Code splitting 설정
- [ ] Lazy loading 구현

### 📝 Commit Message

```
perf: optimize React rendering and bundle size

- Add React.memo to expensive components
- Implement virtual scrolling for large lists
- Enable code splitting for routes
- Lazy load heavy components (Monaco Editor)
- Minimize preload script

Performance improvements:
- Reduced initial bundle size
- Faster rendering for large message lists
```

---

## Commit 34-36: Native 통합, 접근성, 창 관리

*Consolidated for brevity*

### 핵심 기능

**Commit 34: Native 통합**
- System tray icon with context menu
- Badge count on Dock/Taskbar
- Progress bar (macOS/Windows)
- Native notifications

**Commit 35: 접근성**
- Keyboard navigation
- Screen reader support (ARIA labels)
- High contrast mode
- Zoom support

**Commit 36: 창 관리**
- Window state 저장 (크기, 위치)
- Multi-window support
- Fullscreen mode
- Split view (side-by-side)

### 핵심 코드 - System Tray

**파일**: `src/main/tray.ts`

```typescript
import { Tray, Menu, nativeImage, app } from 'electron';
import path from 'path';

let tray: Tray | null = null;

export function createTray(window: BrowserWindow) {
  const iconPath = path.join(__dirname, '..', 'resources', 'tray-icon.png');
  const icon = nativeImage.createFromPath(iconPath);

  tray = new Tray(icon.resize({ width: 16, height: 16 }));

  const contextMenu = Menu.buildFromTemplate([
    {
      label: 'Show App',
      click: () => {
        window.show();
      },
    },
    {
      label: 'New Session',
      click: () => {
        window.webContents.send('tray:new-session');
      },
    },
    { type: 'separator' },
    {
      label: 'Quit',
      click: () => {
        app.quit();
      },
    },
  ]);

  tray.setContextMenu(contextMenu);
  tray.setToolTip('Codex UI');

  tray.on('click', () => {
    window.show();
  });
}

export function setBadgeCount(count: number) {
  if (process.platform === 'darwin') {
    app.dock.setBadge(count > 0 ? String(count) : '');
  } else if (process.platform === 'win32') {
    // Windows taskbar badge
    // Requires overlay icon
  }
}

export function setProgress(progress: number) {
  BrowserWindow.getAllWindows().forEach((window) => {
    window.setProgressBar(progress);
  });
}
```

### Window State Persistence

**파일**: `src/main/windowState.ts`

```typescript
import { BrowserWindow, screen } from 'electron';
import Store from 'electron-store';

const store = new Store();

export function saveWindowState(window: BrowserWindow) {
  const bounds = window.getBounds();
  const isMaximized = window.isMaximized();

  store.set('windowState', {
    x: bounds.x,
    y: bounds.y,
    width: bounds.width,
    height: bounds.height,
    isMaximized,
  });
}

export function restoreWindowState(window: BrowserWindow) {
  const state = store.get('windowState') as any;

  if (state) {
    const { width, height } = screen.getPrimaryDisplay().workAreaSize;

    // Validate bounds
    const x = Math.max(0, Math.min(state.x || 0, width - (state.width || 800)));
    const y = Math.max(0, Math.min(state.y || 0, height - (state.height || 600)));

    window.setBounds({
      x,
      y,
      width: state.width || 1200,
      height: state.height || 800,
    });

    if (state.isMaximized) {
      window.maximize();
    }
  }
}
```

### ✅ Day 6 완료 기준

- [ ] 키보드 단축키 시스템
- [ ] 명령 팔레트 (Cmd+K)
- [ ] 성능 최적화 적용
- [ ] System tray 작동
- [ ] Badge count 표시
- [ ] Window state 저장/복원
- [ ] 접근성 개선

### 📦 Dependencies

```json
{
  "dependencies": {
    "cmdk": "^0.2.0"
  }
}
```

---

**다음**: Day 7에서는 테스트, 문서화, 자동 업데이트, 배포를 구현합니다.
