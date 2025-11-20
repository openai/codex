# Day 6 TODO - 고급 기능 및 개선

## 목표
애플리케이션의 사용자 경험을 향상시키기 위한 고급 기능, 성능 최적화, 접근성, 반응형 디자인을 구현합니다.

---

## 1. 키보드 단축키 시스템 (Commit 31)

### 요구사항
- 전역 키보드 단축키
- 컨텍스트별 단축키
- 단축키 커스터마이징
- 도움말 모달

### 작업 내용

#### 키보드 단축키 정의
- [ ] `src/lib/keyboard-shortcuts.ts` 생성
  ```typescript
  export interface KeyboardShortcut {
    id: string;
    key: string;
    description: string;
    category: string;
    action: () => void;
  }

  export const SHORTCUT_CATEGORIES = {
    GENERAL: 'General',
    NAVIGATION: 'Navigation',
    EDITING: 'Editing',
    SESSION: 'Session',
  };

  export function createShortcutKey(
    modifiers: string[],
    key: string
  ): string {
    const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
    const ctrlKey = isMac ? 'cmd' : 'ctrl';

    const mappedModifiers = modifiers.map((m) =>
      m === 'mod' ? ctrlKey : m
    );

    return [...mappedModifiers, key].join('+');
  }

  export function formatShortcutKey(key: string): string {
    const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;

    return key
      .split('+')
      .map((part) => {
        switch (part) {
          case 'cmd':
            return '⌘';
          case 'ctrl':
            return isMac ? '⌃' : 'Ctrl';
          case 'alt':
            return isMac ? '⌥' : 'Alt';
          case 'shift':
            return isMac ? '⇧' : 'Shift';
          default:
            return part.toUpperCase();
        }
      })
      .join(isMac ? '' : '+');
  }
  ```

#### KeyboardShortcuts 컴포넌트
- [ ] `src/components/keyboard/KeyboardShortcuts.tsx` 생성
  ```typescript
  import { useEffect } from 'react';
  import { useHotkeys } from 'react-hotkeys-hook';
  import { useNavigate } from 'react-router-dom';
  import { useSessionStore } from '@/store/session-store';
  import { useChatStore } from '@/store/chat-store';

  export function KeyboardShortcuts() {
    const navigate = useNavigate();
    const { createSession } = useSessionStore();
    const { clearMessages } = useChatStore();

    // General shortcuts
    useHotkeys('mod+k', (e) => {
      e.preventDefault();
      // Open command palette (implemented in next commit)
      const event = new CustomEvent('openCommandPalette');
      window.dispatchEvent(event);
    });

    useHotkeys('mod+/', (e) => {
      e.preventDefault();
      // Open keyboard shortcuts help
      const event = new CustomEvent('openShortcutsHelp');
      window.dispatchEvent(event);
    });

    // Navigation shortcuts
    useHotkeys('mod+1', (e) => {
      e.preventDefault();
      navigate('/');
    });

    useHotkeys('mod+2', (e) => {
      e.preventDefault();
      navigate('/chat');
    });

    useHotkeys('mod+3', (e) => {
      e.preventDefault();
      navigate('/stats');
    });

    useHotkeys('mod+,', (e) => {
      e.preventDefault();
      navigate('/settings');
    });

    // Session shortcuts
    useHotkeys('mod+n', async (e) => {
      e.preventDefault();
      await createSession();
    });

    useHotkeys('mod+shift+n', (e) => {
      e.preventDefault();
      clearMessages();
    });

    // Search
    useHotkeys('mod+f', (e) => {
      e.preventDefault();
      const event = new CustomEvent('openGlobalSearch');
      window.dispatchEvent(event);
    });

    return null;
  }
  ```

