# Week 1 TODO - Codex Electron App 개발 로드맵

## 주간 목표
Electron 기반 standalone 데스크톱 애플리케이션으로 Codex의 모든 기능을 구현하고, 크로스 플랫폼 배포 준비 완료

---

## 개발 철학: Electron First

모든 기능을 처음부터 Electron 환경에서 개발합니다.
- ✅ Native OS 통합
- ✅ IPC 통신 활용
- ✅ 번들된 Rust 서버
- ✅ Offline 지원
- ✅ Auto-update

---

## Day 1: Electron + React 프로젝트 초기 설정

### Commits (6개)
1. **Electron 프로젝트 초기화** - electron-vite로 프로젝트 생성
2. **Rust 서버 번들링** - 서버 빌드 자동화 및 Main Process 통합
3. **IPC 통신 구조** - Preload script, Types, Handlers
4. **UI 기반 구축** - Tailwind, shadcn/ui, 커스텀 타이틀바
5. **라우팅 설정** - React Router, 기본 페이지
6. **개발 환경 최적화** - ESLint, Prettier, 빌드 테스트

**핵심 성과:**
- 독립 실행 가능한 Electron 앱
- 번들된 Rust 서버 자동 시작
- 타입 안전한 IPC 통신
- 커스텀 타이틀바

---

## Day 2: 실시간 채팅 및 상태 관리

### Commits (6개)

#### Commit 7: Zustand 상태 관리
- 메시지 타입 정의 (Message, ToolCall 등)
- Chat store 구현
- Electron store와 통합 (설정 영속화)

#### Commit 8: 채팅 UI 컴포넌트
- MessageList (가상 스크롤)
- MessageItem (사용자/AI 구분)
- MessageInput (키보드 단축키)
- CodeBlock (복사 버튼)

#### Commit 9: WebSocket 통신
- WebSocket 클라이언트
- 재연결 로직
- Main Process를 통한 서버 URL 가져오기
- IPC 이벤트로 연결 상태 공유

#### Commit 10: 스트리밍 응답
- SSE/WebSocket 스트리밍
- react-markdown 통합
- 실시간 코드 하이라이팅
- 타이핑 애니메이션

#### Commit 11: 메시지 기능
- 복사, 편집, 삭제
- 검색 (IPC를 통한 파일 시스템 검색)
- 내보내기 (Native dialog 사용)

#### Commit 12: 에러 처리
- Toast 알림 (react-hot-toast)
- 에러 바운더리
- Native notification 통합
- 재시도 로직

**Electron 특화:**
- electron-store로 메시지 영속화
- Native notification으로 백그라운드 알림
- IPC를 통한 파일 저장

---

## Day 3: 파일 작업 및 도구 UI

### Commits (6개)

#### Commit 13: 파일 탐색기
- 파일 트리 구조
- Native dialog로 폴더 선택
- IPC를 통한 파일 시스템 접근
- Drag & Drop 지원

#### Commit 14: Monaco Editor 통합
- 파일 뷰어
- 다중 탭 지원
- Native menu에 파일 메뉴 추가
- Cmd/Ctrl+S로 저장

#### Commit 15: 파일 업로드/다운로드
- Native file picker
- 진행률 표시
- IPC를 통한 파일 전송
- 시스템 알림

#### Commit 16: 도구 호출 시각화
- ToolCall 컴포넌트
- 실행 상태 표시
- 승인 다이얼로그 (Native)
- System tray 알림

#### Commit 17: Diff 뷰어
- react-diff-viewer
- Side-by-side 뷰
- Native save dialog

#### Commit 18: 승인 플로우
- Native dialog 활용
- System notification
- electron-store에 승인 설정 저장

**Electron 특화:**
- Native file dialogs (open, save)
- System tray notifications
- Menu bar integration
- Global shortcuts

---

## Day 4: 세션 관리 및 검색

### Commits (6개)

