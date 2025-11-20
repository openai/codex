# Day 4 TODO - 세션 및 히스토리 관리

## 목표
세션 관리 시스템을 구축하여 여러 대화를 관리하고, 히스토리를 저장/로드하며, 검색 및 통계 기능을 제공합니다.

---

## 1. 세션 관리 구조 (Commit 19)

### 요구사항
- 세션 데이터 구조 정의
- Zustand 세션 스토어 구현
- localStorage/IndexedDB 통합
- 세션 CRUD 작업

### 작업 내용

#### 세션 타입 정의
- [ ] `src/types/session.ts` 파일 생성
  ```typescript
  import { Message } from './message';

  export interface Session {
    id: string;
    name: string;
    createdAt: number;
    updatedAt: number;
    messages: Message[];
    metadata: {
      messageCount: number;
      toolCallCount: number;
      totalTokens?: number;
      model?: string;
    };
    tags?: string[];
    pinned?: boolean;
  }

  export interface SessionSummary {
    id: string;
    name: string;
    createdAt: number;
    updatedAt: number;
    messageCount: number;
    preview?: string; // 마지막 메시지 미리보기
    pinned?: boolean;
  }

  export interface SessionFilter {
    query?: string;
    tags?: string[];
    dateRange?: {
      start: number;
      end: number;
    };
    sortBy?: 'createdAt' | 'updatedAt' | 'name';
    sortOrder?: 'asc' | 'desc';
  }
  ```

#### IndexedDB 유틸리티
- [ ] `src/lib/indexeddb.ts` 생성
  ```typescript
  import { openDB, DBSchema, IDBPDatabase } from 'idb';
  import { Session } from '@/types/session';

  interface CodexDB extends DBSchema {
    sessions: {
      key: string;
      value: Session;
      indexes: {
        'by-updated': number;
        'by-created': number;
        'by-name': string;
      };
    };
  }

  const DB_NAME = 'codex-ui-db';
  const DB_VERSION = 1;

  let dbInstance: IDBPDatabase<CodexDB> | null = null;

  export async function getDB(): Promise<IDBPDatabase<CodexDB>> {
    if (dbInstance) {
      return dbInstance;
    }

    dbInstance = await openDB<CodexDB>(DB_NAME, DB_VERSION, {
      upgrade(db) {
        // Create sessions store
        const sessionStore = db.createObjectStore('sessions', {
          keyPath: 'id',
        });

        sessionStore.createIndex('by-updated', 'updatedAt');
        sessionStore.createIndex('by-created', 'createdAt');
        sessionStore.createIndex('by-name', 'name');
      },
    });

    return dbInstance;
  }

  export async function getAllSessions(): Promise<Session[]> {
    const db = await getDB();
    return db.getAll('sessions');
  }

  export async function getSession(id: string): Promise<Session | undefined> {
    const db = await getDB();
    return db.get('sessions', id);
  }

  export async function saveSession(session: Session): Promise<void> {
    const db = await getDB();
    await db.put('sessions', session);
  }

  export async function deleteSession(id: string): Promise<void> {
    const db = await getDB();
    await db.delete('sessions', id);
  }

  export async function clearAllSessions(): Promise<void> {
    const db = await getDB();
    await db.clear('sessions');
  }

  export async function getSessionsByDateRange(
    start: number,
    end: number
  ): Promise<Session[]> {
    const db = await getDB();
    const index = db.transaction('sessions').store.index('by-updated');
    return index.getAll(IDBKeyRange.bound(start, end));
  }
  ```

#### IDB 라이브러리 설치
- [ ] IndexedDB 래퍼 설치
  ```bash
  pnpm add idb
  ```

#### 세션 스토어 구현
- [ ] `src/store/session-store.ts` 생성
  ```typescript
  import { create } from 'zustand';
  import { Session, SessionSummary, SessionFilter } from '@/types/session';
  import {
    getAllSessions,
    getSession,
    saveSession,
    deleteSession,
    clearAllSessions,
  } from '@/lib/indexeddb';

  interface SessionState {
    // State
    sessions: SessionSummary[];
    currentSessionId: string | null;
    currentSession: Session | null;
    isLoading: boolean;

    // Actions
    loadSessions: () => Promise<void>;
    createSession: (name?: string) => Promise<Session>;
    loadSession: (id: string) => Promise<void>;
    updateSession: (id: string, updates: Partial<Session>) => Promise<void>;
    deleteSession: (id: string) => Promise<void>;
    setCurrentSession: (id: string) => Promise<void>;
    clearAllSessions: () => Promise<void>;

    // Utility
    getSummaries: () => SessionSummary[];
    filterSessions: (filter: SessionFilter) => SessionSummary[];
  }

  export const useSessionStore = create<SessionState>((set, get) => ({
    sessions: [],
    currentSessionId: null,
    currentSession: null,
    isLoading: false,

    loadSessions: async () => {
      set({ isLoading: true });
      try {
        const sessions = await getAllSessions();
        const summaries: SessionSummary[] = sessions.map((session) => ({
          id: session.id,
          name: session.name,
          createdAt: session.createdAt,
          updatedAt: session.updatedAt,
          messageCount: session.metadata.messageCount,
          preview: session.messages[session.messages.length - 1]?.content[0]?.content?.slice(0, 100),
          pinned: session.pinned,
        }));

        // Sort by updatedAt descending
        summaries.sort((a, b) => b.updatedAt - a.updatedAt);

        set({ sessions: summaries, isLoading: false });
      } catch (error) {
        console.error('Failed to load sessions', error);
        set({ isLoading: false });
      }
    },

    createSession: async (name) => {
      const newSession: Session = {
        id: crypto.randomUUID(),
        name: name || `Session ${new Date().toLocaleString()}`,
        createdAt: Date.now(),
        updatedAt: Date.now(),
        messages: [],
        metadata: {
          messageCount: 0,
          toolCallCount: 0,
        },
      };

      await saveSession(newSession);
      await get().loadSessions();
      set({ currentSessionId: newSession.id, currentSession: newSession });

      return newSession;
    },

    loadSession: async (id) => {
      set({ isLoading: true });
      try {
        const session = await getSession(id);
        if (session) {
          set({
            currentSessionId: id,
            currentSession: session,
            isLoading: false,
          });
        } else {
          console.error('Session not found', id);
          set({ isLoading: false });
        }
      } catch (error) {
        console.error('Failed to load session', error);
        set({ isLoading: false });
      }
    },

    updateSession: async (id, updates) => {
      const session = await getSession(id);
      if (!session) return;

      const updatedSession: Session = {
        ...session,
        ...updates,
        updatedAt: Date.now(),
      };

      await saveSession(updatedSession);
      await get().loadSessions();

      if (get().currentSessionId === id) {
        set({ currentSession: updatedSession });
      }
    },

    deleteSession: async (id) => {
      await deleteSession(id);
      await get().loadSessions();

      if (get().currentSessionId === id) {
        set({ currentSessionId: null, currentSession: null });
      }
    },

    setCurrentSession: async (id) => {
      await get().loadSession(id);
    },

    clearAllSessions: async () => {
      await clearAllSessions();
      set({
        sessions: [],
        currentSessionId: null,
        currentSession: null,
      });
    },

    getSummaries: () => {
      return get().sessions;
    },

    filterSessions: (filter) => {
      let filtered = [...get().sessions];

      // Query filter
      if (filter.query) {
        const query = filter.query.toLowerCase();
        filtered = filtered.filter(
          (s) =>
            s.name.toLowerCase().includes(query) ||
            s.preview?.toLowerCase().includes(query)
        );
      }

      // Date range filter
      if (filter.dateRange) {
        filtered = filtered.filter(
          (s) =>
            s.updatedAt >= filter.dateRange!.start &&
            s.updatedAt <= filter.dateRange!.end
        );
      }

      // Sort
      const sortBy = filter.sortBy || 'updatedAt';
      const sortOrder = filter.sortOrder || 'desc';

      filtered.sort((a, b) => {
        const aValue = a[sortBy];
        const bValue = b[sortBy];

        if (typeof aValue === 'string' && typeof bValue === 'string') {
          return sortOrder === 'asc'
            ? aValue.localeCompare(bValue)
            : bValue.localeCompare(aValue);
        }

        return sortOrder === 'asc'
          ? (aValue as number) - (bValue as number)
          : (bValue as number) - (aValue as number);
      });

      return filtered;
    },
  }));
  ```

