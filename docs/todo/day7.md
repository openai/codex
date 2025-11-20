# Day 7 TODO - 테스트, 문서화, 배포 준비

## 목표
프로덕션 배포를 위한 테스트 작성, 문서화, 빌드 최적화, CI/CD 설정 및 최종 점검을 수행합니다.

---

## 1. 단위 테스트 (Commit 37)

### 요구사항
- Vitest 설정
- React Testing Library 설정
- 주요 컴포넌트 테스트
- 유틸리티 함수 테스트

### 작업 내용

#### Vitest 설정
- [ ] 테스트 라이브러리 설치
  ```bash
  pnpm add -D vitest @vitest/ui
  pnpm add -D @testing-library/react @testing-library/jest-dom @testing-library/user-event
  pnpm add -D jsdom
  ```

- [ ] `vitest.config.ts` 생성
  ```typescript
  import { defineConfig } from 'vitest/config';
  import react from '@vitejs/plugin-react';
  import path from 'path';

  export default defineConfig({
    plugins: [react()],
    test: {
      globals: true,
      environment: 'jsdom',
      setupFiles: './src/test/setup.ts',
      coverage: {
        provider: 'v8',
        reporter: ['text', 'json', 'html'],
        exclude: [
          'node_modules/',
          'src/test/',
          '**/*.d.ts',
          '**/*.config.*',
          '**/mockData',
        ],
      },
    },
    resolve: {
      alias: {
        '@': path.resolve(__dirname, './src'),
      },
    },
  });
  ```

- [ ] `src/test/setup.ts` 생성
  ```typescript
  import { expect, afterEach } from 'vitest';
  import { cleanup } from '@testing-library/react';
  import * as matchers from '@testing-library/jest-dom/matchers';

  expect.extend(matchers);

  afterEach(() => {
    cleanup();
  });
  ```

#### 컴포넌트 테스트
- [ ] `src/components/chat/__tests__/MessageInput.test.tsx` 생성
  ```typescript
  import { describe, it, expect, vi } from 'vitest';
  import { render, screen, fireEvent } from '@testing-library/react';
  import userEvent from '@testing-library/user-event';
  import { MessageInput } from '../MessageInput';

  describe('MessageInput', () => {
    it('renders input field', () => {
      render(<MessageInput />);
      expect(screen.getByPlaceholderText(/type a message/i)).toBeInTheDocument();
    });

    it('updates input value on typing', async () => {
      const user = userEvent.setup();
      render(<MessageInput />);

      const input = screen.getByPlaceholderText(/type a message/i);
      await user.type(input, 'Hello');

      expect(input).toHaveValue('Hello');
    });

    it('sends message on Enter key', async () => {
      const user = userEvent.setup();
      const onSend = vi.fn();
      render(<MessageInput onSend={onSend} />);

      const input = screen.getByPlaceholderText(/type a message/i);
      await user.type(input, 'Hello{Enter}');

      expect(onSend).toHaveBeenCalledWith('Hello');
      expect(input).toHaveValue('');
    });

    it('adds new line on Shift+Enter', async () => {
      const user = userEvent.setup();
      render(<MessageInput />);

      const input = screen.getByPlaceholderText(/type a message/i);
      await user.type(input, 'Line 1{Shift>}{Enter}{/Shift}Line 2');

      expect(input).toHaveValue('Line 1\nLine 2');
    });
  });
  ```

- [ ] `src/components/session/__tests__/SessionItem.test.tsx` 생성
  ```typescript
  import { describe, it, expect, vi } from 'vitest';
  import { render, screen, fireEvent } from '@testing-library/react';
  import { SessionItem } from '../SessionItem';
  import { SessionSummary } from '@/types/session';

  const mockSession: SessionSummary = {
    id: '1',
    name: 'Test Session',
    createdAt: Date.now(),
    updatedAt: Date.now(),
    messageCount: 5,
  };

  describe('SessionItem', () => {
    it('renders session name', () => {
      render(
        <SessionItem
          session={mockSession}
          isActive={false}
          onSelect={() => {}}
        />
      );

      expect(screen.getByText('Test Session')).toBeInTheDocument();
    });

    it('shows message count', () => {
      render(
        <SessionItem
          session={mockSession}
          isActive={false}
          onSelect={() => {}}
        />
      );

      expect(screen.getByText(/5 messages/i)).toBeInTheDocument();
    });

    it('calls onSelect when clicked', () => {
      const onSelect = vi.fn();
      render(
        <SessionItem
          session={mockSession}
          isActive={false}
          onSelect={onSelect}
        />
      );

      fireEvent.click(screen.getByText('Test Session'));
      expect(onSelect).toHaveBeenCalled();
    });

    it('applies active style when isActive is true', () => {
      const { container } = render(
        <SessionItem
          session={mockSession}
          isActive={true}
          onSelect={() => {}}
        />
      );

      expect(container.querySelector('.bg-accent')).toBeInTheDocument();
    });
  });
  ```

