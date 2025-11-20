# Week 1 TODO - Codex Web UI 개발

## 주간 목표
Codex CLI의 핵심 기능을 웹 UI로 구현하고, 기본적인 에이전트 상호작용이 가능한 프로토타입 완성

---

## Day 1: 프로젝트 초기 설정 및 기본 구조 구축

### Morning (오전)

#### ✅ Commit 1: 프로젝트 분석
- [ ] `codex-rs/app-server` 전체 구조 분석 및 문서화
- [ ] `codex-rs/tui` 동작 방식 파악
- [ ] `codex-rs/app-server-protocol` 프로토콜 분석
- [ ] API 엔드포인트 매핑 완료
- [ ] 문서 작성: `docs/todo/architecture-analysis.md`

**Commit 메시지**: `docs: analyze project architecture for UI integration`

#### ✅ Commit 2: 기술 스택 결정
- [ ] 프론트엔드 프레임워크 비교 (React/Vue/Svelte)
- [ ] 상태 관리 라이브러리 선택
- [ ] UI 컴포넌트 라이브러리 결정
- [ ] 문서 작성: `docs/todo/tech-stack-decision.md`

**Commit 메시지**: `docs: finalize frontend tech stack for web UI`

### Afternoon (오후)

#### ✅ Commit 3: 프론트엔드 프로젝트 초기화
- [ ] Vite + React + TypeScript 프로젝트 생성
- [ ] 모노레포에 통합 (`pnpm-workspace.yaml` 수정)
- [ ] 필수 의존성 설치 (Tailwind, Zustand, TanStack Query 등)
- [ ] 프로젝트 폴더 구조 설정
- [ ] 기본 설정 파일 작성 (tsconfig, vite.config, tailwind.config)

**Commit 메시지**: `feat(web-ui): initialize React frontend project`

#### ✅ Commit 4: 디자인 시스템 구축
- [ ] shadcn/ui 초기화
- [ ] 기본 컴포넌트 설치 (Button, Input, Card 등)
- [ ] 디자인 토큰 정의 (`src/lib/design-tokens.ts`)
- [ ] AppLayout, Header, Sidebar 컴포넌트 구현
- [ ] Terminal 스타일 컴포넌트 구현

**Commit 메시지**: `feat(web-ui): implement basic layout and design system`

### Evening (저녁)

#### ✅ Commit 5: 백엔드 통합 설정
- [ ] app-server에 CORS 미들웨어 추가
- [ ] Vite 프록시 설정
- [ ] API 클라이언트 구현 (`src/lib/api-client.ts`)
- [ ] WebSocket 클라이언트 구현 (`src/lib/websocket-client.ts`)
- [ ] 환경 변수 설정

**Commit 메시지**: `feat(web-ui): integrate frontend with app-server`

#### ✅ Commit 6: 기본 페이지 구현
- [ ] React Router 설정
- [ ] HomePage 구현 (대시보드)
- [ ] ChatPage 구현 (터미널 인터페이스)
- [ ] SettingsPage 구현
- [ ] 네비게이션 동작 확인

**Commit 메시지**: `feat(web-ui): implement basic pages and routing`

---

## Day 2: 실시간 채팅 기능 구현

### Morning (오전)

#### ✅ Commit 7: 메시지 상태 관리
- [ ] Zustand 스토어 설정 (`src/store/chat-store.ts`)
  ```typescript
  interface ChatState {
    messages: Message[]
    isLoading: boolean
    addMessage: (message: Message) => void
    clearMessages: () => void
  }
  ```
- [ ] 메시지 타입 정의 (`src/types/message.ts`)
- [ ] 메시지 히스토리 관리 로직

**Commit 메시지**: `feat(web-ui): implement message state management`

#### ✅ Commit 8: 채팅 UI 컴포넌트
- [ ] MessageList 컴포넌트 (`src/components/chat/MessageList.tsx`)
- [ ] MessageItem 컴포넌트 (사용자/에이전트 메시지 구분)
- [ ] MessageInput 컴포넌트 (입력 영역)
- [ ] 자동 스크롤 기능
- [ ] 타이핑 인디케이터