#### 세션 동기화 훅
- [ ] `src/hooks/useSessionSync.ts` 생성
  ```typescript
  import { useEffect } from 'react';
  import { useSessionStore } from '@/store/session-store';
  import { useChatStore } from '@/store/chat-store';
  import { saveSession } from '@/lib/indexeddb';

  export function useSessionSync() {
    const { currentSession, updateSession } = useSessionStore();
    const messages = useChatStore((state) => state.messages);

    useEffect(() => {
      if (!currentSession) return;

      // 메시지가 변경되면 세션 업데이트
      const toolCallCount = messages.reduce(
        (count, msg) => count + (msg.toolCalls?.length || 0),
        0
      );

      updateSession(currentSession.id, {
        messages,
        metadata: {
          messageCount: messages.length,
          toolCallCount,
        },
      });
    }, [messages, currentSession?.id]);
  }
  ```

### 예상 결과물
- 세션 데이터 구조 정의
- IndexedDB 통합
- 세션 CRUD 작업
- 자동 세션 동기화

### Commit 메시지
```
feat(web-ui): implement session management

- Define session types and data structure
- Create IndexedDB utilities with idb library
- Build session store with Zustand
- Implement CRUD operations for sessions
- Add session filtering and sorting
- Create useSessionSync hook for auto-save
```

---

## 2. 세션 UI (Commit 20)

### 요구사항
- 세션 목록 사이드바
- 세션 생성/삭제/이름 변경
- 세션 전환
- 고정/즐겨찾기 기능

### 작업 내용

#### SessionList 컴포넌트
- [ ] `src/components/session/SessionList.tsx` 생성
  ```typescript
  import { useEffect, useState } from 'react';
  import { useSessionStore } from '@/store/session-store';
  import { SessionItem } from './SessionItem';
  import { Button } from '@/components/ui/button';
  import { Input } from '@/components/ui/input';
  import { ScrollArea } from '@/components/ui/scroll-area';
  import { Plus, Search } from 'lucide-react';

  export function SessionList() {
    const {
      sessions,
      currentSessionId,
      loadSessions,
      createSession,
      setCurrentSession,
      filterSessions,
    } = useSessionStore();

    const [searchQuery, setSearchQuery] = useState('');

    useEffect(() => {
      loadSessions();
    }, []);

    const filteredSessions = filterSessions({
      query: searchQuery,
      sortBy: 'updatedAt',
      sortOrder: 'desc',
    });

    const pinnedSessions = filteredSessions.filter((s) => s.pinned);
    const unpinnedSessions = filteredSessions.filter((s) => !s.pinned);

    const handleCreateSession = async () => {
      await createSession();
    };

    return (
      <div className="flex flex-col h-full border-r bg-muted/20">
        <div className="p-3 space-y-2 border-b">
          <Button
            onClick={handleCreateSession}
            className="w-full"
            size="sm"
          >
            <Plus className="w-4 h-4 mr-2" />
            New Session
          </Button>

          <div className="relative">
            <Search className="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              placeholder="Search sessions..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-8"
            />
          </div>
        </div>

        <ScrollArea className="flex-1">
          <div className="p-2 space-y-1">
            {pinnedSessions.length > 0 && (
              <div className="mb-4">
                <h3 className="px-2 py-1 text-xs font-semibold text-muted-foreground">
                  Pinned
                </h3>
                {pinnedSessions.map((session) => (
                  <SessionItem
                    key={session.id}
                    session={session}
                    isActive={session.id === currentSessionId}
                    onSelect={() => setCurrentSession(session.id)}
                  />
                ))}
              </div>
            )}

            {unpinnedSessions.length > 0 && (
              <div>
                <h3 className="px-2 py-1 text-xs font-semibold text-muted-foreground">
                  Recent
                </h3>
                {unpinnedSessions.map((session) => (
                  <SessionItem
                    key={session.id}
                    session={session}
                    isActive={session.id === currentSessionId}
                    onSelect={() => setCurrentSession(session.id)}
                  />
                ))}
              </div>
            )}

            {filteredSessions.length === 0 && (
              <div className="py-8 text-center text-sm text-muted-foreground">
                {searchQuery ? 'No sessions found' : 'No sessions yet'}
              </div>
            )}
          </div>
        </ScrollArea>
      </div>
    );
  }
  ```

