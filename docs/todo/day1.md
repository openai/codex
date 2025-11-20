# Day 1 TODO - Codex UI 프로젝트 초기 설정

## 목표
Codex CLI에 웹 기반 UI를 추가하기 위한 초기 프로젝트 환경을 설정하고 기본 구조를 구축합니다.

---

## 1. 프로젝트 분석 및 이해 (Commit 1)

### 요구사항
- 현재 Codex CLI 프로젝트의 구조와 아키텍처를 완전히 파악
- 기존 app-server와 TUI의 동작 방식 이해
- 웹 UI 통합 지점 파악

### 작업 내용
- [ ] `codex-rs/app-server` 폴더의 전체 구조 분석
  - `src/` 내 모든 주요 파일 리뷰
  - 라우팅 및 API 엔드포인트 파악
  - WebSocket 또는 SSE(Server-Sent Events) 사용 여부 확인

- [ ] `codex-rs/tui` 폴더 분석
  - TUI가 app-server와 통신하는 방식 파악
  - 프로토콜 및 메시지 형식 이해
  - UI 상태 관리 방식 확인

- [ ] `codex-rs/app-server-protocol` 분석
  - 클라이언트-서버 통신 프로토콜 문서화
  - 주요 메시지 타입 및 데이터 구조 파악

- [ ] `codex-rs/core` 분석
  - 핵심 비즈니스 로직 파악
  - 에이전트 실행 흐름 이해

### 예상 결과물
- 프로젝트 구조 문서 (`docs/todo/architecture-analysis.md`)
- API 엔드포인트 목록
- 통신 프로토콜 정리 문서

### Commit 메시지
```
docs: analyze project architecture for UI integration

- Document app-server structure and endpoints
- Analyze TUI communication patterns
- Identify integration points for web UI
```

---

## 2. 프론트엔드 기술 스택 결정 (Commit 2)

### 요구사항
- 현대적이고 유지보수가 용이한 프론트엔드 프레임워크 선택
- Rust 백엔드와의 통합이 용이할 것
- 빠른 개발과 좋은 개발자 경험 제공

### 작업 내용
- [ ] 기술 스택 비교 분석
  - **React + Vite**: 가장 많이 사용되는 프레임워크, 풍부한 생태계
  - **Vue 3 + Vite**: 간단한 학습 곡선, 우수한 문서
  - **Svelte + SvelteKit**: 작은 번들 사이즈, 우수한 성능
  - **Solid.js**: React와 유사하나 더 빠른 성능

- [ ] 상태 관리 라이브러리 선택
  - Zustand (추천: 간단하고 가벼움)
  - Redux Toolkit
  - Jotai
  - TanStack Query (서버 상태 관리)

- [ ] UI 컴포넌트 라이브러리 선택
  - **shadcn/ui** (추천: Tailwind 기반, 커스터마이징 용이)
  - Chakra UI
  - Mantine
  - Headless UI + Tailwind

- [ ] 빌드 도구 및 개발 환경
  - Vite (권장)
  - TypeScript 설정
  - ESLint + Prettier

### 결정 사항 (권장)
```
Frontend: React 18 + TypeScript
Build Tool: Vite
State Management: Zustand + TanStack Query
UI Components: shadcn/ui + Tailwind CSS
Package Manager: pnpm (monorepo와 일관성)
```

### 예상 결과물
- 기술 스택 결정 문서 (`docs/todo/tech-stack-decision.md`)

### Commit 메시지
```
docs: finalize frontend tech stack for web UI

- Choose React + TypeScript + Vite
- Select Zustand for state management
- Plan shadcn/ui + Tailwind for components
```

---

## 3. 프론트엔드 프로젝트 초기화 (Commit 3)

### 요구사항
- 모노레포 구조에 프론트엔드 프로젝트 추가
- 기본 개발 환경 설정 완료

### 작업 내용
- [ ] 프론트엔드 프로젝트 생성
  ```bash
  cd /home/user/codex-ui
  pnpm create vite@latest codex-web-ui --template react-ts
  cd codex-web-ui
  pnpm install
  ```