**Commit 메시지**: `feat(web-ui): create chat UI components`

### Afternoon (오후)

#### ✅ Commit 9: WebSocket 통신 구현
- [ ] WebSocket 연결 관리자 개선
- [ ] 메시지 송수신 로직
- [ ] 재연결 로직 (exponential backoff)
- [ ] 연결 상태 표시 UI

**Commit 메시지**: `feat(web-ui): implement WebSocket communication`

#### ✅ Commit 10: 스트리밍 응답 처리
- [ ] SSE(Server-Sent Events) 또는 WebSocket 스트리밍 구현
- [ ] 실시간 응답 렌더링
- [ ] 마크다운 렌더링 (react-markdown)
- [ ] 코드 블록 하이라이팅 (prism.js 또는 highlight.js)

**Commit 메시지**: `feat(web-ui): add streaming response handling`

### Evening (저녁)

#### ✅ Commit 11: 메시지 기능 개선
- [ ] 코드 복사 버튼
- [ ] 메시지 편집/삭제
- [ ] 메시지 검색 기능
- [ ] 메시지 필터링

**Commit 메시지**: `feat(web-ui): enhance message functionality`

#### ✅ Commit 12: 에러 처리
- [ ] API 에러 핸들링
- [ ] WebSocket 연결 실패 처리
- [ ] 사용자 친화적 에러 메시지
- [ ] Toast 알림 구현

**Commit 메시지**: `feat(web-ui): implement error handling and notifications`

---

## Day 3: 파일 작업 및 도구 호출 UI

### Morning (오전)

#### ✅ Commit 13: 파일 탐색기 UI
- [ ] FileExplorer 컴포넌트 (`src/components/files/FileExplorer.tsx`)
- [ ] 트리 뷰 구현 (react-arborist 또는 직접 구현)
- [ ] 파일/폴더 아이콘 표시
- [ ] 파일 선택 및 열기 기능

**Commit 메시지**: `feat(web-ui): implement file explorer component`

#### ✅ Commit 14: 파일 뷰어
- [ ] FileViewer 컴포넌트
- [ ] 코드 에디터 통합 (Monaco Editor 또는 CodeMirror)
- [ ] 문법 하이라이팅
- [ ] 읽기 전용 모드

**Commit 메시지**: `feat(web-ui): add file viewer with syntax highlighting`

### Afternoon (오후)

#### ✅ Commit 15: 파일 업로드/다운로드
- [ ] 파일 업로드 컴포넌트
- [ ] 드래그 앤 드롭 지원
- [ ] 파일 다운로드 기능
- [ ] 진행률 표시

**Commit 메시지**: `feat(web-ui): implement file upload and download`

#### ✅ Commit 16: 도구 호출 시각화
- [ ] ToolCall 컴포넌트 (`src/components/chat/ToolCall.tsx`)
- [ ] 도구 실행 상태 표시 (pending/running/completed/failed)
- [ ] 도구 입력/출력 표시
- [ ] 확장/축소 가능한 UI

**Commit 메시지**: `feat(web-ui): visualize tool calls in chat`

### Evening (저녁)

#### ✅ Commit 17: 파일 diff 뷰어
- [ ] FileDiff 컴포넌트
- [ ] 변경 사항 하이라이팅
- [ ] side-by-side 또는 unified 뷰
- [ ] react-diff-viewer 통합

**Commit 메시지**: `feat(web-ui): add file diff viewer`

#### ✅ Commit 18: 승인 플로우 UI
- [ ] ApprovalDialog 컴포넌트
- [ ] 도구 실행 전 사용자 승인 요청
- [ ] 승인/거부 버튼
- [ ] 항상 허용 옵션

**Commit 메시지**: `feat(web-ui): implement approval flow for tool execution`

---

## Day 4: 세션 및 히스토리 관리

### Morning (오전)

