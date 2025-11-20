# Day 4 TODO - 세션 관리 및 검색 (Electron)

> **목표**: electron-store를 활용한 완전한 세션 관리 시스템 및 검색 기능 구현

## 전체 개요

Day 4는 세션 관리의 핵심 기능을 구현합니다:
- Session CRUD 작업
- electron-store로 영속화
- Global shortcuts (Cmd/Ctrl+N, Cmd/Ctrl+F)
- Native menu 통합
- 전체 세션 검색
- 내보내기/가져오기 (JSON, Markdown, PDF)

**Electron 특화:**
- electron-store로 세션 자동 저장
- Native menu에 세션 메뉴 추가
- Global shortcuts 등록
- Native dialogs로 가져오기/내보내기
- Share menu (macOS)

---

## Commit 19: 세션 관리 구조

### 📋 작업 내용

1. **Session 타입 정의 (확장)**
2. **Session Store 구현**
3. **Session CRUD IPC Handlers**
4. **Session 자동 저장**

### 📁 파일 구조

```
src/renderer/store/
└── useSessionStore.ts    # Session store (확장)

src/main/handlers/
└── session.ts            # Session IPC handlers
```

### 1️⃣ Extended Session Types

**파일**: `src/renderer/types/session.ts`

```typescript
export interface SessionMetadata {
  model: string;
  totalTokens: number;
  totalCost: number;
  messageCount: number;
  createdAt: number;
  updatedAt: number;
  lastAccessedAt: number;
  tags?: string[];
  starred?: boolean;
}

export interface Session {
  id: string;
  title: string;
  messages: Message[];
  metadata: SessionMetadata;
}

export interface SessionGroup {
  id: string;
  name: string;
  sessionIds: string[];
  color?: string;
}
```

### 2️⃣ Session Store

**파일**: `src/renderer/store/useSessionStore.ts`