#### 유틸리티 함수 테스트
- [ ] `src/lib/__tests__/format-utils.test.ts` 생성
  ```typescript
  import { describe, it, expect } from 'vitest';
  import { formatBytes, formatDate, formatDuration } from '../format-utils';

  describe('formatBytes', () => {
    it('formats bytes correctly', () => {
      expect(formatBytes(0)).toBe('0 Bytes');
      expect(formatBytes(1024)).toBe('1 KB');
      expect(formatBytes(1048576)).toBe('1 MB');
      expect(formatBytes(1073741824)).toBe('1 GB');
    });
  });

  describe('formatDuration', () => {
    it('formats milliseconds', () => {
      expect(formatDuration(500)).toBe('500ms');
    });

    it('formats seconds', () => {
      expect(formatDuration(1500)).toBe('1.5s');
    });

    it('formats minutes', () => {
      expect(formatDuration(65000)).toBe('1m 5s');
    });

    it('formats hours', () => {
      expect(formatDuration(3665000)).toBe('1h 1m');
    });
  });
  ```

#### 스토어 테스트
- [ ] `src/store/__tests__/chat-store.test.ts` 생성
  ```typescript
  import { describe, it, expect, beforeEach } from 'vitest';
  import { useChatStore } from '../chat-store';
  import { MessageRole, MessageStatus } from '@/types/message';

  describe('ChatStore', () => {
    beforeEach(() => {
      useChatStore.setState({ messages: [] });
    });

    it('adds message', () => {
      const { addMessage } = useChatStore.getState();

      const message = addMessage({
        role: MessageRole.USER,
        content: [{ type: 'text', content: 'Hello' }],
        status: MessageStatus.COMPLETED,
      });

      expect(message.id).toBeDefined();
      expect(message.role).toBe(MessageRole.USER);
      expect(useChatStore.getState().messages).toHaveLength(1);
    });

    it('updates message', () => {
      const { addMessage, updateMessage } = useChatStore.getState();

      const message = addMessage({
        role: MessageRole.USER,
        content: [{ type: 'text', content: 'Hello' }],
        status: MessageStatus.PENDING,
      });

      updateMessage(message.id, { status: MessageStatus.COMPLETED });

      const updated = useChatStore.getState().messages[0];
      expect(updated.status).toBe(MessageStatus.COMPLETED);
    });

    it('deletes message', () => {
      const { addMessage, deleteMessage } = useChatStore.getState();

      const message = addMessage({
        role: MessageRole.USER,
        content: [{ type: 'text', content: 'Hello' }],
        status: MessageStatus.COMPLETED,
      });

      deleteMessage(message.id);

      expect(useChatStore.getState().messages).toHaveLength(0);
    });

    it('clears all messages', () => {
      const { addMessage, clearMessages } = useChatStore.getState();

      addMessage({
        role: MessageRole.USER,
        content: [{ type: 'text', content: 'Hello' }],
        status: MessageStatus.COMPLETED,
      });

      clearMessages();

      expect(useChatStore.getState().messages).toHaveLength(0);
    });
  });
  ```

#### package.json 스크립트 추가
- [ ] `package.json`에 테스트 스크립트 추가
  ```json
  {
    "scripts": {
      "test": "vitest",
      "test:ui": "vitest --ui",
      "test:coverage": "vitest --coverage"
    }
  }
  ```

### 예상 결과물
- Vitest 설정 완료
- 주요 컴포넌트 테스트
- 유틸리티 함수 테스트
- 스토어 테스트

### Commit 메시지
```
test(web-ui): add unit tests for components

- Setup Vitest and React Testing Library
- Add tests for MessageInput component
- Test SessionItem component
- Add utility function tests
- Test chat store functionality
- Configure test coverage reporting
```

---

## 2. 통합 테스트 (Commit 38)

### 요구사항
- API 통신 테스트 (MSW)
- WebSocket 테스트
- E2E 테스트 (Playwright)
- 주요 사용자 플로우 테스트

### 작업 내용

#### MSW 설정
- [ ] MSW 설치
  ```bash
  pnpm add -D msw
  ```