#### SessionItem 컴포넌트
- [ ] `src/components/session/SessionItem.tsx` 생성
  ```typescript
  import { useState } from 'react';
  import { SessionSummary } from '@/types/session';
  import { useSessionStore } from '@/store/session-store';
  import { Button } from '@/components/ui/button';
  import { Input } from '@/components/ui/input';
  import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
  } from '@/components/ui/dropdown-menu';
  import {
    AlertDialog,
    AlertDialogAction,
    AlertDialogCancel,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle,
  } from '@/components/ui/alert-dialog';
  import {
    MessageSquare,
    MoreVertical,
    Pin,
    PinOff,
    Edit2,
    Trash2,
    Check,
    X,
  } from 'lucide-react';
  import { cn } from '@/lib/utils';
  import { formatTimestamp } from '@/lib/message-utils';

  interface SessionItemProps {
    session: SessionSummary;
    isActive: boolean;
    onSelect: () => void;
  }

  export function SessionItem({ session, isActive, onSelect }: SessionItemProps) {
    const { updateSession, deleteSession } = useSessionStore();
    const [isEditing, setIsEditing] = useState(false);
    const [editName, setEditName] = useState(session.name);
    const [showDeleteDialog, setShowDeleteDialog] = useState(false);

    const handleRename = async () => {
      if (editName.trim() && editName !== session.name) {
        await updateSession(session.id, { name: editName.trim() });
      }
      setIsEditing(false);
    };

    const handleTogglePin = async () => {
      await updateSession(session.id, { pinned: !session.pinned });
    };

    const handleDelete = async () => {
      await deleteSession(session.id);
      setShowDeleteDialog(false);
    };

    return (
      <>
        <div
          className={cn(
            'group flex items-center gap-2 px-2 py-2 rounded cursor-pointer hover:bg-accent transition-colors',
            isActive && 'bg-accent'
          )}
          onClick={!isEditing ? onSelect : undefined}
        >
          <MessageSquare className="w-4 h-4 flex-shrink-0 text-muted-foreground" />

          <div className="flex-1 min-w-0">
            {isEditing ? (
              <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
                <Input
                  value={editName}
                  onChange={(e) => setEditName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') handleRename();
                    if (e.key === 'Escape') {
                      setIsEditing(false);
                      setEditName(session.name);
                    }
                  }}
                  className="h-6 text-sm"
                  autoFocus
                />
                <Button size="icon" variant="ghost" className="h-6 w-6" onClick={handleRename}>
                  <Check className="h-3 w-3" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-6 w-6"
                  onClick={() => {
                    setIsEditing(false);
                    setEditName(session.name);
                  }}
                >
                  <X className="h-3 w-3" />
                </Button>
              </div>
            ) : (
              <>
                <div className="flex items-center gap-1">
                  <h4 className={cn('text-sm truncate', isActive && 'font-semibold')}>
                    {session.name}
                  </h4>
                  {session.pinned && <Pin className="w-3 h-3 text-primary flex-shrink-0" />}
                </div>
                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                  <span>{session.messageCount} messages</span>
                  <span>·</span>
                  <span>{formatTimestamp(session.updatedAt)}</span>
                </div>
                {session.preview && (
                  <p className="text-xs text-muted-foreground truncate mt-0.5">
                    {session.preview}
                  </p>
                )}
              </>
            )}
          </div>

          {!isEditing && (
            <DropdownMenu>
              <DropdownMenuTrigger asChild onClick={(e) => e.stopPropagation()}>
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-6 w-6 opacity-0 group-hover:opacity-100"
                >
                  <MoreVertical className="h-4 w-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem onClick={handleTogglePin}>
                  {session.pinned ? (
                    <>
                      <PinOff className="h-4 w-4 mr-2" />
                      Unpin
                    </>
                  ) : (
                    <>
                      <Pin className="h-4 w-4 mr-2" />
                      Pin
                    </>
                  )}
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => setIsEditing(true)}>
                  <Edit2 className="h-4 w-4 mr-2" />
                  Rename
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem
                  onClick={() => setShowDeleteDialog(true)}
                  className="text-destructive"
                >
                  <Trash2 className="h-4 w-4 mr-2" />
                  Delete
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          )}
        </div>

        <AlertDialog open={showDeleteDialog} onOpenChange={setShowDeleteDialog}>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Delete Session?</AlertDialogTitle>
              <AlertDialogDescription>
                This will permanently delete "{session.name}" and all its messages. This
                action cannot be undone.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Cancel</AlertDialogCancel>
              <AlertDialogAction onClick={handleDelete} className="bg-destructive">
                Delete
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </>
    );
  }
  ```

#### shadcn AlertDialog 설치
- [ ] AlertDialog 컴포넌트 설치
  ```bash
  npx shadcn@latest add alert-dialog
  ```

#### AppLayout에 통합
- [ ] `src/components/layout/AppLayout.tsx` 업데이트
  ```typescript
  import { SessionList } from '@/components/session/SessionList';

  export function AppLayout({ children }: { children: React.ReactNode }) {
    return (
      <div className="flex h-screen">
        <div className="w-64 flex-shrink-0">
          <SessionList />
        </div>
        <div className="flex-1">
          {children}
        </div>
      </div>
    );
  }
  ```

### 예상 결과물
- 세션 목록 사이드바
- 세션 생성/삭제
- 이름 변경 인라인 편집
- 고정 기능

### Commit 메시지
```
feat(web-ui): create session management UI

- Build SessionList sidebar component
- Implement SessionItem with actions
- Add session creation and deletion
- Support inline rename editing
- Implement pin/unpin functionality
- Add delete confirmation dialog
- Integrate with AppLayout
```

---

## 3. 히스토리 저장 및 로드 (Commit 21)

### 요구사항
- 자동 세션 저장
- 페이지 새로고침 시 복원
- 세션 내보내기/가져오기
- 세션 백업

### 작업 내용

#### 자동 저장 훅
- [ ] `src/hooks/useAutoSave.ts` 생성
  ```typescript
  import { useEffect, useRef } from 'react';
  import { useSessionStore } from '@/store/session-store';
  import { useChatStore } from '@/store/chat-store';
  import { saveSession } from '@/lib/indexeddb';
  import { debounce } from '@/lib/utils';

  export function useAutoSave() {
    const { currentSession } = useSessionStore();
    const messages = useChatStore((state) => state.messages);
    const saveTimeoutRef = useRef<NodeJS.Timeout>();

    useEffect(() => {
      if (!currentSession) return;

      // Debounce save to avoid too frequent writes
      const debouncedSave = debounce(async () => {
        const toolCallCount = messages.reduce(
          (count, msg) => count + (msg.toolCalls?.length || 0),
          0
        );

        await saveSession({
          ...currentSession,
          messages,
          updatedAt: Date.now(),
          metadata: {
            ...currentSession.metadata,
            messageCount: messages.length,
            toolCallCount,
          },
        });

        console.log('Session auto-saved');
      }, 1000);

      debouncedSave();

      return () => {
        if (saveTimeoutRef.current) {
          clearTimeout(saveTimeoutRef.current);
        }
      };
    }, [messages, currentSession]);
  }
  ```