```typescript
import { create } from 'zustand';
import { devtools, persist } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import { nanoid } from 'nanoid';

interface SessionState {
  sessions: Map<string, Session>;
  groups: Map<string, SessionGroup>;
  currentSessionId: string | null;
  searchQuery: string;
  filterTags: string[];
  sortBy: 'recent' | 'title' | 'tokens';
}

interface SessionActions {
  // CRUD
  createSession: (title?: string, groupId?: string) => string;
  deleteSession: (id: string) => void;
  updateSession: (id: string, updates: Partial<Session>) => void;
  duplicateSession: (id: string) => string;

  // Navigation
  switchSession: (id: string) => void;
  getNextSession: () => string | null;
  getPrevSession: () => string | null;

  // Groups
  createGroup: (name: string) => string;
  addToGroup: (sessionId: string, groupId: string) => void;
  removeFromGroup: (sessionId: string, groupId: string) => void;

  // Search & Filter
  setSearchQuery: (query: string) => void;
  setFilterTags: (tags: string[]) => void;
  setSortBy: (sort: 'recent' | 'title' | 'tokens') => void;

  // Persistence
  saveToElectron: () => Promise<void>;
  loadFromElectron: () => Promise<void>;

  // Import/Export
  exportSession: (id: string, format: 'json' | 'md' | 'pdf') => Promise<void>;
  importSession: (data: any) => Promise<string>;
}

export const useSessionStore = create<SessionState & SessionActions>()(
  devtools(
    immer((set, get) => ({
      // State
      sessions: new Map(),
      groups: new Map(),
      currentSessionId: null,
      searchQuery: '',
      filterTags: [],
      sortBy: 'recent',

      // CRUD
      createSession: (title, groupId) => {
        const id = nanoid();
        const session: Session = {
          id,
          title: title || `New Session ${new Date().toLocaleString()}`,
          messages: [],
          metadata: {
            model: 'claude-3-5-sonnet-20241022',
            totalTokens: 0,
            totalCost: 0,
            messageCount: 0,
            createdAt: Date.now(),
            updatedAt: Date.now(),
            lastAccessedAt: Date.now(),
          },
        };

        set((state) => {
          state.sessions.set(id, session);
          state.currentSessionId = id;

          if (groupId && state.groups.has(groupId)) {
            state.groups.get(groupId)!.sessionIds.push(id);
          }
        });

        get().saveToElectron();
        return id;
      },

      deleteSession: (id) => {
        set((state) => {
          state.sessions.delete(id);

          // Remove from groups
          for (const group of state.groups.values()) {
            const index = group.sessionIds.indexOf(id);
            if (index !== -1) {
              group.sessionIds.splice(index, 1);
            }
          }

          if (state.currentSessionId === id) {
            const remaining = Array.from(state.sessions.keys());
            state.currentSessionId = remaining[0] || null;
          }
        });

        get().saveToElectron();
      },

      updateSession: (id, updates) => {
        set((state) => {
          const session = state.sessions.get(id);
          if (session) {
            Object.assign(session, updates);
            session.metadata.updatedAt = Date.now();
          }
        });

        get().saveToElectron();
      },

      duplicateSession: (id) => {
        const session = get().sessions.get(id);
        if (!session) return '';

        const newId = nanoid();
        const duplicate: Session = {
          ...session,
          id: newId,
          title: `${session.title} (Copy)`,
          metadata: {
            ...session.metadata,
            createdAt: Date.now(),
            updatedAt: Date.now(),
            lastAccessedAt: Date.now(),
          },
        };

        set((state) => {
          state.sessions.set(newId, duplicate);
        });

        get().saveToElectron();
        return newId;
      },

      // Navigation
      switchSession: (id) => {
        set((state) => {
          if (state.sessions.has(id)) {
            state.currentSessionId = id;
            const session = state.sessions.get(id);
            if (session) {
              session.metadata.lastAccessedAt = Date.now();
            }
          }
        });

        get().saveToElectron();
      },

      getNextSession: () => {
        const { sessions, currentSessionId } = get();
        const ids = Array.from(sessions.keys());
        const currentIndex = currentSessionId ? ids.indexOf(currentSessionId) : -1;
        return ids[currentIndex + 1] || null;
      },

      getPrevSession: () => {
        const { sessions, currentSessionId } = get();
        const ids = Array.from(sessions.keys());
        const currentIndex = currentSessionId ? ids.indexOf(currentSessionId) : -1;
        return ids[currentIndex - 1] || null;
      },

      // Groups
      createGroup: (name) => {
        const id = nanoid();
        set((state) => {
          state.groups.set(id, {
            id,
            name,
            sessionIds: [],
          });
        });

        get().saveToElectron();
        return id;
      },

      addToGroup: (sessionId, groupId) => {
        set((state) => {
          const group = state.groups.get(groupId);
          if (group && !group.sessionIds.includes(sessionId)) {
            group.sessionIds.push(sessionId);
          }
        });

        get().saveToElectron();
      },

      removeFromGroup: (sessionId, groupId) => {
        set((state) => {
          const group = state.groups.get(groupId);
          if (group) {
            const index = group.sessionIds.indexOf(sessionId);
            if (index !== -1) {
              group.sessionIds.splice(index, 1);
            }
          }
        });

        get().saveToElectron();
      },

      // Search & Filter
      setSearchQuery: (query) => {
        set((state) => {
          state.searchQuery = query;
        });
      },

      setFilterTags: (tags) => {
        set((state) => {
          state.filterTags = tags;
        });
      },

      setSortBy: (sort) => {
        set((state) => {
          state.sortBy = sort;
        });
      },

      // Persistence
      saveToElectron: async () => {
        if (!window.electronAPI) return;

        const { sessions, groups } = get();
        await window.electronAPI.setSetting('sessions', {
          sessions: Array.from(sessions.values()),
          groups: Array.from(groups.values()),
        });
      },

      loadFromElectron: async () => {
        if (!window.electronAPI) return;

        const data = await window.electronAPI.getSetting('sessions');
        if (data) {
          set((state) => {
            state.sessions = new Map(data.sessions.map((s: Session) => [s.id, s]));
            state.groups = new Map(data.groups?.map((g: SessionGroup) => [g.id, g]) || []);
          });
        }
      },

      // Export
      exportSession: async (id, format) => {
        if (!window.electronAPI) return;

        const session = get().sessions.get(id);
        if (!session) return;

        if (format === 'md') {
          await window.electronAPI.exportMarkdown(session);
        } else if (format === 'json') {
          const filePath = await window.electronAPI.saveDialog({
            defaultPath: `${session.title}.json`,
            filters: [{ name: 'JSON', extensions: ['json'] }],
          });

          if (filePath) {
            await window.electronAPI.writeFile(
              filePath,
              JSON.stringify(session, null, 2)
            );
          }
        }
      },

      importSession: async (data) => {
        const newId = nanoid();
        const session: Session = {
          ...data,
          id: newId,
          metadata: {
            ...data.metadata,
            createdAt: Date.now(),
            updatedAt: Date.now(),
            lastAccessedAt: Date.now(),
          },
        };

        set((state) => {
          state.sessions.set(newId, session);
        });

        get().saveToElectron();
        return newId;
      },
    }))
  )
);
```