#### Commit 19: 세션 관리
- Session 타입 정의
- electron-store로 영속화
- IPC로 세션 CRUD
- Native menu에 세션 메뉴 추가

#### Commit 20: 세션 UI
- SessionList 사이드바
- Cmd/Ctrl+N으로 새 세션
- electron-store 동기화
- 최근 세션 자동 복원

#### Commit 21: 히스토리
- electron-store 백업
- Native save/open dialog로 가져오기/내보내기
- 자동 백업 (background)
- 앱 재시작 시 복원

#### Commit 22: 검색 기능
- Cmd/Ctrl+F로 검색 (Global shortcut)
- 전체 세션 검색
- Fuzzy matching
- 검색 결과 하이라이팅

#### Commit 23: 세션 내보내기
- Native save dialog
- JSON, Markdown, HTML, PDF
- Share menu (macOS)
- 클립보드 복사

#### Commit 24: 통계 대시보드
- Chart.js 통합
- 세션 분석
- Native print dialog

**Electron 특화:**
- electron-store 활용
- Global shortcuts (Cmd+F)
- Native dialogs
- Share menu (macOS)
- Print support

---

## Day 5: 설정 및 Native 통합

### Commits (6개)

#### Commit 25: 설정 관리
- Settings 타입
- electron-store 통합
- IPC handlers
- 테마 자동 적용 (nativeTheme)

#### Commit 26: 인증 설정
- API 키 암호화 (safeStorage)
- Keychain 통합 (macOS)
- OAuth flow (shell.openExternal)

#### Commit 27: 모델 설정
- 모델 파라미터 UI
- electron-store 영속화
- 프리셋 저장

#### Commit 28: 테마 및 외관
- nativeTheme 활용
- System theme 감지
- 다크/라이트 자동 전환
- 커스텀 accent color

#### Commit 29: 고급 설정
- MCP 서버 설정
- 샌드박스 옵션
- 실행 정책
- 디버그 모드 (DevTools 토글)

#### Commit 30: Native Menu
- Application menu
- Context menus
- Keyboard shortcuts
- Menu updates (dynamic)

**Electron 특화:**
- safeStorage로 민감 정보 암호화
- Keychain/Credential Manager 통합
- nativeTheme API
- Native menus
- Global shortcuts

---

## Day 6: 성능 및 UX 개선

### Commits (6개)

#### Commit 31: 키보드 단축키
- Global shortcuts (Cmd+K, Cmd+N 등)
- Local shortcuts (앱 내)
- Shortcuts 도움말 (Cmd+/)
- Menu accelerators

#### Commit 32: 명령 팔레트
- Cmd/Ctrl+K
- Fuzzy search (fuse.js)
- 최근 명령어
- IPC actions

#### Commit 33: 성능 최적화
- React.memo
- 가상 스크롤 (react-window)
- Code splitting
- Lazy loading
- Preload optimization

#### Commit 34: Native 통합
- System tray icon
- Badge count (Dock/Taskbar)
- Progress bar (macOS/Windows)
- Notifications

#### Commit 35: 접근성
- 키보드 네비게이션
- Screen reader support
- High contrast mode
- Zoom support

#### Commit 36: 반응형 및 창 관리
- Window state 저장
- Multi-window support
- Fullscreen mode
- Split view

**Electron 특화:**
- Global shortcuts registration
- System tray
- Dock/Taskbar integration
- Multi-window architecture
- Window state persistence

---

## Day 7: 테스트, 문서화, 배포

### Commits (6개)

#### Commit 37: 단위 테스트
- Vitest 설정
- React Testing Library
- IPC mocking
- Store 테스트

#### Commit 38: E2E 테스트
- Playwright for Electron
- Main/Renderer 테스트
- 사용자 플로우 테스트
- CI 통합

#### Commit 39: 문서화
- README
- 사용자 가이드
- 개발자 문서
- API 문서
- Changelog

#### Commit 40: 자동 업데이트
- electron-updater 설정
- Update channel (stable/beta)
- Release notes
- Auto-download