- [ ] `src/test/mocks/handlers.ts` 생성
  ```typescript
  import { http, HttpResponse } from 'msw';

  export const handlers = [
    http.get('/api/files/tree', () => {
      return HttpResponse.json([
        {
          id: '1',
          name: 'src',
          path: '/src',
          type: 'directory',
          children: [],
        },
      ]);
    }),

    http.get('/api/files/content', ({ request }) => {
      const url = new URL(request.url);
      const path = url.searchParams.get('path');

      return HttpResponse.json({
        content: `// Content of ${path}`,
      });
    }),

    http.post('/api/files/upload', async ({ request }) => {
      const formData = await request.formData();
      const file = formData.get('file');

      return HttpResponse.json({
        success: true,
        filename: file?.name,
      });
    }),
  ];
  ```

- [ ] `src/test/mocks/server.ts` 생성
  ```typescript
  import { setupServer } from 'msw/node';
  import { handlers } from './handlers';

  export const server = setupServer(...handlers);
  ```

- [ ] `src/test/setup.ts`에 MSW 통합
  ```typescript
  import { beforeAll, afterEach, afterAll } from 'vitest';
  import { server } from './mocks/server';

  beforeAll(() => server.listen());
  afterEach(() => server.resetHandlers());
  afterAll(() => server.close());
  ```

#### API 테스트
- [ ] `src/lib/__tests__/api-client.test.ts` 생성
  ```typescript
  import { describe, it, expect } from 'vitest';
  import { apiClient } from '../api-client';

  describe('API Client', () => {
    it('fetches file tree', async () => {
      const response = await apiClient.get('/files/tree');
      expect(response.data).toHaveLength(1);
      expect(response.data[0].name).toBe('src');
    });

    it('fetches file content', async () => {
      const response = await apiClient.get('/files/content', {
        params: { path: '/src/index.ts' },
      });
      expect(response.data.content).toContain('index.ts');
    });

    it('uploads file', async () => {
      const formData = new FormData();
      formData.append('file', new File(['content'], 'test.txt'));

      const response = await apiClient.post('/files/upload', formData);
      expect(response.data.success).toBe(true);
    });
  });
  ```

#### Playwright 설정
- [ ] Playwright 설치
  ```bash
  pnpm add -D @playwright/test
  npx playwright install
  ```

- [ ] `playwright.config.ts` 생성
  ```typescript
  import { defineConfig, devices } from '@playwright/test';

  export default defineConfig({
    testDir: './e2e',
    fullyParallel: true,
    forbidOnly: !!process.env.CI,
    retries: process.env.CI ? 2 : 0,
    workers: process.env.CI ? 1 : undefined,
    reporter: 'html',
    use: {
      baseURL: 'http://localhost:3000',
      trace: 'on-first-retry',
    },

    projects: [
      {
        name: 'chromium',
        use: { ...devices['Desktop Chrome'] },
      },
      {
        name: 'firefox',
        use: { ...devices['Desktop Firefox'] },
      },
      {
        name: 'webkit',
        use: { ...devices['Desktop Safari'] },
      },
    ],

    webServer: {
      command: 'pnpm dev',
      url: 'http://localhost:3000',
      reuseExistingServer: !process.env.CI,
    },
  });
  ```

#### E2E 테스트
- [ ] `e2e/chat.spec.ts` 생성
  ```typescript
  import { test, expect } from '@playwright/test';

  test.describe('Chat Flow', () => {
    test('creates new session and sends message', async ({ page }) => {
      await page.goto('/');

      // Create new session
      await page.click('button:has-text("New Session")');
      await expect(page.locator('text=New Session')).toBeVisible();

      // Navigate to chat
      await page.goto('/chat');

      // Send message
      const input = page.locator('textarea[aria-label="Message input"]');
      await input.fill('Hello, Codex!');
      await input.press('Enter');

      // Verify message appears
      await expect(page.locator('text=Hello, Codex!')).toBeVisible();
    });

    test('displays streaming response', async ({ page }) => {
      await page.goto('/chat');

      // Send message
      const input = page.locator('textarea[aria-label="Message input"]');
      await input.fill('Write a function');
      await input.press('Enter');

      // Wait for typing indicator
      await expect(page.locator('text=Codex is thinking')).toBeVisible();

      // Wait for response
      await expect(page.locator('[data-testid="assistant-message"]')).toBeVisible({
        timeout: 10000,
      });
    });
  });
  ```

- [ ] `e2e/settings.spec.ts` 생성
  ```typescript
  import { test, expect } from '@playwright/test';

  test.describe('Settings', () => {
    test('updates theme setting', async ({ page }) => {
      await page.goto('/settings');

      // Switch to dark mode
      await page.click('input[value="dark"]');

      // Verify dark mode applied
      const html = page.locator('html');
      await expect(html).toHaveClass(/dark/);
    });

    test('saves API key', async ({ page }) => {
      await page.goto('/settings');

      // Select API key method
      await page.click('input[value="api_key"]');

      // Enter API key
      await page.fill('input[type="password"]', 'sk-test-key');
      await page.click('button:has-text("Save")');

      // Verify success message
      await expect(page.locator('text=API key saved')).toBeVisible();
    });
  });
  ```

#### package.json 스크립트 추가
- [ ] E2E 테스트 스크립트 추가
  ```json
  {
    "scripts": {
      "test:e2e": "playwright test",
      "test:e2e:ui": "playwright test --ui",
      "test:e2e:report": "playwright show-report"
    }
  }
  ```

### 예상 결과물
- MSW로 API 모킹
- API 통신 테스트
- Playwright E2E 테스트
- 주요 플로우 테스트

### Commit 메시지
```
test(web-ui): add integration and e2e tests