#### 세션 복원
- [ ] `src/hooks/useSessionRestore.ts` 생성
  ```typescript
  import { useEffect } from 'react';
  import { useSessionStore } from '@/store/session-store';
  import { useChatStore } from '@/store/chat-store';

  export function useSessionRestore() {
    const { currentSession, loadSessions, createSession } = useSessionStore();
    const { messages, clearMessages } = useChatStore();

    useEffect(() => {
      const restoreSession = async () => {
        await loadSessions();

        // Try to restore last session from localStorage
        const lastSessionId = localStorage.getItem('lastSessionId');
        if (lastSessionId) {
          try {
            await useSessionStore.getState().setCurrentSession(lastSessionId);
          } catch (error) {
            console.error('Failed to restore session', error);
            // Create new session if restore fails
            await createSession('New Session');
          }
        } else {
          // Create first session
          await createSession('New Session');
        }
      };

      restoreSession();
    }, []);

    // Save current session ID to localStorage
    useEffect(() => {
      if (currentSession) {
        localStorage.setItem('lastSessionId', currentSession.id);
      }
    }, [currentSession?.id]);

    // Load messages from current session
    useEffect(() => {
      if (currentSession) {
        clearMessages();
        currentSession.messages.forEach((msg) => {
          useChatStore.getState().addMessage(msg);
        });
      }
    }, [currentSession?.id]);
  }
  ```

#### 세션 내보내기/가져오기
- [ ] `src/lib/session-export.ts` 생성
  ```typescript
  import { Session } from '@/types/session';
  import { saveSession } from './indexeddb';

  export function exportSessionToJSON(session: Session): string {
    return JSON.stringify(session, null, 2);
  }

  export function downloadSession(session: Session) {
    const json = exportSessionToJSON(session);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${session.name}-${session.id}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }

  export async function importSessionFromJSON(json: string): Promise<Session> {
    const session = JSON.parse(json) as Session;

    // Validate session structure
    if (!session.id || !session.name || !Array.isArray(session.messages)) {
      throw new Error('Invalid session format');
    }

    // Generate new ID to avoid conflicts
    const newSession: Session = {
      ...session,
      id: crypto.randomUUID(),
      createdAt: Date.now(),
      updatedAt: Date.now(),
    };

    await saveSession(newSession);
    return newSession;
  }

  export async function importSessionFromFile(file: File): Promise<Session> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();

      reader.onload = async (e) => {
        try {
          const json = e.target?.result as string;
          const session = await importSessionFromJSON(json);
          resolve(session);
        } catch (error) {
          reject(error);
        }
      };

      reader.onerror = () => reject(new Error('Failed to read file'));
      reader.readAsText(file);
    });
  }

  export async function exportAllSessions(sessions: Session[]): Promise<void> {
    const data = {
      version: 1,
      exportedAt: Date.now(),
      sessions,
    };

    const json = JSON.stringify(data, null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `codex-sessions-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }
  ```

#### SessionExportDialog 컴포넌트
- [ ] `src/components/session/SessionExportDialog.tsx` 생성
  ```typescript
  import { useState } from 'react';
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
  } from '@/components/ui/dialog';
  import { Button } from '@/components/ui/button';
  import { Input } from '@/components/ui/input';
  import { Label } from '@/components/ui/label';
  import { useSessionStore } from '@/store/session-store';
  import {
    downloadSession,
    importSessionFromFile,
    exportAllSessions,
  } from '@/lib/session-export';
  import { getAllSessions } from '@/lib/indexeddb';
  import { Download, Upload } from 'lucide-react';
  import { toast } from '@/lib/toast';

  interface SessionExportDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
  }

  export function SessionExportDialog({
    open,
    onOpenChange,
  }: SessionExportDialogProps) {
    const { currentSession, loadSessions } = useSessionStore();
    const [isImporting, setIsImporting] = useState(false);

    const handleExportCurrent = () => {
      if (currentSession) {
        downloadSession(currentSession);
        toast.success('Session exported');
      }
    };

    const handleExportAll = async () => {
      const sessions = await getAllSessions();
      await exportAllSessions(sessions);
      toast.success(`Exported ${sessions.length} sessions`);
    };

    const handleImport = async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;

      setIsImporting(true);
      try {
        const session = await importSessionFromFile(file);
        await loadSessions();
        toast.success(`Imported session: ${session.name}`);
        onOpenChange(false);
      } catch (error) {
        console.error('Import failed', error);
        toast.error('Failed to import session');
      } finally {
        setIsImporting(false);
      }
    };

    return (
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Export & Import</DialogTitle>
            <DialogDescription>
              Backup your sessions or import from a file
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <div className="space-y-2">
              <Label>Export</Label>
              <div className="flex gap-2">
                <Button
                  onClick={handleExportCurrent}
                  disabled={!currentSession}
                  className="flex-1"
                >
                  <Download className="w-4 h-4 mr-2" />
                  Current Session
                </Button>
                <Button onClick={handleExportAll} className="flex-1">
                  <Download className="w-4 h-4 mr-2" />
                  All Sessions
                </Button>
              </div>
            </div>

            <div className="space-y-2">
              <Label htmlFor="import">Import</Label>
              <div className="flex items-center gap-2">
                <Input
                  id="import"
                  type="file"
                  accept=".json"
                  onChange={handleImport}
                  disabled={isImporting}
                />
                <Upload className="w-5 h-5 text-muted-foreground" />
              </div>
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => onOpenChange(false)}>
              Close
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
  ```

#### shadcn Label 설치
- [ ] Label 컴포넌트 설치
  ```bash
  npx shadcn@latest add label
  ```

### 예상 결과물
- 자동 세션 저장
- 페이지 복원
- JSON 내보내기/가져오기
- 전체 세션 백업

### Commit 메시지
```
feat(web-ui): persist and restore chat history