- [ ] 필수 의존성 추가
  ```bash
  # UI 라이브러리
  pnpm add tailwindcss postcss autoprefixer
  pnpm add class-variance-authority clsx tailwind-merge
  pnpm add lucide-react

  # 상태 관리
  pnpm add zustand
  pnpm add @tanstack/react-query

  # 라우팅
  pnpm add react-router-dom

  # 통신
  pnpm add axios
  ```

- [ ] 개발 도구 설정
  ```bash
  pnpm add -D @types/node
  pnpm add -D eslint-config-prettier
  ```

- [ ] Tailwind CSS 초기화
  ```bash
  npx tailwindcss init -p
  ```

- [ ] 프로젝트 구조 설정
  ```
  codex-web-ui/
  ├── public/
  ├── src/
  │   ├── components/     # UI 컴포넌트
  │   ├── features/       # 기능별 모듈
  │   ├── hooks/          # 커스텀 훅
  │   ├── lib/            # 유틸리티
  │   ├── services/       # API 서비스
  │   ├── store/          # 상태 관리
  │   ├── types/          # TypeScript 타입
  │   ├── App.tsx
  │   └── main.tsx
  ├── package.json
  ├── tsconfig.json
  ├── vite.config.ts
  └── tailwind.config.js
  ```

- [ ] 기본 설정 파일 작성
  - `tsconfig.json` 경로 별칭 설정 (@/ 추가)
  - `vite.config.ts` 프록시 설정 (app-server 연동)
  - `tailwind.config.js` 설정
  - `.eslintrc.json` 설정
  - `.prettierrc` 설정

### 예상 결과물
- 실행 가능한 기본 React 앱
- Tailwind CSS가 적용된 환경

### Commit 메시지
```
feat(web-ui): initialize React frontend project

- Setup Vite + React + TypeScript
- Configure Tailwind CSS
- Add essential dependencies
- Setup project structure
```

---

## 4. 기본 레이아웃 및 디자인 시스템 구축 (Commit 4)

### 요구사항
- shadcn/ui 컴포넌트 설치 및 설정
- 기본 레이아웃 구조 구현
- 디자인 토큰 정의

### 작업 내용
- [ ] shadcn/ui 초기화
  ```bash
  npx shadcn@latest init
  ```

- [ ] 기본 컴포넌트 설치
  ```bash
  npx shadcn@latest add button
  npx shadcn@latest add input
  npx shadcn@latest add card
  npx shadcn@latest add dialog
  npx shadcn@latest add dropdown-menu
  npx shadcn@latest add tabs
  npx shadcn@latest add textarea
  npx shadcn@latest add scroll-area
  npx shadcn@latest add toast
  ```

- [ ] 디자인 토큰 정의 (`src/lib/design-tokens.ts`)
  ```typescript
  // 색상 스키마
  export const colors = {
    primary: { ... },
    secondary: { ... },
    accent: { ... },
    // CLI 터미널 느낌을 위한 색상
    terminal: {
      background: '#1e1e1e',
      text: '#d4d4d4',
      green: '#4ec9b0',
      blue: '#569cd6',
      yellow: '#dcdcaa',
      red: '#f48771',
    }
  }

  // 타이포그래피
  export const typography = { ... }

  // 간격
  export const spacing = { ... }
  ```

- [ ] 기본 레이아웃 컴포넌트 구현
  - `src/components/layout/AppLayout.tsx`: 전체 앱 레이아웃
  - `src/components/layout/Header.tsx`: 헤더 (로고, 네비게이션)
  - `src/components/layout/Sidebar.tsx`: 사이드바 (프로젝트 목록, 설정)
  - `src/components/layout/MainContent.tsx`: 메인 컨텐츠 영역

- [ ] 터미널 스타일 컴포넌트 구현
  - `src/components/terminal/Terminal.tsx`: 터미널 디스플레이
  - `src/components/terminal/TerminalOutput.tsx`: 출력 표시
  - `src/components/terminal/TerminalInput.tsx`: 입력 영역

### 예상 결과물
- 기본 레이아웃이 적용된 앱 껍데기
- 재사용 가능한 컴포넌트 라이브러리

### Commit 메시지
```
feat(web-ui): implement basic layout and design system

- Setup shadcn/ui components
- Define design tokens and color scheme
- Create AppLayout, Header, Sidebar components
- Implement terminal-style UI components
```