- Setup MSW for API mocking
- Add API client tests
- Configure Playwright for e2e testing
- Test chat flow end-to-end
- Test settings functionality
- Add test scripts to package.json
```

---

## 3. 문서화 (Commit 39)

### 요구사항
- README 작성
- 컴포넌트 문서
- API 문서
- 개발 가이드

### 작업 내용

#### README.md
- [ ] `codex-web-ui/README.md` 생성
  ```markdown
  # Codex Web UI

  A modern web interface for OpenAI Codex CLI, built with React, TypeScript, and Tailwind CSS.

  ## Features

  - 🚀 Real-time chat interface with streaming responses
  - 📁 File explorer and code viewer with syntax highlighting
  - 🔧 Tool call visualization and approval flow
  - 💾 Session management with IndexedDB persistence
  - 🎨 Customizable themes and appearance
  - ⌨️ Keyboard shortcuts and command palette
  - 📱 Responsive design for mobile and desktop
  - ♿ Accessibility-first approach

  ## Quick Start

  ### Prerequisites

  - Node.js >= 22
  - pnpm >= 9.0.0

  ### Installation

  \`\`\`bash
  # Install dependencies
  pnpm install

  # Start development server
  pnpm dev

  # Build for production
  pnpm build

  # Preview production build
  pnpm preview
  \`\`\`

  ## Project Structure

  \`\`\`
  codex-web-ui/
  ├── public/          # Static assets
  ├── src/
  │   ├── components/  # React components
  │   ├── features/    # Feature modules
  │   ├── hooks/       # Custom hooks
  │   ├── lib/         # Utilities and helpers
  │   ├── pages/       # Page components
  │   ├── store/       # Zustand stores
  │   ├── types/       # TypeScript types
  │   ├── App.tsx      # Main app component
  │   └── main.tsx     # Entry point
  ├── e2e/             # E2E tests
  └── docs/            # Documentation
  \`\`\`

  ## Development

  ### Available Scripts

  - `pnpm dev` - Start development server
  - `pnpm build` - Build for production
  - `pnpm preview` - Preview production build
  - `pnpm test` - Run unit tests
  - `pnpm test:e2e` - Run e2e tests
  - `pnpm lint` - Lint code
  - `pnpm format` - Format code

  ### Tech Stack

  - **Framework**: React 18
  - **Language**: TypeScript
  - **Build Tool**: Vite
  - **Styling**: Tailwind CSS
  - **UI Components**: shadcn/ui
  - **State Management**: Zustand
  - **Data Fetching**: TanStack Query
  - **Testing**: Vitest + Playwright
  - **Code Editor**: Monaco Editor

  ## Configuration

  Settings are stored in localStorage and IndexedDB. You can configure:

  - Authentication (ChatGPT or API Key)
  - Model settings (provider, parameters)
  - Appearance (theme, colors, fonts)
  - Advanced options (sandbox, MCP servers)

  ## Keyboard Shortcuts

  - `Cmd/Ctrl + K` - Open command palette
  - `Cmd/Ctrl + N` - New session
  - `Cmd/Ctrl + F` - Search
  - `Cmd/Ctrl + /` - Show shortcuts help
  - `Cmd/Ctrl + ,` - Open settings

  ## Contributing

  See [CONTRIBUTING.md](../docs/contributing.md) for development guidelines.

  ## License

  Apache-2.0
  ```