#### ✅ Commit 19: 세션 관리 구조
- [ ] Session 타입 정의 (`src/types/session.ts`)
- [ ] 세션 스토어 구현 (`src/store/session-store.ts`)
- [ ] 세션 생성/삭제/전환 로직
- [ ] localStorage에 세션 저장

**Commit 메시지**: `feat(web-ui): implement session management`

#### ✅ Commit 20: 세션 UI
- [ ] SessionList 컴포넌트 (사이드바)
- [ ] 세션 생성 버튼
- [ ] 세션 이름 변경
- [ ] 세션 삭제 확인 다이얼로그

**Commit 메시지**: `feat(web-ui): create session management UI`

### Afternoon (오후)

#### ✅ Commit 21: 히스토리 저장 및 로드
- [ ] IndexedDB 또는 localStorage 활용
- [ ] 세션별 메시지 히스토리 저장
- [ ] 페이지 새로고침 시 복원
- [ ] 히스토리 내보내기/가져오기

**Commit 메시지**: `feat(web-ui): persist and restore chat history`

#### ✅ Commit 22: 검색 기능
- [ ] 전체 세션 검색
- [ ] 메시지 내용 검색
- [ ] 파일명 검색
- [ ] 검색 결과 하이라이팅

**Commit 메시지**: `feat(web-ui): add search functionality`

### Evening (저녁)

#### ✅ Commit 23: 세션 내보내기
- [ ] 세션을 JSON으로 내보내기
- [ ] 세션을 Markdown으로 내보내기
- [ ] 공유 가능한 링크 생성 (선택사항)

**Commit 메시지**: `feat(web-ui): export sessions in multiple formats`

#### ✅ Commit 24: 세션 통계
- [ ] 세션당 메시지 수
- [ ] 도구 사용 통계
- [ ] 세션 소요 시간
- [ ] 통계 대시보드 페이지

**Commit 메시지**: `feat(web-ui): add session statistics dashboard`

---

## Day 5: 설정 및 커스터마이징

### Morning (오전)

#### ✅ Commit 25: 설정 관리
- [ ] Settings 타입 정의 (`src/types/settings.ts`)
- [ ] 설정 스토어 구현 (`src/store/settings-store.ts`)
- [ ] 설정 저장/로드
- [ ] 기본값 관리

**Commit 메시지**: `feat(web-ui): implement settings management`

#### ✅ Commit 26: 인증 설정 UI
- [ ] API 키 입력 필드
- [ ] ChatGPT 로그인 버튼
- [ ] 인증 상태 표시
- [ ] 로그아웃 기능

**Commit 메시지**: `feat(web-ui): create authentication settings UI`

### Afternoon (오후)

#### ✅ Commit 27: 모델 설정 UI
- [ ] 모델 선택 드롭다운
- [ ] 사용 가능한 모델 목록 가져오기
- [ ] 모델 파라미터 조정 (temperature, max_tokens 등)
- [ ] 프리셋 저장

**Commit 메시지**: `feat(web-ui): add model configuration UI`

#### ✅ Commit 28: 테마 및 외관 설정
- [ ] 라이트/다크 모드 토글
- [ ] 터미널 색상 스키마 선택
- [ ] 폰트 크기 조정
- [ ] 레이아웃 설정 (사이드바 위치 등)

**Commit 메시지**: `feat(web-ui): implement theme and appearance settings`

### Evening (저녁)

#### ✅ Commit 29: 고급 설정
- [ ] MCP 서버 설정
- [ ] 샌드박스 옵션
- [ ] 실행 정책 설정
- [ ] 디버그 모드

**Commit 메시지**: `feat(web-ui): add advanced settings panel`

#### ✅ Commit 30: 설정 검증
- [ ] 설정 값 유효성 검사
- [ ] 잘못된 설정 경고
- [ ] 기본값으로 재설정 옵션
- [ ] 설정 백업/복원

**Commit 메시지**: `feat(web-ui): validate and backup settings`

---

## Day 6: 고급 기능 및 개선

### Morning (오전)