- Implement auto-save with debouncing
- Create session restore on page load
- Add session export/import functionality
- Support JSON file import/export
- Build SessionExportDialog
- Save last session ID to localStorage
```

---

## 4. 검색 기능 (Commit 22)

### 요구사항
- 전체 세션 검색
- 메시지 내용 검색
- 검색 결과 하이라이팅
- 필터 및 정렬

### 작업 내용

#### 검색 유틸리티
- [ ] `src/lib/search-utils.ts` 생성
  ```typescript
  import { Session } from '@/types/session';
  import { Message } from '@/types/message';

  export interface SearchResult {
    sessionId: string;
    sessionName: string;
    messageId: string;
    message: Message;
    matchedContent: string;
    highlightedContent: string;
  }

  export function searchSessions(
    sessions: Session[],
    query: string
  ): SearchResult[] {
    if (!query.trim()) return [];

    const results: SearchResult[] = [];
    const lowerQuery = query.toLowerCase();

    sessions.forEach((session) => {
      session.messages.forEach((message) => {
        message.content.forEach((content) => {
          if (content.type === 'text') {
            const text = content.content;
            const lowerText = text.toLowerCase();

            if (lowerText.includes(lowerQuery)) {
              // Find the position of the match
              const index = lowerText.indexOf(lowerQuery);
              const start = Math.max(0, index - 50);
              const end = Math.min(text.length, index + query.length + 50);

              const matchedContent = text.slice(start, end);
              const highlightedContent = highlightText(
                matchedContent,
                query
              );

              results.push({
                sessionId: session.id,
                sessionName: session.name,
                messageId: message.id,
                message,
                matchedContent,
                highlightedContent,
              });
            }
          }
        });
      });
    });

    return results;
  }

  export function highlightText(text: string, query: string): string {
    if (!query) return text;

    const regex = new RegExp(`(${escapeRegExp(query)})`, 'gi');
    return text.replace(regex, '<mark>$1</mark>');
  }

  function escapeRegExp(string: string): string {
    return string.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }

  export function groupResultsBySession(
    results: SearchResult[]
  ): Map<string, SearchResult[]> {
    const grouped = new Map<string, SearchResult[]>();

    results.forEach((result) => {
      const existing = grouped.get(result.sessionId) || [];
      grouped.set(result.sessionId, [...existing, result]);
    });

    return grouped;
  }
  ```

#### GlobalSearch 컴포넌트
- [ ] `src/components/search/GlobalSearch.tsx` 생성
  ```typescript
  import { useState, useEffect } from 'react';
  import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
  } from '@/components/ui/dialog';
  import { Input } from '@/components/ui/input';
  import { ScrollArea } from '@/components/ui/scroll-area';
  import { Search, Loader2 } from 'lucide-react';
  import { useSessionStore } from '@/store/session-store';
  import { getAllSessions } from '@/lib/indexeddb';
  import { searchSessions, SearchResult, groupResultsBySession } from '@/lib/search-utils';
  import { SearchResultItem } from './SearchResultItem';
  import { useDebounce } from '@/hooks/useDebounce';

  interface GlobalSearchProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
  }

  export function GlobalSearch({ open, onOpenChange }: GlobalSearchProps) {
    const [query, setQuery] = useState('');
    const [results, setResults] = useState<SearchResult[]>([]);
    const [isSearching, setIsSearching] = useState(false);
    const debouncedQuery = useDebounce(query, 300);

    useEffect(() => {
      if (!debouncedQuery.trim()) {
        setResults([]);
        return;
      }

      performSearch(debouncedQuery);
    }, [debouncedQuery]);

    const performSearch = async (searchQuery: string) => {
      setIsSearching(true);
      try {
        const sessions = await getAllSessions();
        const searchResults = searchSessions(sessions, searchQuery);
        setResults(searchResults);
      } catch (error) {
        console.error('Search failed', error);
      } finally {
        setIsSearching(false);
      }
    };

    const groupedResults = groupResultsBySession(results);

    return (
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="max-w-3xl max-h-[80vh]">
          <DialogHeader>
            <DialogTitle>Search All Sessions</DialogTitle>
          </DialogHeader>

          <div className="relative">
            <Search className="absolute left-3 top-3 h-4 w-4 text-muted-foreground" />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search messages..."
              className="pl-9"
              autoFocus
            />
            {isSearching && (
              <Loader2 className="absolute right-3 top-3 h-4 w-4 animate-spin text-muted-foreground" />
            )}
          </div>

          <ScrollArea className="h-[500px]">
            {results.length === 0 ? (
              <div className="py-12 text-center text-sm text-muted-foreground">
                {query ? 'No results found' : 'Start typing to search'}
              </div>
            ) : (
              <div className="space-y-4">
                {Array.from(groupedResults.entries()).map(([sessionId, sessionResults]) => (
                  <div key={sessionId} className="space-y-2">
                    <h3 className="font-semibold text-sm sticky top-0 bg-background py-2">
                      {sessionResults[0].sessionName}
                      <span className="ml-2 text-muted-foreground font-normal">
                        ({sessionResults.length} {sessionResults.length === 1 ? 'match' : 'matches'})
                      </span>
                    </h3>
                    {sessionResults.map((result) => (
                      <SearchResultItem
                        key={result.messageId}
                        result={result}
                        onSelect={() => {
                          useSessionStore.getState().setCurrentSession(sessionId);
                          onOpenChange(false);
                        }}
                      />
                    ))}
                  </div>
                ))}
              </div>
            )}
          </ScrollArea>

          <div className="text-xs text-muted-foreground">
            {results.length > 0 && `Found ${results.length} results`}
          </div>
        </DialogContent>
      </Dialog>
    );
  }
  ```

#### SearchResultItem 컴포넌트
- [ ] `src/components/search/SearchResultItem.tsx` 생성
  ```typescript
  import { SearchResult } from '@/lib/search-utils';
  import { Card } from '@/components/ui/card';
  import { MessageRole } from '@/types/message';
  import { User, Bot } from 'lucide-react';
  import { formatTimestamp } from '@/lib/message-utils';

  interface SearchResultItemProps {
    result: SearchResult;
    onSelect: () => void;
  }

  export function SearchResultItem({ result, onSelect }: SearchResultItemProps) {
    const isUser = result.message.role === MessageRole.USER;

    return (
      <Card
        className="p-3 cursor-pointer hover:bg-accent transition-colors"
        onClick={onSelect}
      >
        <div className="flex items-start gap-2">
          {isUser ? (
            <User className="w-4 h-4 mt-0.5 text-muted-foreground" />
          ) : (
            <Bot className="w-4 h-4 mt-0.5 text-primary" />
          )}

          <div className="flex-1 min-w-0">
            <div className="flex items-center justify-between mb-1">
              <span className="text-xs font-medium">
                {isUser ? 'You' : 'Codex'}
              </span>
              <span className="text-xs text-muted-foreground">
                {formatTimestamp(result.message.timestamp)}
              </span>
            </div>

            <div
              className="text-sm"
              dangerouslySetInnerHTML={{ __html: result.highlightedContent }}
            />
          </div>
        </div>
      </Card>
    );
  }
  ```

#### useDebounce 훅
- [ ] `src/hooks/useDebounce.ts` 생성
  ```typescript
  import { useState, useEffect } from 'react';

  export function useDebounce<T>(value: T, delay: number): T {
    const [debouncedValue, setDebouncedValue] = useState<T>(value);

    useEffect(() => {
      const timer = setTimeout(() => {
        setDebouncedValue(value);
      }, delay);

      return () => {
        clearTimeout(timer);
      };
    }, [value, delay]);

    return debouncedValue;
  }
  ```

#### 키보드 단축키로 검색 열기
- [ ] `src/App.tsx`에 단축키 추가
  ```typescript
  import { useHotkeys } from 'react-hotkeys-hook';
  import { GlobalSearch } from '@/components/search/GlobalSearch';

  function App() {
    const [searchOpen, setSearchOpen] = useState(false);

    useHotkeys('ctrl+f, cmd+f', (e) => {
      e.preventDefault();
      setSearchOpen(true);
    });

    return (
      <>
        {/* ... existing code ... */}
        <GlobalSearch open={searchOpen} onOpenChange={setSearchOpen} />
      </>
    );
  }
  ```

#### react-hotkeys-hook 설치
- [ ] 키보드 단축키 라이브러리 설치
  ```bash
  pnpm add react-hotkeys-hook
  ```

### 예상 결과물
- 전체 세션 검색
- 실시간 검색 결과
- 하이라이팅
- 키보드 단축키

### Commit 메시지
```
feat(web-ui): add search functionality