#### 컴포넌트 문서
- [ ] `docs/components.md` 생성
  ```markdown
  # Component Documentation

  ## Core Components

  ### MessageList
  Displays a list of chat messages with virtual scrolling for performance.

  **Props:**
  - None (uses chat store)

  **Features:**
  - Auto-scroll to bottom
  - Virtual scrolling for large message lists
  - Typing indicator

  ### MessageInput
  Input field for composing messages.

  **Props:**
  - `onSend?: (message: string) => void`

  **Keyboard Shortcuts:**
  - `Enter` - Send message
  - `Shift + Enter` - New line

  ### FileExplorer
  Tree view for browsing files and folders.

  **Props:**
  - None (uses file store)

  **Features:**
  - Lazy loading of directory contents
  - File type icons
  - Git status indicators

  ### SessionList
  Sidebar displaying all sessions.

  **Props:**
  - None (uses session store)

  **Features:**
  - Search sessions
  - Pin/unpin sessions
  - Rename and delete

  ## Utility Components

  ### ErrorBoundary
  Catches and displays React errors.

  ### LoadingSpinner
  Displays loading state with optional message.

  ### LazyImage
  Lazy loads images with placeholder.

  ## Hooks

  ### useAutoSave
  Automatically saves session changes with debouncing.

  ### useTheme
  Applies theme settings to the document.

  ### useMediaQuery
  Detects viewport size changes.
  ```

#### API 문서
- [ ] `docs/api.md` 생성
  ```markdown
  # API Documentation

  ## Endpoints

  ### Files

  #### GET /api/files/tree
  Returns file tree structure.

  **Query Parameters:**
  - `path?: string` - Root path (default: current directory)

  **Response:**
  \`\`\`json
  [
    {
      "id": "1",
      "name": "src",
      "path": "/src",
      "type": "directory",
      "children": []
    }
  ]
  \`\`\`

  #### GET /api/files/content
  Returns file content.

  **Query Parameters:**
  - `path: string` - File path

  **Response:**
  \`\`\`json
  {
    "content": "file contents..."
  }
  \`\`\`

  #### POST /api/files/upload
  Uploads a file.

  **Body:** FormData with `file` field

  **Response:**
  \`\`\`json
  {
    "success": true,
    "filename": "uploaded-file.txt"
  }
  \`\`\`

  ### Chat

  #### WebSocket /ws
  Real-time chat connection.

  **Client Messages:**
  \`\`\`json
  {
    "type": "user_message",
    "content": "Hello",
    "messageId": "msg-123"
  }
  \`\`\`

  **Server Messages:**
  \`\`\`json
  {
    "type": "response_chunk",
    "data": {
      "content": "Hello! How can I help?"
    }
  }
  \`\`\`
  ```

#### 개발 가이드
- [ ] `docs/development.md` 생성
  ```markdown
  # Development Guide

  ## Getting Started

  1. Clone the repository
  2. Install dependencies: `pnpm install`
  3. Start dev server: `pnpm dev`

  ## Code Style

  - Use TypeScript for all new code
  - Follow ESLint rules
  - Use Prettier for formatting
  - Write tests for new features

  ## Component Guidelines

  - Use functional components with hooks
  - Memoize expensive computations
  - Add ARIA labels for accessibility
  - Support keyboard navigation

  ## State Management

  - Use Zustand for global state
  - Use React Query for server state
  - Keep component state local when possible

  ## Testing

  - Write unit tests for utilities
  - Test component interactions
  - Add e2e tests for critical flows
  - Aim for >70% code coverage

  ## Performance

  - Use React.memo for expensive components
  - Implement virtual scrolling for lists
  - Lazy load routes and images
  - Monitor bundle size

  ## Accessibility

  - Use semantic HTML
  - Add ARIA labels
  - Support keyboard navigation
  - Test with screen readers
  ```

### 예상 결과물
- 완전한 README
- 컴포넌트 문서
- API 문서
- 개발 가이드

### Commit 메시지
```
docs(web-ui): add comprehensive documentation

- Create detailed README with quick start
- Document all major components
- Add API endpoint documentation
- Write development guide
- Include keyboard shortcuts reference
```

---

## 4. 빌드 최적화 (Commit 40)

### 요구사항
- 프로덕션 빌드 설정
- 번들 크기 최적화
- 이미지 최적화
- 캐싱 전략

### 작업 내용