### ✅ 완료 기준

- [ ] Session CRUD 작동
- [ ] electron-store 자동 저장
- [ ] Session groups 지원
- [ ] Import/Export 기능

### 📝 Commit Message

```
feat(session): implement comprehensive session management

- Add extended session metadata (tokens, cost, tags)
- Implement session CRUD operations
- Add session grouping functionality
- Support session duplication
- Add import/export (JSON, Markdown)
- Auto-save to electron-store

Electron-specific:
- Persist sessions via electron-store
- Native dialogs for export
```

---

## Commit 20: 세션 UI 및 Shortcuts

### 📋 작업 내용

1. **SessionList 사이드바**
2. **Global Shortcuts (Cmd/Ctrl+N)**
3. **Native Menu 통합**
4. **최근 세션 자동 복원**

### 1️⃣ SessionList Component

**파일**: `src/renderer/components/session/SessionList.tsx`

```typescript
import React from 'react';
import { Plus, Search, MoreVertical } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';
import { useSessionStore } from '@/store/useSessionStore';
import { SessionItem } from './SessionItem';

export function SessionList() {
  const {
    sessions,
    currentSessionId,
    searchQuery,
    createSession,
    setSearchQuery,
  } = useSessionStore();

  const sessionArray = Array.from(sessions.values())
    .filter((s) =>
      s.title.toLowerCase().includes(searchQuery.toLowerCase())
    )
    .sort((a, b) => b.metadata.lastAccessedAt - a.metadata.lastAccessedAt);

  const handleNewSession = () => {
    createSession();
  };

  return (
    <div className="flex flex-col h-full w-64 border-r bg-background">
      {/* Header */}
      <div className="p-3 border-b">
        <div className="flex items-center justify-between mb-2">
          <h2 className="font-semibold text-sm">Sessions</h2>
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6"
            onClick={handleNewSession}
            title="New Session (Cmd/Ctrl+N)"
          >
            <Plus className="h-4 w-4" />
          </Button>
        </div>

        <div className="relative">
          <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground" />
          <Input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search sessions..."
            className="pl-7 h-8 text-sm"
          />
        </div>
      </div>

      {/* Session List */}
      <ScrollArea className="flex-1">
        <div className="p-2 space-y-1">
          {sessionArray.map((session) => (
            <SessionItem
              key={session.id}
              session={session}
              isActive={session.id === currentSessionId}
            />
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}
```

### 2️⃣ Global Shortcuts Registration

**파일**: `src/main/shortcuts.ts`

```typescript
import { app, globalShortcut, BrowserWindow } from 'electron';

export function registerGlobalShortcuts(window: BrowserWindow) {
  // New session: Cmd/Ctrl+N
  globalShortcut.register('CommandOrControl+N', () => {
    window.webContents.send('shortcut:new-session');
  });

  // Search: Cmd/Ctrl+F
  globalShortcut.register('CommandOrControl+F', () => {
    window.webContents.send('shortcut:search');
  });

  // Next session: Cmd/Ctrl+]
  globalShortcut.register('CommandOrControl+]', () => {
    window.webContents.send('shortcut:next-session');
  });

  // Previous session: Cmd/Ctrl+[
  globalShortcut.register('CommandOrControl+[', () => {
    window.webContents.send('shortcut:prev-session');
  });
}

export function unregisterGlobalShortcuts() {
  globalShortcut.unregisterAll();
}

// Cleanup on app quit
app.on('will-quit', () => {
  unregisterGlobalShortcuts();
});
```