#### ✅ Commit 31: 키보드 단축키
- [ ] 단축키 시스템 구현 (react-hotkeys-hook)
- [ ] 새 채팅: Cmd/Ctrl + N
- [ ] 검색: Cmd/Ctrl + F
- [ ] 설정: Cmd/Ctrl + ,
- [ ] 단축키 도움말 모달

**Commit 메시지**: `feat(web-ui): implement keyboard shortcuts`

#### ✅ Commit 32: 명령 팔레트
- [ ] CommandPalette 컴포넌트 (Cmd/Ctrl + K)
- [ ] fuzzy 검색 (fuse.js)
- [ ] 최근 명령어 표시
- [ ] 액션 실행

**Commit 메시지**: `feat(web-ui): add command palette`

### Afternoon (오후)

#### ✅ Commit 33: 성능 최적화
- [ ] React.memo 적용
- [ ] useMemo, useCallback 최적화
- [ ] 가상 스크롤링 (react-window)
- [ ] 코드 스플리팅 (lazy loading)

**Commit 메시지**: `perf(web-ui): optimize component rendering`

#### ✅ Commit 34: 로딩 상태 개선
- [ ] 스켈레톤 UI
- [ ] 스피너 컴포넌트
- [ ] 진행률 표시
- [ ] Suspense 경계 설정

**Commit 메시지**: `feat(web-ui): improve loading states`

### Evening (저녁)

#### ✅ Commit 35: 접근성 개선
- [ ] 키보드 네비게이션
- [ ] ARIA 레이블 추가
- [ ] 포커스 관리
- [ ] 스크린 리더 지원

**Commit 메시지**: `feat(web-ui): enhance accessibility`

#### ✅ Commit 36: 반응형 디자인
- [ ] 모바일 레이아웃
- [ ] 태블릿 레이아웃
- [ ] 브레이크포인트 설정
- [ ] 터치 제스처 지원

**Commit 메시지**: `feat(web-ui): implement responsive design`

---

## Day 7: 테스트, 문서화, 배포 준비

### Morning (오전)

#### ✅ Commit 37: 단위 테스트
- [ ] Vitest 설정
- [ ] React Testing Library 설정
- [ ] 주요 컴포넌트 테스트
- [ ] 유틸리티 함수 테스트

**Commit 메시지**: `test(web-ui): add unit tests for components`

#### ✅ Commit 38: 통합 테스트
- [ ] API 통신 테스트 (MSW)
- [ ] WebSocket 테스트
- [ ] E2E 테스트 설정 (Playwright)
- [ ] 주요 사용자 플로우 테스트

**Commit 메시지**: `test(web-ui): add integration and e2e tests`

### Afternoon (오후)

#### ✅ Commit 39: 문서화
- [ ] README.md 작성
- [ ] 컴포넌트 문서 (Storybook 선택사항)
- [ ] API 문서
- [ ] 개발 가이드

**Commit 메시지**: `docs(web-ui): add comprehensive documentation`

#### ✅ Commit 40: 빌드 최적화
- [ ] 프로덕션 빌드 설정
- [ ] 번들 크기 최적화
- [ ] 이미지 최적화
- [ ] 캐싱 전략

**Commit 메시지**: `build(web-ui): optimize production build`

### Evening (저녁)

#### ✅ Commit 41: 배포 설정
- [ ] Docker 설정 (선택사항)
- [ ] 정적 파일 서빙 설정
- [ ] 환경 변수 관리
- [ ] CI/CD 파이프라인 (.github/workflows)

**Commit 메시지**: `ci(web-ui): setup deployment pipeline`

#### ✅ Commit 42: 최종 점검 및 정리
- [ ] 코드 린팅 및 포맷팅
- [ ] 사용하지 않는 의존성 제거
- [ ] TODO 주석 정리
- [ ] CHANGELOG 업데이트

**Commit 메시지**: `chore(web-ui): final cleanup and polish`

---

## Week 1 완료 체크리스트