#### Vite 빌드 설정
- [ ] `vite.config.ts` 최적화
  ```typescript
  import { defineConfig } from 'vite';
  import react from '@vitejs/plugin-react';
  import path from 'path';
  import { visualizer } from 'rollup-plugin-visualizer';

  export default defineConfig({
    plugins: [
      react(),
      visualizer({
        filename: './dist/stats.html',
        open: false,
        gzipSize: true,
      }),
    ],
    resolve: {
      alias: {
        '@': path.resolve(__dirname, './src'),
      },
    },
    build: {
      outDir: 'dist',
      sourcemap: false,
      minify: 'terser',
      terserOptions: {
        compress: {
          drop_console: true,
          drop_debugger: true,
        },
      },
      rollupOptions: {
        output: {
          manualChunks: {
            'react-vendor': ['react', 'react-dom', 'react-router-dom'],
            'ui-vendor': ['@radix-ui/react-dialog', '@radix-ui/react-dropdown-menu'],
            'editor': ['@monaco-editor/react', 'monaco-editor'],
            'utils': ['lodash-es', 'zustand', '@tanstack/react-query'],
          },
        },
      },
      chunkSizeWarningLimit: 1000,
    },
    optimizeDeps: {
      include: ['react', 'react-dom', 'react-router-dom'],
    },
  });
  ```

#### 번들 분석 도구 설치
- [ ] rollup-plugin-visualizer 설치
  ```bash
  pnpm add -D rollup-plugin-visualizer
  ```

#### 이미지 최적화
- [ ] vite-plugin-image-optimizer 설치
  ```bash
  pnpm add -D vite-plugin-image-optimizer
  ```

- [ ] vite.config.ts에 추가
  ```typescript
  import { ViteImageOptimizer } from 'vite-plugin-image-optimizer';

  export default defineConfig({
    plugins: [
      ViteImageOptimizer({
        png: {
          quality: 80,
        },
        jpeg: {
          quality: 80,
        },
        jpg: {
          quality: 80,
        },
      }),
    ],
  });
  ```

#### PWA 설정 (선택사항)
- [ ] vite-plugin-pwa 설치
  ```bash
  pnpm add -D vite-plugin-pwa
  ```

- [ ] vite.config.ts에 PWA 추가
  ```typescript
  import { VitePWA } from 'vite-plugin-pwa';

  export default defineConfig({
    plugins: [
      VitePWA({
        registerType: 'autoUpdate',
        manifest: {
          name: 'Codex Web UI',
          short_name: 'Codex',
          description: 'Web interface for OpenAI Codex',
          theme_color: '#0ea5e9',
          icons: [
            {
              src: '/icon-192.png',
              sizes: '192x192',
              type: 'image/png',
            },
            {
              src: '/icon-512.png',
              sizes: '512x512',
              type: 'image/png',
            },
          ],
        },
      }),
    ],
  });
  ```

#### 환경 변수
- [ ] `.env.production` 생성
  ```env
  VITE_API_URL=https://api.codex.example.com
  VITE_WS_URL=wss://ws.codex.example.com
  VITE_APP_VERSION=1.0.0
  ```

### 예상 결과물
- 최적화된 프로덕션 빌드
- 번들 크기 분석
- 이미지 최적화
- PWA 지원

### Commit 메시지
```
build(web-ui): optimize production build

- Configure Vite for production
- Add bundle visualization
- Implement code splitting strategy
- Optimize images with compression
- Setup PWA support (optional)
- Add production environment variables
```

---

## 5. 배포 설정 (Commit 41)

### 요구사항
- Docker 설정
- CI/CD 파이프라인
- 정적 파일 서빙
- 환경 변수 관리

### 작업 내용

#### Dockerfile
- [ ] `codex-web-ui/Dockerfile` 생성
  ```dockerfile
  # Build stage
  FROM node:22-alpine AS builder

  # Install pnpm
  RUN corepack enable && corepack prepare pnpm@latest --activate

  WORKDIR /app

  # Copy dependency files
  COPY package.json pnpm-lock.yaml ./

  # Install dependencies
  RUN pnpm install --frozen-lockfile

  # Copy source files
  COPY . .

  # Build application
  RUN pnpm build

  # Production stage
  FROM nginx:alpine

  # Copy built files
  COPY --from=builder /app/dist /usr/share/nginx/html

  # Copy nginx configuration
  COPY nginx.conf /etc/nginx/nginx.conf

  EXPOSE 80

  CMD ["nginx", "-g", "daemon off;"]
  ```

#### Nginx 설정
- [ ] `codex-web-ui/nginx.conf` 생성
  ```nginx
  events {
    worker_connections 1024;
  }

  http {
    include /etc/nginx/mime.types;
    default_type application/octet-stream;

    gzip on;
    gzip_vary on;
    gzip_types text/plain text/css application/json application/javascript text/xml application/xml application/xml+rss text/javascript;

    server {
      listen 80;
      server_name localhost;
      root /usr/share/nginx/html;
      index index.html;

      # Security headers
      add_header X-Frame-Options "SAMEORIGIN" always;
      add_header X-Content-Type-Options "nosniff" always;
      add_header X-XSS-Protection "1; mode=block" always;

      # Cache static assets
      location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|woff|woff2|ttf|eot)$ {
        expires 1y;
        add_header Cache-Control "public, immutable";
      }

      # API proxy
      location /api {
        proxy_pass http://app-server:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_cache_bypass $http_upgrade;
      }

      # WebSocket proxy
      location /ws {
        proxy_pass http://app-server:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "Upgrade";
        proxy_set_header Host $host;
      }

      # SPA fallback
      location / {
        try_files $uri $uri/ /index.html;
      }
    }
  }
  ```