### 3️⃣ Native Menu Integration

**파일**: `src/main/menu.ts`

```typescript
import { Menu, MenuItem, app, BrowserWindow } from 'electron';

export function createApplicationMenu(window: BrowserWindow) {
  const template: MenuItem[] = [
    {
      label: 'File',
      submenu: [
        {
          label: 'New Session',
          accelerator: 'CommandOrControl+N',
          click: () => {
            window.webContents.send('menu:new-session');
          },
        },
        { type: 'separator' },
        {
          label: 'Export Session...',
          click: () => {
            window.webContents.send('menu:export-session');
          },
        },
        {
          label: 'Import Session...',
          click: () => {
            window.webContents.send('menu:import-session');
          },
        },
        { type: 'separator' },
        { role: 'quit' },
      ],
    },
    {
      label: 'Edit',
      submenu: [
        { role: 'undo' },
        { role: 'redo' },
        { type: 'separator' },
        { role: 'cut' },
        { role: 'copy' },
        { role: 'paste' },
      ],
    },
    {
      label: 'Session',
      submenu: [
        {
          label: 'Next Session',
          accelerator: 'CommandOrControl+]',
          click: () => {
            window.webContents.send('menu:next-session');
          },
        },
        {
          label: 'Previous Session',
          accelerator: 'CommandOrControl+[',
          click: () => {
            window.webContents.send('menu:prev-session');
          },
        },
        { type: 'separator' },
        {
          label: 'Delete Current Session',
          click: () => {
            window.webContents.send('menu:delete-session');
          },
        },
      ],
    },
  ];

  const menu = Menu.buildFromTemplate(template);
  Menu.setApplicationMenu(menu);
}
```

### ✅ 완료 기준

- [ ] SessionList 렌더링
- [ ] Cmd/Ctrl+N으로 새 세션
- [ ] Global shortcuts 작동
- [ ] Native menu 통합
- [ ] 최근 세션 자동 복원

### 📝 Commit Message

```
feat(session): add session UI with global shortcuts

- Implement SessionList sidebar
- Add session search functionality
- Register global shortcuts (Cmd+N, Cmd+F)
- Integrate native menu with session commands
- Auto-restore last session on app start

Electron-specific:
- Global shortcuts via globalShortcut API
- Native menu integration
- IPC for menu commands
```

---

## Commits 21-24: 히스토리, 검색, 내보내기, 통계

*Consolidated for brevity*

### 핵심 기능

**Commit 21: 히스토리 백업**
- electron-store 자동 백업
- Native dialogs로 가져오기/내보내기
- 앱 재시작 시 자동 복원

**Commit 22: 전체 검색**
- Fuzzy search (fuse.js)
- Cmd/Ctrl+F global shortcut
- 검색 결과 하이라이팅

**Commit 23: 내보내기**
- JSON, Markdown, HTML, PDF
- Native save dialog
- macOS Share menu
- 클립보드 복사

**Commit 24: 통계**
- Chart.js 통합
- 세션 분석 (토큰, 비용, 메시지 수)
- Native print dialog

### ✅ Day 4 완료 기준

- [ ] Session CRUD 완성
- [ ] electron-store 자동 저장
- [ ] Global shortcuts 작동
- [ ] Native menu 통합
- [ ] 전체 검색 기능
- [ ] 내보내기/가져오기
- [ ] 통계 대시보드

### 📦 Dependencies

```json
{
  "dependencies": {
    "fuse.js": "^7.0.0",
    "chart.js": "^4.4.1",
    "react-chartjs-2": "^5.2.0"
  }
}
```

---

**다음**: Day 5에서는 설정 관리, 인증, 테마, Native 통합을 구현합니다.