- Create search utilities with highlighting
- Build GlobalSearch dialog component
- Implement SearchResultItem
- Add useDebounce hook
- Group results by session
- Support keyboard shortcut (Ctrl/Cmd+F)
- Install react-hotkeys-hook
```

---

## 5. 세션 내보내기 (Commit 23)

### 요구사항
- JSON 형식 내보내기
- Markdown 형식 내보내기
- 선택적 내보내기 (메시지 범위)
- 공유 가능한 형식

### 작업 내용

#### Markdown 내보내기
- [ ] `src/lib/export-markdown.ts` 생성
  ```typescript
  import { Session } from '@/types/session';
  import { Message, MessageRole } from '@/types/message';
  import { formatTimestamp } from './message-utils';

  export function exportSessionToMarkdown(session: Session): string {
    let markdown = '';

    // Header
    markdown += `# ${session.name}\n\n`;
    markdown += `**Created:** ${new Date(session.createdAt).toLocaleString()}\n`;
    markdown += `**Last Updated:** ${new Date(session.updatedAt).toLocaleString()}\n`;
    markdown += `**Messages:** ${session.metadata.messageCount}\n`;
    markdown += `**Tool Calls:** ${session.metadata.toolCallCount}\n`;
    if (session.metadata.model) {
      markdown += `**Model:** ${session.metadata.model}\n`;
    }
    markdown += '\n---\n\n';

    // Messages
    session.messages.forEach((message, index) => {
      markdown += formatMessageAsMarkdown(message, index + 1);
      markdown += '\n';
    });

    // Footer
    markdown += '\n---\n\n';
    markdown += `*Exported from Codex UI on ${new Date().toLocaleString()}*\n`;

    return markdown;
  }

  function formatMessageAsMarkdown(message: Message, index: number): string {
    let md = '';

    const role = message.role === MessageRole.USER ? '👤 **You**' : '🤖 **Codex**';
    const timestamp = formatTimestamp(message.timestamp);

    md += `## Message ${index}: ${role}\n`;
    md += `*${timestamp}*\n\n`;

    // Content
    message.content.forEach((content) => {
      if (content.type === 'text') {
        md += `${content.content}\n\n`;
      } else if (content.type === 'code') {
        md += `\`\`\`${content.language || ''}\n${content.content}\n\`\`\`\n\n`;
      } else if (content.type === 'image') {
        md += `![Image](${content.content})\n\n`;
      }
    });

    // Tool Calls
    if (message.toolCalls && message.toolCalls.length > 0) {
      md += '### 🔧 Tool Calls\n\n';
      message.toolCalls.forEach((tc, i) => {
        md += `#### ${i + 1}. \`${tc.name}\` - ${tc.status}\n\n`;

        md += '**Arguments:**\n```json\n';
        md += JSON.stringify(tc.arguments, null, 2);
        md += '\n```\n\n';

        if (tc.result) {
          md += '**Result:**\n```\n';
          md += tc.result;
          md += '\n```\n\n';
        }

        if (tc.error) {
          md += '**Error:**\n```\n';
          md += tc.error;
          md += '\n```\n\n';
        }
      });
    }

    md += '---\n\n';

    return md;
  }

  export function exportMessagesToMarkdown(
    messages: Message[],
    sessionName: string
  ): string {
    let markdown = `# ${sessionName}\n\n`;

    messages.forEach((message, index) => {
      markdown += formatMessageAsMarkdown(message, index + 1);
    });

    markdown += `\n*Exported on ${new Date().toLocaleString()}*\n`;

    return markdown;
  }
  ```

#### HTML 내보내기
- [ ] `src/lib/export-html.ts` 생성
  ```typescript
  import { Session } from '@/types/session';
  import { Message, MessageRole } from '@/types/message';

  export function exportSessionToHTML(session: Session): string {
    const html = `
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${session.name}</title>
  <style>
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      max-width: 800px;
      margin: 0 auto;
      padding: 20px;
      background: #f5f5f5;
    }
    .header {
      background: white;
      padding: 20px;
      border-radius: 8px;
      margin-bottom: 20px;
    }
    .message {
      background: white;
      padding: 16px;
      border-radius: 8px;
      margin-bottom: 12px;
    }
    .message.user {
      background: #e3f2fd;
    }
    .message.assistant {
      background: #f5f5f5;
    }
    .role {
      font-weight: bold;
      margin-bottom: 8px;
    }
    .timestamp {
      color: #666;
      font-size: 12px;
    }
    code {
      background: #f5f5f5;
      padding: 2px 6px;
      border-radius: 3px;
      font-family: 'Courier New', monospace;
    }
    pre {
      background: #1e1e1e;
      color: #d4d4d4;
      padding: 12px;
      border-radius: 6px;
      overflow-x: auto;
    }
    .tool-call {
      border-left: 3px solid #2196f3;
      padding-left: 12px;
      margin-top: 12px;
    }
  </style>
</head>
<body>
  <div class="header">
    <h1>${session.name}</h1>
    <p class="timestamp">
      Created: ${new Date(session.createdAt).toLocaleString()}<br>
      Updated: ${new Date(session.updatedAt).toLocaleString()}<br>
      Messages: ${session.metadata.messageCount}
    </p>
  </div>

  ${session.messages.map((msg) => formatMessageAsHTML(msg)).join('')}

  <p style="text-align: center; color: #999; margin-top: 40px;">
    Exported from Codex UI on ${new Date().toLocaleString()}
  </p>
</body>
</html>
`;

    return html;
  }

  function formatMessageAsHTML(message: Message): string {
    const isUser = message.role === MessageRole.USER;
    const roleClass = isUser ? 'user' : 'assistant';
    const roleName = isUser ? '👤 You' : '🤖 Codex';

    let content = '';
    message.content.forEach((c) => {
      if (c.type === 'text') {
        content += `<p>${escapeHTML(c.content)}</p>`;
      } else if (c.type === 'code') {
        content += `<pre><code>${escapeHTML(c.content)}</code></pre>`;
      }
    });

    let toolCalls = '';
    if (message.toolCalls && message.toolCalls.length > 0) {
      toolCalls = message.toolCalls
        .map(
          (tc) => `
        <div class="tool-call">
          <strong>🔧 ${tc.name}</strong> - ${tc.status}<br>
          <pre><code>${escapeHTML(JSON.stringify(tc.arguments, null, 2))}</code></pre>
          ${tc.result ? `<pre><code>${escapeHTML(tc.result)}</code></pre>` : ''}
        </div>
      `
        )
        .join('');
    }

    return `
    <div class="message ${roleClass}">
      <div class="role">${roleName}</div>
      <div class="timestamp">${new Date(message.timestamp).toLocaleString()}</div>
      ${content}
      ${toolCalls}
    </div>
  `;
  }

  function escapeHTML(str: string): string {
    return str
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#039;');
  }
  ```

#### ExportOptionsDialog 컴포넌트
- [ ] `src/components/session/ExportOptionsDialog.tsx` 생성
  ```typescript
  import { useState } from 'react';
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
  } from '@/components/ui/dialog';
  import { Button } from '@/components/ui/button';
  import { Label } from '@/components/ui/label';
  import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group';
  import { Session } from '@/types/session';
  import { exportSessionToMarkdown } from '@/lib/export-markdown';
  import { exportSessionToHTML } from '@/lib/export-html';
  import { exportSessionToJSON } from '@/lib/session-export';
  import { FileDown } from 'lucide-react';
  import { toast } from '@/lib/toast';

  interface ExportOptionsDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    session: Session;
  }

  type ExportFormat = 'json' | 'markdown' | 'html';

  export function ExportOptionsDialog({
    open,
    onOpenChange,
    session,
  }: ExportOptionsDialogProps) {
    const [format, setFormat] = useState<ExportFormat>('markdown');

    const handleExport = () => {
      let content: string;
      let extension: string;
      let mimeType: string;

      switch (format) {
        case 'json':
          content = exportSessionToJSON(session);
          extension = 'json';
          mimeType = 'application/json';
          break;
        case 'markdown':
          content = exportSessionToMarkdown(session);
          extension = 'md';
          mimeType = 'text/markdown';
          break;
        case 'html':
          content = exportSessionToHTML(session);
          extension = 'html';
          mimeType = 'text/html';
          break;
      }

      const blob = new Blob([content], { type: mimeType });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${session.name.replace(/[^a-z0-9]/gi, '-')}.${extension}`;
      a.click();
      URL.revokeObjectURL(url);

      toast.success(`Exported as ${format.toUpperCase()}`);
      onOpenChange(false);
    };

    return (
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Export Session</DialogTitle>
            <DialogDescription>
              Choose a format to export "{session.name}"
            </DialogDescription>
          </DialogHeader>

          <RadioGroup value={format} onValueChange={(v) => setFormat(v as ExportFormat)}>
            <div className="flex items-center space-x-2">
              <RadioGroupItem value="markdown" id="markdown" />
              <Label htmlFor="markdown" className="flex-1">
                <div className="font-medium">Markdown (.md)</div>
                <div className="text-sm text-muted-foreground">
                  Human-readable format, good for documentation
                </div>
              </Label>
            </div>

            <div className="flex items-center space-x-2">
              <RadioGroupItem value="html" id="html" />
              <Label htmlFor="html" className="flex-1">
                <div className="font-medium">HTML (.html)</div>
                <div className="text-sm text-muted-foreground">
                  Standalone webpage, can be viewed in any browser
                </div>
              </Label>
            </div>

            <div className="flex items-center space-x-2">
              <RadioGroupItem value="json" id="json" />
              <Label htmlFor="json" className="flex-1">
                <div className="font-medium">JSON (.json)</div>
                <div className="text-sm text-muted-foreground">
                  Complete data structure, can be re-imported
                </div>
              </Label>
            </div>
          </RadioGroup>

          <DialogFooter>
            <Button variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button onClick={handleExport}>
              <FileDown className="w-4 h-4 mr-2" />
              Export
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
  ```

#### shadcn RadioGroup 설치
- [ ] RadioGroup 컴포넌트 설치
  ```bash
  npx shadcn@latest add radio-group
  ```

### 예상 결과물
- Markdown 내보내기
- HTML 내보내기
- JSON 내보내기
- 포맷 선택 다이얼로그

### Commit 메시지
```
feat(web-ui): export sessions in multiple formats

- Create Markdown export with formatting
- Implement HTML export with styling
- Add ExportOptionsDialog for format selection
- Support JSON, Markdown, and HTML formats
- Generate downloadable files
- Install radio-group component
```

---

## 6. 세션 통계 (Commit 24)

### 요구사항
- 세션별 통계 집계
- 메시지 수, 도구 호출 통계
- 차트 시각화
- 통계 대시보드

### 작업 내용

#### 통계 계산 유틸리티
- [ ] `src/lib/session-stats.ts` 생성
  ```typescript
  import { Session } from '@/types/session';
  import { MessageRole } from '@/types/message';

  export interface SessionStats {
    totalSessions: number;
    totalMessages: number;
    totalToolCalls: number;
    averageMessagesPerSession: number;
    mostActiveSession: {
      id: string;
      name: string;
      messageCount: number;
    } | null;
    messagesByRole: {
      user: number;
      assistant: number;
      system: number;
    };
    toolCallsByTool: Map<string, number>;
    sessionsOverTime: {
      date: string;
      count: number;
    }[];
  }

  export function calculateSessionStats(sessions: Session[]): SessionStats {
    let totalMessages = 0;
    let totalToolCalls = 0;
    const messagesByRole = { user: 0, assistant: 0, system: 0 };
    const toolCallsByTool = new Map<string, number>();
    const sessionsByDate = new Map<string, number>();

    let mostActiveSession: SessionStats['mostActiveSession'] = null;

    sessions.forEach((session) => {
      totalMessages += session.metadata.messageCount;
      totalToolCalls += session.metadata.toolCallCount;

      // Track most active session
      if (
        !mostActiveSession ||
        session.metadata.messageCount > mostActiveSession.messageCount
      ) {
        mostActiveSession = {
          id: session.id,
          name: session.name,
          messageCount: session.metadata.messageCount,
        };
      }

      // Count messages by role
      session.messages.forEach((msg) => {
        if (msg.role in messagesByRole) {
          messagesByRole[msg.role]++;
        }

        // Count tool calls by tool name
        msg.toolCalls?.forEach((tc) => {
          const count = toolCallsByTool.get(tc.name) || 0;
          toolCallsByTool.set(tc.name, count + 1);
        });
      });

      // Group sessions by date
      const date = new Date(session.createdAt).toLocaleDateString();
      const count = sessionsByDate.get(date) || 0;
      sessionsByDate.set(date, count + 1);
    });

    const sessionsOverTime = Array.from(sessionsByDate.entries())
      .map(([date, count]) => ({ date, count }))
      .sort((a, b) => new Date(a.date).getTime() - new Date(b.date).getTime());

    return {
      totalSessions: sessions.length,
      totalMessages,
      totalToolCalls,
      averageMessagesPerSession:
        sessions.length > 0 ? totalMessages / sessions.length : 0,
      mostActiveSession,
      messagesByRole,
      toolCallsByTool,
      sessionsOverTime,
    };
  }

  export function getTopTools(
    toolCalls: Map<string, number>,
    limit: number = 5
  ): Array<{ name: string; count: number }> {
    return Array.from(toolCalls.entries())
      .map(([name, count]) => ({ name, count }))
      .sort((a, b) => b.count - a.count)
      .slice(0, limit);
  }
  ```

#### SessionStats 컴포넌트
- [ ] `src/components/session/SessionStats.tsx` 생성
  ```typescript
  import { useEffect, useState } from 'react';
  import { getAllSessions } from '@/lib/indexeddb';
  import { calculateSessionStats, getTopTools, SessionStats as Stats } from '@/lib/session-stats';
  import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
  import { MessageSquare, Wrench, TrendingUp, Award } from 'lucide-react';

  export function SessionStats() {
    const [stats, setStats] = useState<Stats | null>(null);
    const [isLoading, setIsLoading] = useState(true);

    useEffect(() => {
      loadStats();
    }, []);

    const loadStats = async () => {
      setIsLoading(true);
      const sessions = await getAllSessions();
      const calculatedStats = calculateSessionStats(sessions);
      setStats(calculatedStats);
      setIsLoading(false);
    };

    if (isLoading || !stats) {
      return <div>Loading stats...</div>;
    }

    const topTools = getTopTools(stats.toolCallsByTool, 5);

    return (
      <div className="space-y-4">
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <Card>
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium">Total Sessions</CardTitle>
              <MessageSquare className="h-4 w-4 text-muted-foreground" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{stats.totalSessions}</div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium">Total Messages</CardTitle>
              <MessageSquare className="h-4 w-4 text-muted-foreground" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{stats.totalMessages}</div>
              <p className="text-xs text-muted-foreground">
                {stats.averageMessagesPerSession.toFixed(1)} avg per session
              </p>
            </CardContent>
          </Card>

          <Card>
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium">Tool Calls</CardTitle>
              <Wrench className="h-4 w-4 text-muted-foreground" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{stats.totalToolCalls}</div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium">Most Active</CardTitle>
              <Award className="h-4 w-4 text-muted-foreground" />
            </CardHeader>
            <CardContent>
              <div className="text-sm font-medium truncate">
                {stats.mostActiveSession?.name || 'N/A'}
              </div>
              <p className="text-xs text-muted-foreground">
                {stats.mostActiveSession?.messageCount || 0} messages
              </p>
            </CardContent>
          </Card>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <Card>
            <CardHeader>
              <CardTitle>Messages by Role</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-sm">User Messages</span>
                <span className="font-bold">{stats.messagesByRole.user}</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm">Assistant Messages</span>
                <span className="font-bold">{stats.messagesByRole.assistant}</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm">System Messages</span>
                <span className="font-bold">{stats.messagesByRole.system}</span>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Top Tools Used</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              {topTools.length > 0 ? (
                topTools.map((tool) => (
                  <div key={tool.name} className="flex items-center justify-between">
                    <span className="text-sm font-mono">{tool.name}</span>
                    <span className="font-bold">{tool.count}</span>
                  </div>
                ))
              ) : (
                <div className="text-sm text-muted-foreground">No tools used yet</div>
              )}
            </CardContent>
          </Card>
        </div>
      </div>
    );
  }
  ```

#### shadcn Card 설치
- [ ] Card 컴포넌트 설치 (이미 설치되어 있을 수 있음)
  ```bash
  npx shadcn@latest add card
  ```

#### StatsPage 생성
- [ ] `src/pages/StatsPage.tsx` 생성
  ```typescript
  import { SessionStats } from '@/components/session/SessionStats';

  export function StatsPage() {
    return (
      <div className="container mx-auto p-6">
        <div className="mb-6">
          <h1 className="text-3xl font-bold">Statistics</h1>
          <p className="text-muted-foreground">
            Overview of your Codex usage and activity
          </p>
        </div>

        <SessionStats />
      </div>
    );
  }
  ```

#### 라우팅에 추가
- [ ] `src/App.tsx`에 라우트 추가
  ```typescript
  <Route path="/stats" element={<StatsPage />} />
  ```

### 예상 결과물
- 세션 통계 계산
- 통계 대시보드
- 메시지/도구 사용 분석
- 시각화 카드

### Commit 메시지
```
feat(web-ui): add session statistics dashboard

- Create session stats calculation utilities
- Build SessionStats component with cards
- Display total sessions, messages, tool calls
- Show messages by role breakdown
- List top 5 most used tools
- Add StatsPage and route
```

---

## Day 4 완료 체크리스트

- [ ] 세션 관리 구조 (IndexedDB, Zustand 스토어)
- [ ] 세션 UI (목록, 생성, 삭제, 이름 변경, 고정)
- [ ] 히스토리 저장/로드 (자동 저장, 복원, 내보내기/가져오기)
- [ ] 검색 기능 (전체 세션 검색, 하이라이팅, 키보드 단축키)
- [ ] 세션 내보내기 (JSON, Markdown, HTML)
- [ ] 세션 통계 (대시보드, 차트, 분석)
- [ ] 모든 커밋 메시지 명확하게 작성
- [ ] 기능 테스트 및 검증

---

## 다음 단계 (Day 5 예고)

1. 설정 관리 (API 키, 모델 선택)
2. 인증 설정 UI
3. 모델 설정 UI
4. 테마 및 외관 설정
5. 고급 설정 (MCP, 샌드박스)
6. 설정 검증 및 백업

---

## 참고 자료

- [IndexedDB API](https://developer.mozilla.org/en-US/docs/Web/API/IndexedDB_API)
- [idb Library](https://github.com/jakearchibald/idb)
- [react-hotkeys-hook](https://github.com/JohannesKlauss/react-hotkeys-hook)

---

**Last Updated**: 2025-11-20
**Version**: 1.0
**Day**: 4 / 7