#### Docker Compose
- [ ] `docker-compose.yml` (루트에) 업데이트
  ```yaml
  version: '3.8'

  services:
    web-ui:
      build:
        context: ./codex-web-ui
        dockerfile: Dockerfile
      ports:
        - "3000:80"
      environment:
        - VITE_API_URL=http://localhost:8080
        - VITE_WS_URL=ws://localhost:8080
      depends_on:
        - app-server

    app-server:
      build:
        context: ./codex-rs
        dockerfile: Dockerfile
      ports:
        - "8080:8080"
      volumes:
        - ./workspace:/workspace
  ```

#### GitHub Actions
- [ ] `.github/workflows/web-ui-ci.yml` 생성
  ```yaml
  name: Web UI CI/CD

  on:
    push:
      branches: [main, develop]
      paths:
        - 'codex-web-ui/**'
    pull_request:
      branches: [main, develop]
      paths:
        - 'codex-web-ui/**'

  jobs:
    test:
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v4

        - name: Setup Node.js
          uses: actions/setup-node@v4
          with:
            node-version: '22'

        - name: Setup pnpm
          uses: pnpm/action-setup@v2
          with:
            version: 9

        - name: Install dependencies
          run: |
            cd codex-web-ui
            pnpm install --frozen-lockfile

        - name: Run linter
          run: |
            cd codex-web-ui
            pnpm lint

        - name: Run tests
          run: |
            cd codex-web-ui
            pnpm test:coverage

        - name: Upload coverage
          uses: codecov/codecov-action@v3
          with:
            files: ./codex-web-ui/coverage/coverage-final.json

    build:
      runs-on: ubuntu-latest
      needs: test
      steps:
        - uses: actions/checkout@v4

        - name: Setup Node.js
          uses: actions/setup-node@v4
          with:
            node-version: '22'

        - name: Setup pnpm
          uses: pnpm/action-setup@v2
          with:
            version: 9

        - name: Install dependencies
          run: |
            cd codex-web-ui
            pnpm install --frozen-lockfile

        - name: Build
          run: |
            cd codex-web-ui
            pnpm build

        - name: Upload build artifacts
          uses: actions/upload-artifact@v3
          with:
            name: dist
            path: codex-web-ui/dist

    deploy:
      runs-on: ubuntu-latest
      needs: build
      if: github.ref == 'refs/heads/main'
      steps:
        - uses: actions/checkout@v4

        - name: Download build artifacts
          uses: actions/download-artifact@v3
          with:
            name: dist
            path: codex-web-ui/dist

        - name: Deploy to production
          run: |
            # Add deployment steps here
            echo "Deploying to production..."
  ```

#### 배포 스크립트
- [ ] `codex-web-ui/scripts/deploy.sh` 생성
  ```bash
  #!/bin/bash

  set -e

  echo "Building production image..."
  docker build -t codex-web-ui:latest .

  echo "Pushing to registry..."
  # docker push your-registry/codex-web-ui:latest

  echo "Deploying to server..."
  # Add your deployment commands here

  echo "Deployment complete!"
  ```

### 예상 결과물
- Docker 설정
- Nginx 설정
- CI/CD 파이프라인
- 배포 스크립트

### Commit 메시지
```
ci(web-ui): setup deployment pipeline

- Create Dockerfile for production
- Add nginx configuration
- Setup Docker Compose
- Create GitHub Actions workflow
- Add deployment script
- Configure environment variables
```

---

## 6. 최종 점검 및 정리 (Commit 42)

### 요구사항
- 코드 린팅 및 포맷팅
- 사용하지 않는 의존성 제거
- TODO 주석 정리
- CHANGELOG 업데이트

### 작업 내용

#### ESLint 설정
- [ ] `.eslintrc.json` 업데이트
  ```json
  {
    "extends": [
      "eslint:recommended",
      "plugin:@typescript-eslint/recommended",
      "plugin:react-hooks/recommended",
      "plugin:jsx-a11y/recommended"
    ],
    "rules": {
      "no-console": "warn",
      "no-unused-vars": "off",
      "@typescript-eslint/no-unused-vars": ["warn", { "argsIgnorePattern": "^_" }],
      "@typescript-eslint/no-explicit-any": "warn"
    }
  }
  ```