#### ShortcutsHelpDialog 컴포넌트
- [ ] `src/components/keyboard/ShortcutsHelpDialog.tsx` 생성
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
  import { Search } from 'lucide-react';
  import { formatShortcutKey } from '@/lib/keyboard-shortcuts';

  interface Shortcut {
    key: string;
    description: string;
    category: string;
  }

  const SHORTCUTS: Shortcut[] = [
    // General
    { key: 'mod+k', description: 'Open command palette', category: 'General' },
    { key: 'mod+/', description: 'Show keyboard shortcuts', category: 'General' },
    { key: 'mod+f', description: 'Global search', category: 'General' },

    // Navigation
    { key: 'mod+1', description: 'Go to Home', category: 'Navigation' },
    { key: 'mod+2', description: 'Go to Chat', category: 'Navigation' },
    { key: 'mod+3', description: 'Go to Statistics', category: 'Navigation' },
    { key: 'mod+,', description: 'Open Settings', category: 'Navigation' },

    // Session
    { key: 'mod+n', description: 'New session', category: 'Session' },
    { key: 'mod+shift+n', description: 'Clear current session', category: 'Session' },

    // Editing
    { key: 'enter', description: 'Send message', category: 'Editing' },
    { key: 'shift+enter', description: 'New line in message', category: 'Editing' },
    { key: 'mod+z', description: 'Undo', category: 'Editing' },
    { key: 'mod+shift+z', description: 'Redo', category: 'Editing' },
  ];

  export function ShortcutsHelpDialog() {
    const [open, setOpen] = useState(false);
    const [search, setSearch] = useState('');

    useEffect(() => {
      const handleOpen = () => setOpen(true);
      window.addEventListener('openShortcutsHelp', handleOpen);
      return () => window.removeEventListener('openShortcutsHelp', handleOpen);
    }, []);

    const filteredShortcuts = SHORTCUTS.filter(
      (s) =>
        s.description.toLowerCase().includes(search.toLowerCase()) ||
        s.category.toLowerCase().includes(search.toLowerCase())
    );

    const groupedShortcuts = filteredShortcuts.reduce((acc, shortcut) => {
      if (!acc[shortcut.category]) {
        acc[shortcut.category] = [];
      }
      acc[shortcut.category].push(shortcut);
      return acc;
    }, {} as Record<string, Shortcut[]>);

    return (
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>Keyboard Shortcuts</DialogTitle>
          </DialogHeader>

          <div className="relative">
            <Search className="absolute left-3 top-3 h-4 w-4 text-muted-foreground" />
            <Input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search shortcuts..."
              className="pl-9"
            />
          </div>

          <ScrollArea className="h-[400px]">
            <div className="space-y-4">
              {Object.entries(groupedShortcuts).map(([category, shortcuts]) => (
                <div key={category}>
                  <h3 className="font-semibold mb-2">{category}</h3>
                  <div className="space-y-1">
                    {shortcuts.map((shortcut, index) => (
                      <div
                        key={index}
                        className="flex items-center justify-between py-2 px-3 rounded hover:bg-muted"
                      >
                        <span className="text-sm">{shortcut.description}</span>
                        <kbd className="px-2 py-1 text-xs font-mono bg-muted border rounded">
                          {formatShortcutKey(shortcut.key)}
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

#### App.tsx에 통합
- [ ] `src/App.tsx`에 추가
  ```typescript
  import { KeyboardShortcuts } from '@/components/keyboard/KeyboardShortcuts';
  import { ShortcutsHelpDialog } from '@/components/keyboard/ShortcutsHelpDialog';

  function App() {
    return (
      <>
        <KeyboardShortcuts />
        <ShortcutsHelpDialog />
        {/* ... existing code ... */}
      </>
    );
  }
  ```

### 예상 결과물
- 전역 키보드 단축키
- 도움말 다이얼로그
- 검색 가능한 단축키 목록
- 플랫폼별 키 표시

### Commit 메시지
```
feat(web-ui): implement keyboard shortcuts

- Create keyboard shortcuts system
- Add global shortcuts for navigation and actions
- Build ShortcutsHelpDialog with search
- Support platform-specific key formatting
- Integrate with app navigation
```

---

## 2. 명령 팔레트 (Commit 32)

### 요구사항
- Cmd/Ctrl+K로 열기
- Fuzzy 검색
- 최근 명령어 표시
- 액션 실행

### 작업 내용

#### CommandPalette 컴포넌트
- [ ] `src/components/command/CommandPalette.tsx` 생성
  ```typescript
  import { useState, useEffect } from 'react';
  import { useNavigate } from 'react-router-dom';
  import {
    CommandDialog,
    CommandEmpty,
    CommandGroup,
    CommandInput,
    CommandItem,
    CommandList,
    CommandSeparator,
  } from '@/components/ui/command';
  import { useSessionStore } from '@/store/session-store';
  import { useChatStore } from '@/store/chat-store';
  import {
    Search,
    Plus,
    Settings,
    BarChart3,
    FileText,
    Trash2,
    Download,
    Upload,
  } from 'lucide-react';

  interface Command {
    id: string;
    label: string;
    icon: any;
    action: () => void;
    keywords?: string[];
  }

  export function CommandPalette() {
    const [open, setOpen] = useState(false);
    const navigate = useNavigate();
    const { createSession, sessions, setCurrentSession } = useSessionStore();
    const { clearMessages } = useChatStore();

    useEffect(() => {
      const handleOpen = () => setOpen(true);
      window.addEventListener('openCommandPalette', handleOpen);

      // Keyboard shortcut
      const down = (e: KeyboardEvent) => {
        if (e.key === 'k' && (e.metaKey || e.ctrlKey)) {
          e.preventDefault();
          setOpen((open) => !open);
        }
      };

      document.addEventListener('keydown', down);
      return () => {
        window.removeEventListener('openCommandPalette', handleOpen);
        document.removeEventListener('keydown', down);
      };
    }, []);

    const commands: Command[] = [
      {
        id: 'new-session',
        label: 'New Session',
        icon: Plus,
        action: async () => {
          await createSession();
          setOpen(false);
        },
      },
      {
        id: 'clear-session',
        label: 'Clear Current Session',
        icon: Trash2,
        action: () => {
          clearMessages();
          setOpen(false);
        },
      },
      {
        id: 'search',
        label: 'Search All Sessions',
        icon: Search,
        action: () => {
          setOpen(false);
          window.dispatchEvent(new CustomEvent('openGlobalSearch'));
        },
        keywords: ['find', 'search'],
      },
      {
        id: 'settings',
        label: 'Open Settings',
        icon: Settings,
        action: () => {
          navigate('/settings');
          setOpen(false);
        },
      },
      {
        id: 'stats',
        label: 'View Statistics',
        icon: BarChart3,
        action: () => {
          navigate('/stats');
          setOpen(false);
        },
        keywords: ['analytics', 'metrics'],
      },
    ];

    return (
      <CommandDialog open={open} onOpenChange={setOpen}>
        <CommandInput placeholder="Type a command or search..." />
        <CommandList>
          <CommandEmpty>No results found.</CommandEmpty>

          <CommandGroup heading="Actions">
            {commands.map((command) => {
              const Icon = command.icon;
              return (
                <CommandItem
                  key={command.id}
                  onSelect={command.action}
                  keywords={command.keywords}
                >
                  <Icon className="mr-2 h-4 w-4" />
                  <span>{command.label}</span>
                </CommandItem>
              );
            })}
          </CommandGroup>

          {sessions.length > 0 && (
            <>
              <CommandSeparator />
              <CommandGroup heading="Recent Sessions">
                {sessions.slice(0, 5).map((session) => (
                  <CommandItem
                    key={session.id}
                    onSelect={() => {
                      setCurrentSession(session.id);
                      navigate('/chat');
                      setOpen(false);
                    }}
                  >
                    <FileText className="mr-2 h-4 w-4" />
                    <span>{session.name}</span>
                  </CommandItem>
                ))}
              </CommandGroup>
            </>
          )}
        </CommandList>
      </CommandDialog>
    );
  }
  ```

#### shadcn Command 설치
- [ ] Command 컴포넌트 설치
  ```bash
  npx shadcn@latest add command
  ```

#### cmdk 라이브러리 (Command가 사용)
- Command 컴포넌트는 cmdk를 사용하므로 자동으로 설치됨

### 예상 결과물
- Cmd/Ctrl+K 명령 팔레트
- Fuzzy 검색
- 최근 세션 표시
- 액션 실행

### Commit 메시지
```
feat(web-ui): add command palette

- Create CommandPalette component
- Support fuzzy search with cmdk
- Add quick actions (new session, search, settings)
- Show recent sessions
- Install command component
- Trigger with Cmd/Ctrl+K
```

---

## 3. 성능 최적화 (Commit 33)

### 요구사항
- React.memo 적용
- useMemo, useCallback 최적화
- 가상 스크롤링
- 코드 스플리팅

### 작업 내용

#### 가상 스크롤링 적용
- [ ] react-window 설치
  ```bash
  pnpm add react-window
  pnpm add -D @types/react-window
  ```

- [ ] `src/components/chat/VirtualMessageList.tsx` 생성
  ```typescript
  import { useRef, useEffect } from 'react';
  import { VariableSizeList as List } from 'react-window';
  import AutoSizer from 'react-virtualized-auto-sizer';
  import { useChatStore } from '@/store/chat-store';
  import { MessageItem } from './MessageItem';

  export function VirtualMessageList() {
    const messages = useChatStore((state) => state.messages);
    const listRef = useRef<List>(null);

    useEffect(() => {
      // Scroll to bottom when new message arrives
      if (listRef.current && messages.length > 0) {
        listRef.current.scrollToItem(messages.length - 1, 'end');
      }
    }, [messages.length]);

    const getItemSize = (index: number) => {
      // Estimate item height based on message content
      const message = messages[index];
      let height = 80; // Base height

      message.content.forEach((content) => {
        if (content.type === 'text') {
          height += Math.ceil(content.content.length / 80) * 24;
        } else if (content.type === 'code') {
          height += 200;
        }
      });

      if (message.toolCalls && message.toolCalls.length > 0) {
        height += message.toolCalls.length * 60;
      }

      return Math.min(height, 1000);
    };

    return (
      <AutoSizer>
        {({ height, width }) => (
          <List
            ref={listRef}
            height={height}
            width={width}
            itemCount={messages.length}
            itemSize={getItemSize}
            overscanCount={3}
          >
            {({ index, style }) => (
              <div style={style}>
                <MessageItem message={messages[index]} />
              </div>
            )}
          </List>
        )}
      </AutoSizer>
    );
  }
  ```

#### 컴포넌트 메모이제이션
- [ ] MessageItem 최적화
  ```typescript
  // src/components/chat/MessageItem.tsx
  import { memo } from 'react';

  export const MessageItem = memo(function MessageItem({ message }: MessageItemProps) {
    // ... existing code ...
  }, (prevProps, nextProps) => {
    // Custom comparison
    return (
      prevProps.message.id === nextProps.message.id &&
      prevProps.message.status === nextProps.message.status &&
      prevProps.message.content.length === nextProps.message.content.length
    );
  });
  ```

#### 코드 스플리팅
- [ ] `src/App.tsx`에서 lazy loading 적용
  ```typescript
  import { lazy, Suspense } from 'react';
  import { Loader2 } from 'lucide-react';

  const ChatPage = lazy(() => import('@/pages/ChatPage'));
  const SettingsPage = lazy(() => import('@/pages/SettingsPage'));
  const StatsPage = lazy(() => import('@/pages/StatsPage'));

  function LoadingFallback() {
    return (
      <div className="flex items-center justify-center h-screen">
        <Loader2 className="w-8 h-8 animate-spin" />
      </div>
    );
  }

  function App() {
    return (
      <Suspense fallback={<LoadingFallback />}>
        <Routes>
          <Route path="/chat" element={<ChatPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="/stats" element={<StatsPage />} />
        </Routes>
      </Suspense>
    );
  }
  ```

#### 이미지 지연 로딩
- [ ] `src/components/LazyImage.tsx` 생성
  ```typescript
  import { useState, useEffect, useRef } from 'react';
  import { cn } from '@/lib/utils';

  interface LazyImageProps {
    src: string;
    alt: string;
    className?: string;
    placeholder?: string;
  }

  export function LazyImage({ src, alt, className, placeholder }: LazyImageProps) {
    const [isLoaded, setIsLoaded] = useState(false);
    const [isInView, setIsInView] = useState(false);
    const imgRef = useRef<HTMLImageElement>(null);

    useEffect(() => {
      if (!imgRef.current) return;

      const observer = new IntersectionObserver(
        ([entry]) => {
          if (entry.isIntersecting) {
            setIsInView(true);
            observer.disconnect();
          }
        },
        { threshold: 0.1 }
      );

      observer.observe(imgRef.current);

      return () => observer.disconnect();
    }, []);

    return (
      <img
        ref={imgRef}
        src={isInView ? src : placeholder}
        alt={alt}
        className={cn(className, !isLoaded && 'blur-sm')}
        onLoad={() => setIsLoaded(true)}
      />
    );
  }
  ```

#### 디바운스 및 스로틀 유틸리티
- [ ] `src/lib/utils.ts`에 추가
  ```typescript
  export function debounce<T extends (...args: any[]) => any>(
    func: T,
    wait: number
  ): (...args: Parameters<T>) => void {
    let timeout: NodeJS.Timeout;

    return function executedFunction(...args: Parameters<T>) {
      const later = () => {
        clearTimeout(timeout);
        func(...args);
      };

      clearTimeout(timeout);
      timeout = setTimeout(later, wait);
    };
  }

  export function throttle<T extends (...args: any[]) => any>(
    func: T,
    limit: number
  ): (...args: Parameters<T>) => void {
    let inThrottle: boolean;

    return function executedFunction(...args: Parameters<T>) {
      if (!inThrottle) {
        func(...args);
        inThrottle = true;
        setTimeout(() => (inThrottle = false), limit);
      }
    };
  }
  ```

### 예상 결과물
- 가상 스크롤링
- 메모이제이션
- 코드 스플리팅
- 지연 로딩

### Commit 메시지
```
perf(web-ui): optimize component rendering

- Add virtual scrolling with react-window
- Memoize MessageItem component
- Implement code splitting with lazy loading
- Create LazyImage component
- Add debounce and throttle utilities
- Install react-window and react-virtualized-auto-sizer
```

---

## 4. 로딩 상태 개선 (Commit 34)

### 요구사항
- 스켈레톤 UI
- 스피너 컴포넌트
- 진행률 표시
- Suspense 경계

### 작업 내용

#### Skeleton 컴포넌트
- [ ] shadcn Skeleton 설치
  ```bash
  npx shadcn@latest add skeleton
  ```

#### MessageSkeleton 컴포넌트
- [ ] `src/components/chat/MessageSkeleton.tsx` 생성
  ```typescript
  import { Skeleton } from '@/components/ui/skeleton';

  export function MessageSkeleton() {
    return (
      <div className="flex gap-3 p-4">
        <Skeleton className="w-8 h-8 rounded-full" />
        <div className="flex-1 space-y-2">
          <Skeleton className="h-4 w-32" />
          <Skeleton className="h-20 w-full" />
          <Skeleton className="h-4 w-3/4" />
        </div>
      </div>
    );
  }

  export function MessageListSkeleton() {
    return (
      <div className="space-y-4">
        {Array.from({ length: 5 }).map((_, i) => (
          <MessageSkeleton key={i} />
        ))}
      </div>
    );
  }
  ```

#### SessionListSkeleton 컴포넌트
- [ ] `src/components/session/SessionListSkeleton.tsx` 생성
  ```typescript
  import { Skeleton } from '@/components/ui/skeleton';

  export function SessionListSkeleton() {
    return (
      <div className="p-2 space-y-2">
        {Array.from({ length: 8 }).map((_, i) => (
          <div key={i} className="flex gap-2 p-2">
            <Skeleton className="w-4 h-4" />
            <div className="flex-1 space-y-1">
              <Skeleton className="h-4 w-full" />
              <Skeleton className="h-3 w-2/3" />
            </div>
          </div>
        ))}
      </div>
    );
  }
  ```

#### LoadingSpinner 컴포넌트
- [ ] `src/components/LoadingSpinner.tsx` 생성
  ```typescript
  import { Loader2 } from 'lucide-react';
  import { cn } from '@/lib/utils';

  interface LoadingSpinnerProps {
    size?: 'sm' | 'md' | 'lg';
    className?: string;
    text?: string;
  }

  export function LoadingSpinner({ size = 'md', className, text }: LoadingSpinnerProps) {
    const sizeClasses = {
      sm: 'w-4 h-4',
      md: 'w-8 h-8',
      lg: 'w-12 h-12',
    };

    return (
      <div className={cn('flex flex-col items-center justify-center gap-2', className)}>
        <Loader2 className={cn('animate-spin text-primary', sizeClasses[size])} />
        {text && <p className="text-sm text-muted-foreground">{text}</p>}
      </div>
    );
  }
  ```

#### ProgressBar 컴포넌트
- [ ] `src/components/ProgressBar.tsx` 생성
  ```typescript
  import { useEffect, useState } from 'react';
  import { Progress } from '@/components/ui/progress';

  interface ProgressBarProps {
    isLoading: boolean;
  }

  export function ProgressBar({ isLoading }: ProgressBarProps) {
    const [progress, setProgress] = useState(0);

    useEffect(() => {
      if (!isLoading) {
        setProgress(0);
        return;
      }

      const interval = setInterval(() => {
        setProgress((prev) => {
          if (prev >= 90) return prev;
          return prev + Math.random() * 10;
        });
      }, 500);

      return () => clearInterval(interval);
    }, [isLoading]);

    useEffect(() => {
      if (!isLoading && progress > 0) {
        setProgress(100);
        setTimeout(() => setProgress(0), 500);
      }
    }, [isLoading]);

    if (progress === 0) return null;

    return (
      <div className="fixed top-0 left-0 right-0 z-50">
        <Progress value={progress} className="h-1 rounded-none" />
      </div>
    );
  }
  ```

#### 전역 로딩 상태
- [ ] `src/store/ui-store.ts` 생성
  ```typescript
  import { create } from 'zustand';

  interface UIState {
    isLoading: boolean;
    loadingMessage: string | null;
    setLoading: (loading: boolean, message?: string) => void;
  }

  export const useUIStore = create<UIState>((set) => ({
    isLoading: false,
    loadingMessage: null,

    setLoading: (loading, message) => {
      set({ isLoading: loading, loadingMessage: message || null });
    },
  }));
  ```

### 예상 결과물
- 스켈레톤 UI
- 로딩 스피너
- 진행률 바
- 전역 로딩 상태

### Commit 메시지
```
feat(web-ui): improve loading states

- Add Skeleton components for messages and sessions
- Create LoadingSpinner component with sizes
- Implement ProgressBar for page transitions
- Add global UI state store
- Install skeleton component
```

---

## 5. 접근성 개선 (Commit 35)

### 요구사항
- 키보드 네비게이션
- ARIA 레이블
- 포커스 관리
- 스크린 리더 지원

### 작업 내용

#### 포커스 관리
- [ ] `src/hooks/useFocusTrap.ts` 생성
  ```typescript
  import { useEffect, useRef } from 'react';

  export function useFocusTrap(active: boolean) {
    const ref = useRef<HTMLElement>(null);

    useEffect(() => {
      if (!active || !ref.current) return;

      const element = ref.current;
      const focusableElements = element.querySelectorAll(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );

      const firstElement = focusableElements[0] as HTMLElement;
      const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

      const handleKeyDown = (e: KeyboardEvent) => {
        if (e.key !== 'Tab') return;

        if (e.shiftKey) {
          if (document.activeElement === firstElement) {
            e.preventDefault();
            lastElement?.focus();
          }
        } else {
          if (document.activeElement === lastElement) {
            e.preventDefault();
            firstElement?.focus();
          }
        }
      };

      element.addEventListener('keydown', handleKeyDown);
      firstElement?.focus();

      return () => {
        element.removeEventListener('keydown', handleKeyDown);
      };
    }, [active]);

    return ref;
  }
  ```

#### Skip Links
- [ ] `src/components/SkipLinks.tsx` 생성
  ```typescript
  export function SkipLinks() {
    return (
      <div className="sr-only focus-within:not-sr-only">
        <a
          href="#main-content"
          className="fixed top-0 left-0 bg-primary text-primary-foreground px-4 py-2 z-50 focus:outline-none focus:ring-2"
        >
          Skip to main content
        </a>
        <a
          href="#navigation"
          className="fixed top-0 left-20 bg-primary text-primary-foreground px-4 py-2 z-50 focus:outline-none focus:ring-2"
        >
          Skip to navigation
        </a>
      </div>
    );
  }
  ```

#### ARIA 개선
- [ ] MessageInput에 ARIA 추가
  ```typescript
  <Textarea
    ref={textareaRef}
    value={input}
    onChange={(e) => setInput(e.target.value)}
    placeholder="Type a message..."
    aria-label="Message input"
    aria-describedby="message-help"
    role="textbox"
    aria-multiline="true"
  />
  <span id="message-help" className="sr-only">
    Press Enter to send, Shift+Enter for new line
  </span>
  ```

#### 스크린 리더 알림
- [ ] `src/components/LiveRegion.tsx` 생성
  ```typescript
  import { useEffect, useState } from 'react';

  interface LiveRegionProps {
    message: string;
    politeness?: 'polite' | 'assertive';
  }

  export function LiveRegion({ message, politeness = 'polite' }: LiveRegionProps) {
    const [announcement, setAnnouncement] = useState('');

    useEffect(() => {
      if (message) {
        setAnnouncement('');
        setTimeout(() => setAnnouncement(message), 100);
      }
    }, [message]);

    return (
      <div
        role="status"
        aria-live={politeness}
        aria-atomic="true"
        className="sr-only"
      >
        {announcement}
      </div>
    );
  }
  ```

#### 포커스 스타일
- [ ] `src/index.css`에 추가
  ```css
  /* Focus visible styles */
  *:focus-visible {
    outline: 2px solid hsl(var(--primary));
    outline-offset: 2px;
  }

  /* Screen reader only */
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border-width: 0;
  }

  .sr-only:focus,
  .sr-only:active {
    position: static;
    width: auto;
    height: auto;
    overflow: visible;
    clip: auto;
    white-space: normal;
  }
  ```

### 예상 결과물
- 포커스 트랩
- Skip links
- ARIA 레이블
- 스크린 리더 지원

### Commit 메시지
```
feat(web-ui): enhance accessibility

- Implement focus trap hook
- Add skip links for navigation
- Improve ARIA labels and roles
- Create LiveRegion for screen reader announcements
- Add focus-visible styles
- Ensure keyboard navigation support
```

---

## 6. 반응형 디자인 (Commit 36)

### 요구사항
- 모바일 레이아웃
- 태블릿 레이아웃
- 브레이크포인트 설정
- 터치 제스처

### 작업 내용

#### 반응형 레이아웃
- [ ] `src/components/layout/ResponsiveLayout.tsx` 생성
  ```typescript
  import { useState } from 'react';
  import { Sheet, SheetContent, SheetTrigger } from '@/components/ui/sheet';
  import { Button } from '@/components/ui/button';
  import { Menu } from 'lucide-react';
  import { SessionList } from '@/components/session/SessionList';

  export function ResponsiveLayout({ children }: { children: React.ReactNode }) {
    const [sidebarOpen, setSidebarOpen] = useState(false);

    return (
      <div className="flex h-screen">
        {/* Desktop Sidebar */}
        <div className="hidden md:block w-64 flex-shrink-0 border-r">
          <SessionList />
        </div>

        {/* Mobile Sidebar */}
        <Sheet open={sidebarOpen} onOpenChange={setSidebarOpen}>
          <SheetContent side="left" className="w-64 p-0">
            <SessionList />
          </SheetContent>
        </Sheet>

        {/* Main Content */}
        <div className="flex-1 flex flex-col">
          {/* Mobile Header */}
          <div className="md:hidden flex items-center gap-2 p-4 border-b">
            <SheetTrigger asChild>
              <Button size="icon" variant="ghost">
                <Menu className="h-5 w-5" />
              </Button>
            </SheetTrigger>
            <h1 className="font-bold">Codex UI</h1>
          </div>

          {children}
        </div>
      </div>
    );
  }
  ```

#### shadcn Sheet 설치
- [ ] Sheet 컴포넌트 설치
  ```bash
  npx shadcn@latest add sheet
  ```

#### 반응형 유틸리티 훅
- [ ] `src/hooks/useMediaQuery.ts` 생성
  ```typescript
  import { useState, useEffect } from 'react';

  export function useMediaQuery(query: string): boolean {
    const [matches, setMatches] = useState(false);

    useEffect(() => {
      const media = window.matchMedia(query);
      setMatches(media.matches);

      const listener = (e: MediaQueryListEvent) => setMatches(e.matches);
      media.addEventListener('change', listener);

      return () => media.removeEventListener('change', listener);
    }, [query]);

    return matches;
  }

  export function useIsMobile() {
    return useMediaQuery('(max-width: 768px)');
  }

  export function useIsTablet() {
    return useMediaQuery('(min-width: 769px) and (max-width: 1024px)');
  }

  export function useIsDesktop() {
    return useMediaQuery('(min-width: 1025px)');
  }
  ```

#### 터치 제스처
- [ ] `src/hooks/useSwipe.ts` 생성
  ```typescript
  import { useRef, useEffect } from 'react';

  interface SwipeHandlers {
    onSwipeLeft?: () => void;
    onSwipeRight?: () => void;
    onSwipeUp?: () => void;
    onSwipeDown?: () => void;
  }

  export function useSwipe(handlers: SwipeHandlers, threshold: number = 50) {
    const touchStart = useRef<{ x: number; y: number } | null>(null);

    useEffect(() => {
      const handleTouchStart = (e: TouchEvent) => {
        touchStart.current = {
          x: e.touches[0].clientX,
          y: e.touches[0].clientY,
        };
      };

      const handleTouchEnd = (e: TouchEvent) => {
        if (!touchStart.current) return;

        const deltaX = e.changedTouches[0].clientX - touchStart.current.x;
        const deltaY = e.changedTouches[0].clientY - touchStart.current.y;

        if (Math.abs(deltaX) > Math.abs(deltaY)) {
          // Horizontal swipe
          if (Math.abs(deltaX) > threshold) {
            if (deltaX > 0) {
              handlers.onSwipeRight?.();
            } else {
              handlers.onSwipeLeft?.();
            }
          }
        } else {
          // Vertical swipe
          if (Math.abs(deltaY) > threshold) {
            if (deltaY > 0) {
              handlers.onSwipeDown?.();
            } else {
              handlers.onSwipeUp?.();
            }
          }
        }

        touchStart.current = null;
      };

      document.addEventListener('touchstart', handleTouchStart);
      document.addEventListener('touchend', handleTouchEnd);

      return () => {
        document.removeEventListener('touchstart', handleTouchStart);
        document.removeEventListener('touchend', handleTouchEnd);
      };
    }, [handlers, threshold]);
  }
  ```

#### Tailwind 브레이크포인트 확장
- [ ] `tailwind.config.js` 업데이트
  ```javascript
  module.exports = {
    theme: {
      extend: {
        screens: {
          'xs': '475px',
          '3xl': '1920px',
        },
      },
    },
  };
  ```

### 예상 결과물
- 모바일 친화적 레이아웃
- 반응형 사이드바
- 미디어 쿼리 훅
- 터치 제스처

### Commit 메시지
```
feat(web-ui): implement responsive design

- Create ResponsiveLayout with mobile sidebar
- Add useMediaQuery hook
- Implement swipe gestures
- Install sheet component
- Support mobile, tablet, and desktop layouts
- Add touch-friendly UI elements
```

---

## Day 6 완료 체크리스트

- [ ] 키보드 단축키 (전역, 도움말 다이얼로그)
- [ ] 명령 팔레트 (Cmd/Ctrl+K, fuzzy 검색)
- [ ] 성능 최적화 (가상 스크롤, 메모이제이션, 코드 스플리팅)
- [ ] 로딩 상태 (스켈레톤, 스피너, 진행률)
- [ ] 접근성 (키보드 네비게이션, ARIA, 포커스)
- [ ] 반응형 디자인 (모바일, 태블릿, 터치 제스처)
- [ ] 모든 커밋 메시지 명확하게 작성
- [ ] 기능 테스트 및 검증

---

## 다음 단계 (Day 7 예고)

1. 단위 테스트 (Vitest, React Testing Library)
2. 통합 테스트 (MSW, E2E)
3. 문서화 (README, 컴포넌트 문서)
4. 빌드 최적화
5. 배포 설정 (CI/CD)
6. 최종 점검 및 정리

---

## 참고 자료

- [react-window](https://github.com/bvaughn/react-window)
- [cmdk](https://cmdk.paco.me/)
- [Web Accessibility Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)
- [Mobile First Design](https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/Responsive/Mobile_first)

---

**Last Updated**: 2025-11-20
**Version**: 1.0
**Day**: 6 / 7