#### Commit 41: 코드 사이닝 및 노타리제이션
- macOS: Notarization
- Windows: Code signing
- Linux: AppImage

#### Commit 42: 배포 및 출시
- GitHub Releases
- Auto-publish workflow
- DMG/NSIS/AppImage
- Version bumping
- Release checklist

**Electron 특화:**
- electron-updater
- Code signing
- Notarization (macOS)
- Auto-publish
- Platform-specific installers

---

## 주간 완료 기준

### 기능 완성도
- [x] Electron 앱 실행
- [x] 번들된 서버 자동 시작
- [x] 실시간 채팅
- [x] 파일 탐색 및 편집
- [x] 세션 관리
- [x] 설정 및 테마
- [x] Native 통합 (menu, tray, shortcuts)
- [x] 자동 업데이트

### 플랫폼 지원
- [x] macOS (Intel + Apple Silicon)
- [x] Windows (x64)
- [x] Linux (x64, AppImage/deb)

### 배포 준비
- [x] Code signing 완료
- [x] Auto-update 설정
- [x] 설치 프로그램 생성
- [x] 문서 완성

---

## 기술 스택 (Electron 환경)

### Frontend
- React 18 + TypeScript
- Vite (electron-vite)
- Tailwind CSS + shadcn/ui
- Zustand + TanStack Query
- React Router

### Electron
- Electron 28+
- electron-builder (패키징)
- electron-updater (자동 업데이트)
- electron-store (설정 저장)

### Backend
- Bundled Rust Server (Child Process)
- IPC Communication
- Native APIs

### 개발 도구
- Vitest (테스팅)
- Playwright (E2E)
- ESLint + Prettier
- TypeScript

---

## 예상 최종 결과물

### macOS
```
Codex UI.app (Universal Binary)
├── Electron Framework
├── React UI (asar)
├── Bundled Rust Server
└── Resources
```

### Windows
```
Codex UI Setup.exe
├── Electron executable
├── React UI (asar)
├── codex-server.exe
└── Resources
```

### Linux
```
Codex-UI-x.x.x.AppImage
├── Electron executable
├── React UI (asar)
├── codex-server
└── Resources
```

---

## 주요 차이점: Web vs Electron

| 기능 | Web 버전 | Electron 버전 |
|------|----------|---------------|
| 설치 | 서버 필요 | 클릭 한 번 |
| 서버 | 별도 실행 | 자동 번들 |
| 파일 접근 | 제한적 | 전체 접근 |
| 단축키 | 브라우저 제약 | Global shortcuts |
| 알림 | Web notifications | Native notifications |
| 메뉴 | 없음 | Native menus |
| 업데이트 | 수동 | 자동 |
| 오프라인 | 제한적 | 완전 지원 |
| 설정 저장 | localStorage | Native store |
| 보안 | HTTPS 필요 | Code signing |

---

## 성공 기준

### 기술적 목표
- ⚡ 앱 시작 < 3초
- ⚡ 메모리 사용 < 300MB
- ⚡ 패키지 크기 < 200MB
- ⚡ Hot reload < 1초

### 사용자 목표
- 👥 한 번의 설치로 즉시 사용
- 🔄 백그라운드 자동 업데이트
- 💾 모든 설정 자동 저장
- 🔐 안전한 credentials 관리

---

## 로드맵

### Week 1 (현재)
- Day 1-2: 기본 구조 + 채팅
- Day 3-4: 파일 + 세션
- Day 5-6: 설정 + Native 통합
- Day 7: 테스트 + 배포

### Week 2-3 (후속)
- 고급 기능 추가
- 플러그인 시스템
- 협업 기능
- 마켓플레이스

### Week 4+
- 베타 출시
- 사용자 피드백
- 안정화
- 정식 출시

---

**Last Updated**: 2025-11-20
**Version**: 2.0 (Electron First)
**Status**: Ready to implement