#### 의존성 정리
- [ ] 사용하지 않는 패키지 확인
  ```bash
  pnpm dlx depcheck
  ```

- [ ] package.json 정리
  ```bash
  pnpm prune
  ```

#### TODO 주석 처리
- [ ] TODO 주석 찾기
  ```bash
  grep -r "TODO\|FIXME\|HACK" src/
  ```

- [ ] 모든 TODO를 issue로 변환하거나 완료

#### CHANGELOG
- [ ] `CHANGELOG.md` 생성
  ```markdown
  # Changelog

  ## [1.0.0] - 2025-11-20

  ### Added
  - Initial release of Codex Web UI
  - Real-time chat interface with streaming
  - File explorer and code viewer
  - Tool call visualization
  - Session management
  - Settings and customization
  - Keyboard shortcuts
  - Command palette
  - Search functionality
  - Export/import sessions
  - Responsive design
  - Accessibility improvements

  ### Features
  - React 18 + TypeScript
  - Tailwind CSS + shadcn/ui
  - Zustand state management
  - IndexedDB persistence
  - Monaco Editor integration
  - PWA support

  ### Developer Experience
  - Vitest for unit testing
  - Playwright for e2e testing
  - ESLint + Prettier
  - Docker support
  - CI/CD with GitHub Actions
  ```

#### 최종 빌드 테스트
- [ ] 로컬 프로덕션 빌드 테스트
  ```bash
  pnpm build
  pnpm preview
  ```

- [ ] Docker 빌드 테스트
  ```bash
  docker build -t codex-web-ui:test .
  docker run -p 3000:80 codex-web-ui:test
  ```

#### 코드 정리
- [ ] 전체 린팅
  ```bash
  pnpm lint --fix
  ```

- [ ] 전체 포맷팅
  ```bash
  pnpm format
  ```

#### 성능 체크
- [ ] Lighthouse 실행
- [ ] 번들 크기 확인
- [ ] 로딩 시간 측정

### 예상 결과물
- 깔끔한 코드베이스
- 업데이트된 의존성
- CHANGELOG
- 프로덕션 준비 완료

### Commit 메시지
```
chore(web-ui): final cleanup and polish

- Update ESLint configuration
- Remove unused dependencies
- Resolve all TODO comments
- Add CHANGELOG
- Fix linting and formatting issues
- Verify production build
- Run final performance checks
```

---

## Day 7 완료 체크리스트

- [ ] 단위 테스트 (Vitest, 컴포넌트, 유틸리티, 스토어)
- [ ] 통합 테스트 (MSW, Playwright, E2E)
- [ ] 문서화 (README, 컴포넌트, API, 개발 가이드)
- [ ] 빌드 최적화 (번들링, 이미지, PWA)
- [ ] 배포 설정 (Docker, CI/CD, Nginx)
- [ ] 최종 점검 (린팅, 정리, CHANGELOG)
- [ ] 모든 커밋 메시지 명확하게 작성
- [ ] 프로덕션 배포 준비 완료 ✅

---

## 프로젝트 완료!

축하합니다! 7일간의 개발을 통해 완전한 Codex Web UI를 구축했습니다.

### 달성한 것들

#### Week 1 완성도
- ✅ 42개 커밋 계획 완료
- ✅ 핵심 기능 모두 구현
- ✅ 테스트 커버리지 확보
- ✅ 문서화 완료
- ✅ 배포 준비 완료

#### 기술적 성과
- React + TypeScript 기반 모던 웹앱
- 실시간 스트리밍 채팅
- 완전한 파일 관리 시스템
- 영구 세션 저장
- 접근성과 성능 최적화
- 프로덕션 준비 완료

### 다음 단계

1. **프로덕션 배포**
   - 서버 환경 설정
   - 도메인 연결
   - SSL 인증서 설정

2. **사용자 피드백**
   - 베타 테스터 모집
   - 피드백 수집
   - 개선사항 정리

3. **지속적 개선**
   - 버그 수정
   - 기능 추가
   - 성능 최적화

---

## 참고 자료

- [Vitest Documentation](https://vitest.dev/)
- [Playwright Documentation](https://playwright.dev/)
- [Docker Documentation](https://docs.docker.com/)
- [GitHub Actions](https://docs.github.com/en/actions)
- [Web Performance Best Practices](https://web.dev/performance/)

---

**Last Updated**: 2025-11-20
**Version**: 1.0
**Day**: 7 / 7
**Status**: ✅ COMPLETE