### 기능 완성도
- [ ] 기본 채팅 인터페이스 동작
- [ ] 실시간 에이전트 응답
- [ ] 파일 탐색 및 뷰어
- [ ] 도구 호출 시각화
- [ ] 세션 관리
- [ ] 설정 페이지
- [ ] 검색 기능
- [ ] 키보드 단축키

### 코드 품질
- [ ] TypeScript 타입 안정성
- [ ] ESLint 규칙 준수
- [ ] Prettier 포맷팅
- [ ] 주요 기능 테스트 커버리지 > 60%

### 사용자 경험
- [ ] 로딩 상태 표시
- [ ] 에러 처리
- [ ] 반응형 디자인
- [ ] 접근성 기본 준수

### 문서화
- [ ] README 작성
- [ ] API 문서
- [ ] 개발 가이드
- [ ] 배포 가이드

---

## 주요 마일스톤

### 🎯 Milestone 1 (Day 1-2)
**목표**: 기본 프로젝트 설정 및 채팅 UI
- 프론트엔드 프로젝트 초기화
- 기본 레이아웃
- 실시간 채팅 기능

### 🎯 Milestone 2 (Day 3-4)
**목표**: 파일 작업 및 세션 관리
- 파일 탐색기
- 도구 호출 UI
- 세션 관리

### 🎯 Milestone 3 (Day 5-6)
**목표**: 설정 및 고급 기능
- 설정 페이지
- 성능 최적화
- 접근성 개선

### 🎯 Milestone 4 (Day 7)
**목표**: 테스트 및 배포 준비
- 테스트 작성
- 문서화
- 배포 설정

---

## 리스크 및 대응 방안

### 기술적 리스크
1. **WebSocket 연결 안정성**
   - 대응: 재연결 로직, 폴백 옵션 (HTTP polling)

2. **실시간 스트리밍 성능**
   - 대응: 가상 스크롤링, 메시지 페이지네이션

3. **파일 크기 제한**
   - 대응: 청크 업로드, 압축

### 일정 리스크
1. **예상보다 긴 백엔드 통합 시간**
   - 대응: 모의 API 사용, 프론트엔드 독립 개발

2. **디자인 시스템 구축 지연**
   - 대응: 기본 컴포넌트 먼저 구현, 점진적 개선

---

## 성공 기준

### 최소 요구사항 (MVP)
✅ 사용자가 웹 브라우저에서 Codex 에이전트와 대화할 수 있다
✅ 실시간 응답 스트리밍이 동작한다
✅ 파일 시스템을 탐색하고 파일을 볼 수 있다
✅ 도구 호출이 시각화된다
✅ 세션을 저장하고 불러올 수 있다

### 추가 목표
🎁 모바일 기기에서도 사용 가능
🎁 오프라인 모드 지원
🎁 다국어 지원
🎁 플러그인 시스템

---

## 참고 자료

### 공식 문서
- [React 공식 문서](https://react.dev/)
- [Vite 문서](https://vitejs.dev/)
- [shadcn/ui](https://ui.shadcn.com/)
- [Tailwind CSS](https://tailwindcss.com/)
- [TanStack Query](https://tanstack.com/query/latest)
- [Zustand](https://docs.pmnd.rs/zustand)

### 도구
- [React DevTools](https://react.dev/learn/react-developer-tools)
- [Redux DevTools](https://github.com/reduxjs/redux-devtools)
- [Lighthouse](https://developers.google.com/web/tools/lighthouse)

### 영감
- [ChatGPT Web UI](https://chat.openai.com/)
- [Cursor IDE](https://cursor.sh/)
- [Warp Terminal](https://www.warp.dev/)
- [VS Code](https://code.visualstudio.com/)

---

## 팀 커뮤니케이션

### 일일 체크인
- [ ] 매일 아침 9시: 오늘 목표 확인
- [ ] 매일 오후 6시: 진행 상황 공유

### 주간 리뷰
- [ ] 금요일 오후 5시: 주간 회고
- [ ] 다음 주 계획 수립

---

**Last Updated**: 2025-11-20
**Version**: 1.0
**Author**: Claude Code Assistant