---

## 5. 개발 서버 통합 설정 (Commit 5)

### 요구사항
- Vite 개발 서버와 Rust app-server 연동
- CORS 설정
- API 프록시 구성

### 작업 내용
- [ ] app-server에 CORS 미들웨어 추가
  - `codex-rs/app-server/Cargo.toml`에 `tower-http` 의존성 추가
  - CORS 설정 코드 구현 (`src/cors.rs`)

- [ ] Vite 프록시 설정
  ```typescript
  // vite.config.ts
  export default defineConfig({
    server: {
      port: 3000,
      proxy: {
        '/api': {
          target: 'http://localhost:8080', // app-server 포트
          changeOrigin: true,
          rewrite: (path) => path.replace(/^\/api/, ''),
        },
        '/ws': {
          target: 'ws://localhost:8080',
          ws: true,
        },
      },
    },
  })
  ```

- [ ] API 클라이언트 설정
  ```typescript
  // src/lib/api-client.ts
  import axios from 'axios'

  export const apiClient = axios.create({
    baseURL: '/api',
    headers: {
      'Content-Type': 'application/json',
    },
  })
  ```

- [ ] WebSocket 클라이언트 설정
  ```typescript
  // src/lib/websocket-client.ts
  export class CodexWebSocket {
    // WebSocket 연결 관리
    // 재연결 로직
    // 메시지 핸들링
  }
  ```

- [ ] 환경 변수 설정
  - `.env.development` 생성
  - `.env.production` 생성

### 예상 결과물
- 프론트엔드에서 백엔드 API 호출 가능
- WebSocket 연결 가능

### Commit 메시지
```
feat(web-ui): integrate frontend with app-server

- Configure Vite proxy for API calls
- Setup CORS in app-server
- Implement API and WebSocket clients
- Add environment configuration
```

---

## 6. 기본 페이지 구현 (Commit 6)

### 요구사항
- 주요 페이지 라우팅 설정
- 기본 UI 페이지 구현

### 작업 내용
- [ ] React Router 설정
  ```typescript
  // src/App.tsx
  import { BrowserRouter, Routes, Route } from 'react-router-dom'

  function App() {
    return (
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<HomePage />} />
          <Route path="/chat" element={<ChatPage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Routes>
      </BrowserRouter>
    )
  }
  ```

- [ ] 홈 페이지 구현 (`src/pages/HomePage.tsx`)
  - 프로젝트 개요
  - 빠른 시작 가이드
  - 최근 대화 목록

- [ ] 채팅 페이지 구현 (`src/pages/ChatPage.tsx`)
  - 터미널 스타일 채팅 인터페이스
  - 메시지 입력 영역
  - 메시지 히스토리 표시

- [ ] 설정 페이지 구현 (`src/pages/SettingsPage.tsx`)
  - API 키 설정
  - 모델 선택
  - 테마 설정

### 예상 결과물
- 기본 페이지 네비게이션 가능
- 각 페이지의 기본 UI 표시

### Commit 메시지
```
feat(web-ui): implement basic pages and routing

- Setup React Router
- Create HomePage with project overview
- Implement ChatPage with terminal UI
- Add SettingsPage for configuration
```

---

## Day 1 완료 체크리스트

- [ ] 프로젝트 구조 완전히 이해
- [ ] 기술 스택 결정 완료
- [ ] 프론트엔드 프로젝트 초기화
- [ ] 기본 레이아웃 및 컴포넌트 구현
- [ ] 백엔드 연동 설정
- [ ] 기본 페이지 구현

---

## 다음 단계 (Day 2 예고)

1. 실시간 채팅 기능 구현
2. 에이전트 응답 스트리밍
3. 파일 업로드/다운로드 기능
4. 세션 관리 구현
5. 에러 처리 및 로딩 상태 관리

---

## 참고 문서

- [Vite 공식 문서](https://vitejs.dev/)
- [React 공식 문서](https://react.dev/)
- [shadcn/ui](https://ui.shadcn.com/)
- [Tailwind CSS](https://tailwindcss.com/)
- [TanStack Query](https://tanstack.com/query/latest)
