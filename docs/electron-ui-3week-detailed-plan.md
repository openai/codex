# Codex Electron UI - 3주 상세 실행 계획

> WOW 요소 중심의 임원 데모용 POC

## 📋 의사결정 확정

✅ **1. RAG 벡터 DB**: In-Memory (빠른 구현)
✅ **2. 멀티모달**: 포함 (WOW 요소 핵심!)
✅ **3. 패키징**: macOS만 (DMG)
✅ **4. 코드 하이라이팅**: Prism.js
✅ **5. 대화 저장**: JSON 파일

---

## 🎯 WOW 요소 우선순위

임원이 눈으로 보고 감탄할 항목들:

1. 🖼️ **멀티모달** - "UI 스크린샷 → React 코드" 자동 생성
2. 📊 **Diff 뷰어** - 변경사항을 아름다운 비교 화면으로
3. 🧠 **RAG 시각화** - 관련 파일이 자동으로 찾아지는 모습
4. ✨ **스트리밍 애니메이션** - AI가 타이핑하는 것처럼 표시
5. 🎨 **아름다운 UI** - 다크 모드, 부드러운 애니메이션
6. 🔒 **지능형 도구 승인** - 명령의 영향 분석 + 위험도 표시 + 샌드박스 차단 증명
7. 📁 **VS Code 스타일 파일 브라우저** - 색상 아이콘 + 애니메이션
8. 📈 **실시간 진행 상황** - 프로그레스 바 + 현재 작업 표시
9. 🎉 **토스트 알림** - 작업 완료 피드백
10. 🌊 **부드러운 전환** - 모든 UI 요소에 애니메이션

---

## Week 1: 기반 구축 (Day 1-5)

### 📅 Day 1: 프로젝트 셋업 & 기본 구조

#### 오전 (4시간)
- [ ] Electron 프로젝트 초기화
  ```bash
  mkdir electron-ui && cd electron-ui
  npm init -y
  npm install electron react react-dom
  npm install -D vite @vitejs/plugin-react typescript
  npm install -D @types/react @types/react-dom
  ```
- [ ] Vite 설정 (electron-vite 또는 수동 설정)
- [ ] TypeScript 설정 (tsconfig.json)
- [ ] 기본 디렉토리 구조 생성
  ```
  main/
  preload/
  renderer/src/
  ```

#### 오후 (4시간)
- [ ] Main Process 기본 구현
  - [ ] BrowserWindow 생성
  - [ ] 개발 환경 설정 (DevTools)
- [ ] Preload Script 기본 틀
  - [ ] contextBridge 설정
- [ ] Renderer 기본 React 앱
  - [ ] "Hello Electron" 화면
- [ ] Hot Reload 동작 확인

**산출물**: ✅ Electron 앱 실행 가능

---

### 📅 Day 2: 레이아웃 & 스타일링 시스템 (WOW 요소 1)

#### 오전 (4시간)
- [ ] Tailwind CSS 설치 및 설정
  ```bash
  npm install -D tailwindcss postcss autoprefixer
  npx tailwindcss init -p
  ```
- [ ] 다크 모드 테마 설정
  - [ ] CSS 변수 정의 (색상, 간격)
  - [ ] 다크/라이트 토글 준비
- [ ] 기본 레이아웃 컴포넌트
  ```tsx
  <Layout>
    <Sidebar /> {/* 250px 고정 */}
    <MainContent />
    <StatusBar />
  </Layout>
  ```

#### 오후 (4시간)
- [ ] Sidebar 구현
  - [ ] 토글 버튼
  - [ ] 리사이즈 가능 (150px-500px)
  - [ ] 탭 구조 (Conversations, Files)
- [ ] StatusBar 구현
  - [ ] 좌: 연결 상태 인디케이터
  - [ ] 우: 샌드박스 모드 표시
- [ ] 부드러운 애니메이션 추가
  - [ ] Framer Motion 설치
  - [ ] 사이드바 슬라이드 애니메이션

**산출물**: ✅ 아름다운 기본 레이아웃 (WOW!)

---

### 📅 Day 3: Backend 통신 (핵심 인프라)

#### 오전 (4시간)
- [ ] BackendManager 클래스 구현 (main/backend-manager.ts)
  - [ ] Rust app-server 프로세스 spawn
  - [ ] stdio 파이프 설정
  - [ ] JSON-RPC 메시지 파싱
- [ ] IPC 핸들러 설정 (main/ipc-handlers.ts)
  - [ ] 'backend:send' 핸들러
  - [ ] 'backend:event' 이벤트 전달

#### 오후 (4시간)
- [ ] Preload API 확장
  ```typescript
  codexAPI.sendMessage(msg)
  codexAPI.onEvent(callback)
  codexAPI.onError(callback)
  ```
- [ ] Backend Service (renderer)
  - [ ] JSON-RPC 클라이언트
  - [ ] 요청/응답 매칭 (ID 기반)
  - [ ] EventEmitter로 이벤트 전달
- [ ] 에러 핸들링
  - [ ] 백엔드 크래시 감지 → 자동 재시작
  - [ ] 연결 끊김 UI 표시

**산출물**: ✅ Backend와 통신 가능

---

### 📅 Day 4: 기본 Chat UI (WOW 요소 2)

#### 오전 (4시간)
- [ ] 메시지 데이터 구조 정의
  ```typescript
  interface Message {
    id: string;
    role: 'user' | 'assistant';
    content: string;
    timestamp: number;
  }
  ```
- [ ] Zustand Store 설정
  ```typescript
  useConversationStore: {
    messages: Message[];
    addMessage, updateMessage
  }
  ```
- [ ] MessageList 컴포넌트
  - [ ] 사용자/AI 메시지 구분 (좌/우 정렬)
  - [ ] 자동 스크롤 (최신 메시지로)

#### 오후 (4시간)
- [ ] InputArea 컴포넌트
  - [ ] Textarea (자동 높이 조절)
  - [ ] Shift+Enter = 줄바꿈
  - [ ] Enter = 전송
  - [ ] 전송 버튼
- [ ] 메시지 전송 로직
  - [ ] Backend에 JSON-RPC 요청
  - [ ] 낙관적 UI 업데이트 (즉시 표시)
- [ ] **스트리밍 애니메이션** (WOW!)
  - [ ] 타이핑 효과 (글자 하나씩)
  - [ ] 커서 깜빡임 효과

**산출물**: ✅ 채팅 가능 + 타이핑 애니메이션 (WOW!)

---

### 📅 Day 5: 마크다운 렌더링 (WOW 요소 3)

#### 오전 (4시간)
- [ ] react-markdown 설치
  ```bash
  npm install react-markdown remark-gfm rehype-raw
  ```
- [ ] Prism.js 설치 및 설정
  ```bash
  npm install prismjs
  ```
- [ ] 코드 블록 하이라이팅
  - [ ] 언어별 테마 (VS Code Dark+)
  - [ ] 줄 번호 표시
  - [ ] **복사 버튼** (WOW!)

#### 오후 (4시간)
- [ ] 마크다운 스타일링
  - [ ] 헤딩, 리스트, 테이블
  - [ ] 인라인 코드 (백틱)
  - [ ] 블록쿼트
- [ ] **코드 블록 확대/축소** (WOW!)
  - [ ] 클릭 시 전체 화면 모달
- [ ] 링크 클릭 시 외부 브라우저 열기
  ```typescript
  shell.openExternal(url)
  ```

**산출물**: ✅ 아름다운 마크다운 렌더링 (WOW!)

**Week 1 체크포인트**: 기본 채팅 가능, 아름다운 UI ✨

---

## Week 2: WOW 요소 집중 (Day 6-10)

### 📅 Day 6: RAG - 코드베이스 인덱싱 (WOW 요소 4)

#### 오전 (4시간)
- [ ] RAG Service 구조 설계 (main/rag/)
  ```
  rag/
  ├── indexer.ts      # 파일 인덱싱
  ├── embeddings.ts   # 임베딩 생성
  ├── vectordb.ts     # In-Memory 벡터 DB
  └── retriever.ts    # 검색
  ```
- [ ] In-Memory 벡터 DB 구현
  ```typescript
  class InMemoryVectorDB {
    private chunks: CodeChunk[] = [];

    insert(chunk: CodeChunk): void
    search(query: number[], topK: number): CodeChunk[]
    // 코사인 유사도 계산
  }
  ```

#### 오후 (4시간)
- [ ] 파일 인덱싱 로직
  - [ ] 작업 공간 스캔 (.gitignore 준수)
  - [ ] 코드 파일 필터링 (.js, .ts, .py, .java 등)
  - [ ] 파일 내용 읽기
  - [ ] 간단한 청킹 (500자 단위)
- [ ] OpenAI Embeddings API 연동
  ```typescript
  async function generateEmbedding(text: string) {
    const response = await openai.embeddings.create({
      model: "text-embedding-3-small",
      input: text,
    });
    return response.data[0].embedding;
  }
  ```

**산출물**: ✅ 코드베이스 인덱싱 가능

---

### 📅 Day 7: RAG - 검색 & UI 시각화 (WOW 요소 5)

#### 오전 (4시간)
- [ ] 관련 코드 검색 로직
  ```typescript
  async findRelevantCode(query: string, topK = 5)
  ```
- [ ] 프롬프트 증강
  - [ ] 사용자 메시지에서 키워드 추출
  - [ ] 벡터 검색 실행
  - [ ] 검색 결과를 프롬프트에 포함
  ```
  [Context]
  File: api/routes/auth.ts
  <code>

  [User Message]
  Add authentication
  ```

#### 오후 (4시간)
- [ ] **RAG 시각화 UI** (WOW!)
  - [ ] 사이드바에 "Related Files" 패널
  - [ ] 검색된 파일 목록 표시
    ```tsx
    <FileItem>
      <FileIcon />
      <FileName>api/routes/auth.ts</FileName>
      <Similarity>87%</Similarity> {/* 진한 색 */}
    </FileItem>
    ```
  - [ ] 유사도 막대 그래프
  - [ ] **파일 클릭 시 미리보기** (WOW!)
- [ ] 검색 중 로딩 애니메이션
  - [ ] "Searching codebase..." 스피너

**산출물**: ✅ RAG 작동 + 시각화 (WOW!)

---

### 📅 Day 8: 도구 승인 UI - Diff 뷰어 (WOW 요소 6)

#### 오전 (4시간)
- [ ] react-diff-viewer 설치
  ```bash
  npm install react-diff-viewer-continued
  ```
- [ ] ApprovalDialog 컴포넌트
  - [ ] 모달 오버레이 (어두운 배경)
  - [ ] 도구 정보 카드
    ```tsx
    <ToolCard>
      <ToolName>Edit File</ToolName>
      <ToolDescription>Modify api/routes.ts</ToolDescription>
    </ToolCard>
    ```
- [ ] **Diff 뷰어 통합** (WOW!)
  - [ ] Side-by-side 모드
  - [ ] 변경된 라인 하이라이팅
  - [ ] 추가(녹색) / 삭제(빨간색)

#### 오후 (4시간)
- [ ] 승인 플로우 구현
  - [ ] Backend에서 'approval_needed' 이벤트 수신
  - [ ] ApprovalDialog 자동 열림
  - [ ] 승인/거부 버튼
    ```tsx
    <Button onClick={approve}>Approve (A)</Button>
    <Button onClick={reject}>Reject (R)</Button>
    ```
- [ ] 키보드 단축키
  - [ ] A = 승인
  - [ ] R = 거부
  - [ ] Esc = 다이얼로그 닫기
- [ ] **애니메이션** (WOW!)
  - [ ] 다이얼로그 페이드 인
  - [ ] Diff 라인 하나씩 나타남

**산출물**: ✅ 아름다운 Diff 뷰어 (WOW!)

---

### 📅 Day 9: 지능형 도구 승인 UI & 보안 데모 (WOW 요소 7)

#### 오전 (4시간)
- [ ] **강화된 명령 미리보기** (WOW!)
  ```tsx
  <CommandApprovalDialog>
    <Terminal>$ npm install express</Terminal>

    <ImpactAnalysis>
      <h4>📋 이 명령이 수행할 작업:</h4>
      ✅ Allowed: Read package.json
      ✅ Allowed: Write package.json
      ✅ Allowed: Write node_modules/
      ⚠️  Network: Download from npmjs.org
    </ImpactAnalysis>
  </CommandApprovalDialog>
  ```
- [ ] **위험도 자동 분석** (WOW!)
  - [ ] 안전 (녹색): `npm install`, `git status`
  - [ ] 주의 (노란색): `rm file.txt`, `mv src/ backup/`
  - [ ] 위험 (빨간색): `rm -rf`, `sudo`, `/etc/*` 접근
  - [ ] 위험 명령 감지 시 큰 경고 배너

#### 오후 (4시간)
- [ ] **보안 데모 시나리오 준비** (WOW!)
  - [ ] 시나리오 A: 안전한 작업
    ```
    "Add authentication"
    → workspace 파일만 수정
    → 승인 → 성공 ✅
    ```
  - [ ] 시나리오 B: 위험한 시도
    ```
    "Delete all files in /Users"
    → 경고 다이얼로그 표시
    → 시도 시 샌드박스 차단
    → 에러 메시지: "Permission denied (sandbox)"
    ```
- [ ] **샌드박스 차단 시 명확한 피드백**
  ```tsx
  <ErrorDialog type="sandbox-blocked">
    <Icon>🚫</Icon>
    <Title>작업이 차단되었습니다</Title>
    <Message>
      샌드박스가 workspace 외부 접근을 차단했습니다.
      경로: /Users/...
    </Message>
    <Details>
      현재 모드: Workspace Write
      허용 범위: /project/workspace/ 만
    </Details>
  </ErrorDialog>
  ```
- [ ] StatusBar에 샌드박스 모드 표시 (간단히)
  ```tsx
  <SandboxBadge mode="workspace-write">
    🔒 Workspace
  </SandboxBadge>
  ```

**산출물**: ✅ 지능형 도구 승인 + 보안 데모 시나리오 (WOW!)

---

### 📅 Day 10: 파일 브라우저 (WOW 요소 8)

#### 오전 (4시간)
- [ ] 작업 디렉토리 선택
  ```typescript
  ipcMain.handle('file:select-directory', async () => {
    const result = await dialog.showOpenDialog({
      properties: ['openDirectory']
    });
    return result.filePaths[0];
  });
  ```
- [ ] 파일 트리 컴포넌트
  - [ ] 재귀적 폴더/파일 구조
  - [ ] 폴더 펼치기/접기 애니메이션
  - [ ] .gitignore 파일 제외

#### 오후 (4시간)
- [ ] **파일 아이콘** (WOW!)
  - [ ] VS Code 아이콘 세트 (react-icons)
  - [ ] 확장자별 색상
    ```tsx
    .ts → 파란색
    .py → 노란색
    .js → 노란색
    .md → 회색
    ```
- [ ] **파일 클릭 시 미리보기** (WOW!)
  - [ ] 코드 하이라이팅
  - [ ] 읽기 전용
- [ ] 검색 기능 (간단히)
  - [ ] 파일명으로 필터링

**산출물**: ✅ 아름다운 파일 브라우저 (WOW!)

**Week 2 체크포인트**: 모든 핵심 WOW 요소 완성! ✨

---

## Week 3: 멀티모달 + 폴리싱 (Day 11-15)

### 📅 Day 11: 설정 패널

#### 오전 (4시간)
- [ ] Settings 모달 컴포넌트
  - [ ] Cmd+, 로 열기
  - [ ] 탭 기반 (General, Model, Sandbox)
- [ ] General 탭
  - [ ] 테마 전환 (다크/라이트)
  - [ ] 폰트 크기 조절
- [ ] Model 탭
  - [ ] 사용 가능 모델 목록 (dropdown)
  - [ ] 기본 모델 선택

#### 오후 (4시간)
- [ ] Sandbox 탭
  - [ ] 모드 선택 (라디오 버튼)
    - [ ] Read-only
    - [ ] Workspace-write
    - [ ] Full-access
  - [ ] 승인 모드 (checkbox)
    - [ ] Suggest
    - [ ] Auto Edit
    - [ ] Full Auto
- [ ] 설정 저장 (JSON 파일)
  ```typescript
  app.getPath('userData') + '/settings.json'
  ```

**산출물**: ✅ 설정 변경 가능

---

### 📅 Day 12: 진행 상황 표시 (WOW 요소 9)

#### 오전 (4시간)
- [ ] **진행 상황 인디케이터** (WOW!)
  - [ ] StatusBar에 프로그레스 바
  - [ ] 현재 실행 중인 작업 표시
    ```tsx
    <ProgressBar>
      Executing: npm install express
    </ProgressBar>
    ```
- [ ] **스피너 애니메이션** (WOW!)
  - [ ] 부드러운 회전
  - [ ] AI 응답 대기 중

#### 오후 (4시간)
- [ ] 연결 상태 애니메이션
  - [ ] 연결됨: 녹색 점 (고정)
  - [ ] 연결 중: 노란 점 (펄스 애니메이션)
  - [ ] 끊김: 빨간 점 (깜빡임)
- [ ] **토스트 알림** (WOW!)
  ```tsx
  <Toast>
    ✅ File saved successfully
  </Toast>
  ```
- [ ] 작업 완료 시 효과음 (optional)

**산출물**: ✅ 실시간 상태 표시 (WOW!)

---

### 📅 Day 13: 멀티모달 - 이미지 첨부 (WOW 요소 10 - 최고!)

#### 오전 (4시간)
- [ ] **드래그 앤 드롭** (WOW!)
  ```typescript
  onDrop={(e) => {
    const file = e.dataTransfer.files[0];
    if (file.type.startsWith('image/')) {
      handleImageUpload(file);
    }
  }}
  ```
- [ ] **이미지 미리보기** (WOW!)
  - [ ] Thumbnail 표시
  - [ ] 클릭 시 전체 크기 모달
- [ ] 클립보드에서 붙여넣기
  ```typescript
  onPaste={(e) => {
    const items = e.clipboardData.items;
    for (const item of items) {
      if (item.type.startsWith('image/')) {
        const file = item.getAsFile();
        handleImageUpload(file);
      }
    }
  }}
  ```

#### 오후 (4시간)
- [ ] 이미지를 Base64로 인코딩
- [ ] Backend에 전송 (JSON-RPC)
  ```typescript
  {
    method: 'turn/start',
    params: {
      message: "Implement this UI",
      attachments: [{
        type: 'image',
        data: base64String,
        mimeType: 'image/png'
      }]
    }
  }
  ```
- [ ] **메시지에 이미지 표시** (WOW!)
  ```tsx
  <MessageItem>
    <Image src={attachment.data} />
    <Text>Implement this UI</Text>
  </MessageItem>
  ```
- [ ] AI 응답에 이미지 참조 표시

**산출물**: ✅ 멀티모달 완성 (최고의 WOW!)

---

### 📅 Day 14: 버그 수정 & 최적화

#### 오전 (4시간)
- [ ] 메모리 누수 확인
  - [ ] Chrome DevTools Profiler
  - [ ] 오래된 메시지 정리 (1000개 이상 시)
- [ ] 에러 핸들링 강화
  - [ ] 모든 try-catch에 사용자 친화적 메시지
  - [ ] 에러 로깅 (파일로 저장)
- [ ] 성능 최적화
  - [ ] 메시지 목록 가상화 (react-window)
  - [ ] 이미지 lazy loading

#### 오후 (4시간)
- [ ] UI 폴리싱
  - [ ] 모든 애니메이션 부드럽게
  - [ ] 색상 일관성 확인
  - [ ] 간격/패딩 조정
- [ ] **로딩 시간 최적화**
  - [ ] 앱 시작 < 3초
  - [ ] Chunk splitting (Vite)
- [ ] 크로스 브라우저 테스트 (Electron 버전)

**산출물**: ✅ 버그 없는 안정적인 앱

---

### 📅 Day 15: 데모 준비 & 패키징

#### 오전 (4시간)
- [ ] 샘플 프로젝트 준비
  - [ ] 실제 코드베이스 또는 공개 OSS
  - [ ] RAG 인덱싱 완료
  - [ ] 데모 시나리오 검증
- [ ] 4가지 시나리오 리허설
  1. "Add authentication" (RAG 시연)
  2. "rm -rf /" 시도 (샌드박스 시연)
  3. "Add email validation" (컨텍스트 유지)
  4. UI 목업 이미지 → React 코드 (멀티모달)
- [ ] **스크린 레코딩** (백업용)

#### 오후 (4시간)
- [ ] macOS 패키징
  ```bash
  npm install -D electron-builder
  npm run build
  npm run package:mac
  ```
- [ ] DMG 파일 생성
  - [ ] 앱 아이콘 설정
  - [ ] 배경 이미지
- [ ] 코드 서명 (선택적)
- [ ] 설치 테스트
  - [ ] 클린 macOS에서 설치
  - [ ] 모든 기능 동작 확인
- [ ] 프레젠테이션 자료 준비
  - [ ] 스크린샷
  - [ ] 키 메시지 정리

**산출물**: ✅ 데모 준비 완료! 🚀

**Week 3 완료**: 완벽한 POC! 🎉

---

## 📊 3주 진행 추적

### Week 1 체크리스트

- [ ] Day 1: 프로젝트 셋업 ✅
- [ ] Day 2: 아름다운 레이아웃 ✅
- [ ] Day 3: Backend 통신 ✅
- [ ] Day 4: 채팅 + 타이핑 애니메이션 ✅
- [ ] Day 5: 마크다운 + 코드 하이라이팅 ✅

**목표**: 기본 채팅 가능한 아름다운 앱

### Week 2 체크리스트

- [ ] Day 6: RAG 인덱싱 ✅
- [ ] Day 7: RAG 검색 + 시각화 ✅
- [ ] Day 8: Diff 뷰어 ✅
- [ ] Day 9: 샌드박스 시각화 ✅
- [ ] Day 10: 파일 브라우저 ✅

**목표**: 모든 WOW 요소 (멀티모달 제외) 완성

### Week 3 체크리스트

- [ ] Day 11: 설정 패널 ✅
- [ ] Day 12: 진행 상황 표시 ✅
- [ ] Day 13: 멀티모달 (최고 WOW!) ✅
- [ ] Day 14: 버그 수정 ✅
- [ ] Day 15: 데모 준비 ✅

**목표**: 완벽한 데모 앱!

---

## 🎨 WOW 요소 완성도 체크

### 시각적 임팩트 (임원이 보는 것)

- [ ] ✨ **타이핑 애니메이션** - AI가 실시간으로 타이핑
- [ ] 📊 **Diff 뷰어** - 변경사항을 아름답게 비교
- [ ] 🧠 **RAG 시각화** - 관련 파일이 자동으로 찾아짐
- [ ] 🖼️ **멀티모달** - 이미지 → 코드 마법 ✨
- [ ] 🔒 **샌드박스 시각화** - 보안 상태가 눈에 보임
- [ ] 🎨 **아름다운 UI** - 다크 모드 + 부드러운 애니메이션
- [ ] 📁 **파일 브라우저** - VS Code 스타일 아이콘
- [ ] 📈 **진행 상황** - 실시간 프로그레스 바
- [ ] 🎉 **토스트 알림** - 작업 완료 피드백
- [ ] 🌊 **부드러운 전환** - 모든 화면 전환 애니메이션

**목표**: 10/10 완성! 🎯

---

## 🚀 3주 후 고도화 계획

> POC 성공 후 추가할 기능들 (Phase 2: 4-6주)

### 1️⃣ 대화 관리 고도화

**현재 (POC)**: 단일 대화만 지원

**고도화**:
- [ ] 여러 대화 생성/전환
  - [ ] 사이드바에 대화 목록
  - [ ] 대화 제목 자동 생성 (첫 메시지 기반)
  - [ ] 생성/수정 날짜 표시
  - [ ] 최신순 정렬
- [ ] 대화 삭제/아카이브
  - [ ] 휴지통 기능
  - [ ] 아카이브된 대화 탭
  - [ ] 영구 삭제 확인
- [ ] 대화 검색
  - [ ] 전체 대화 내용 검색
  - [ ] 정규식 지원
  - [ ] 검색 결과 하이라이팅
- [ ] 즐겨찾기
  - [ ] 별 아이콘으로 즐겨찾기
  - [ ] 즐겨찾기 필터

**예상 기간**: 1주

---

### 2️⃣ 고급 코드 편집

**현재 (POC)**: 읽기 전용 미리보기

**고도화**:
- [ ] Monaco Editor 전체 통합
  - [ ] 파일 직접 편집 가능
  - [ ] 자동 완성 (IntelliSense)
  - [ ] 린팅 표시
  - [ ] Git diff 마크
- [ ] Diff 편집 모드
  - [ ] 변경사항 부분 승인/거부
  - [ ] 인라인 수정 가능
  - [ ] 3-way merge (충돌 해결)
- [ ] 멀티 파일 편집
  - [ ] 탭으로 여러 파일 열기
  - [ ] 분할 화면 (Side-by-side)
- [ ] Git 통합
  - [ ] 변경사항 스테이징
  - [ ] 커밋 생성
  - [ ] 브랜치 전환

**예상 기간**: 2주

---

### 3️⃣ RAG 고급 기능

**현재 (POC)**: 간단한 벡터 검색

**고도화**:
- [ ] LanceDB로 마이그레이션
  - [ ] Rust 네이티브 통합
  - [ ] 영구 저장소
  - [ ] 빠른 성능
- [ ] 하이브리드 검색
  - [ ] 벡터 검색 + 키워드 검색 (BM25)
  - [ ] 재랭킹 (Reranking)
- [ ] 지능형 청킹
  - [ ] AST 기반 함수/클래스 단위 분리
  - [ ] Import 추적
  - [ ] 함수 호출 그래프
- [ ] 컨텍스트 확장
  - [ ] 관련 테스트 파일 자동 포함
  - [ ] 타입 정의 파일 추가
  - [ ] README/문서 통합
- [ ] 캐싱 최적화
  - [ ] 임베딩 캐싱
  - [ ] 검색 결과 캐싱
  - [ ] 증분 인덱싱 (변경된 파일만)

**예상 기간**: 2주

---

### 4️⃣ 파일 시스템 고급 기능

**현재 (POC)**: 기본 파일 브라우저

**고도화**:
- [ ] 퍼지 파일 검색
  - [ ] Cmd+P로 빠른 파일 열기
  - [ ] 타이핑하면 실시간 필터링
  - [ ] 최근 파일 우선 표시
- [ ] 파일 내용 검색
  - [ ] 정규식 지원
  - [ ] 전체 프로젝트 검색
  - [ ] 바꾸기 기능
- [ ] 파일 작업
  - [ ] 파일 생성/삭제/이름 변경
  - [ ] 폴더 생성
  - [ ] 드래그 앤 드롭으로 이동
- [ ] Git 상태 표시
  - [ ] 수정된 파일 (M)
  - [ ] 추가된 파일 (A)
  - [ ] 충돌 파일 (C)

**예상 기간**: 1주

---

### 5️⃣ 키보드 단축키 시스템

**현재 (POC)**: 기본 단축키만 (Enter, Esc, A, R)

**고도화**:
- [ ] 명령 팔레트 (Cmd+K)
  - [ ] 모든 액션 검색 가능
  - [ ] 최근 명령 이력
  - [ ] 키보드 네비게이션
- [ ] 전체 단축키 시스템
  - [ ] Cmd+N: 새 대화
  - [ ] Cmd+F: 검색
  - [ ] Cmd+B: 사이드바 토글
  - [ ] Cmd+Shift+P: 명령 팔레트
  - [ ] Cmd+1/2/3: 탭 전환
- [ ] 커스터마이징
  - [ ] 단축키 재할당
  - [ ] 충돌 감지
  - [ ] 기본값 복원
- [ ] Vim 모드 (선택적)
  - [ ] hjkl 네비게이션
  - [ ] 모달 편집

**예상 기간**: 1주

---

### 6️⃣ 협업 및 공유 기능

**현재 (POC)**: 로컬만

**고도화**:
- [ ] 대화 내보내기
  - [ ] JSON 형식
  - [ ] 마크다운 형식
  - [ ] HTML 형식 (스타일 포함)
- [ ] 코드 스니펫 공유
  - [ ] GitHub Gist 생성
  - [ ] 클립보드 복사 (포맷팅 유지)
- [ ] 템플릿 라이브러리
  - [ ] 자주 사용하는 프롬프트 저장
  - [ ] 변수 치환 (`{{project_name}}`)
  - [ ] 카테고리별 분류
  - [ ] 커뮤니티 템플릿 (선택적)
- [ ] 팀 설정 공유
  - [ ] .codex/config.json 버전 관리
  - [ ] 팀 전체 AGENTS.md

**예상 기간**: 1.5주

---

### 7️⃣ 플러그인 시스템

**현재 (POC)**: 없음

**고도화**:
- [ ] MCP 서버 UI 통합
  - [ ] MCP 서버 목록 표시
  - [ ] 서버별 커스텀 UI
  - [ ] 도구 목록 표시
- [ ] 커스텀 도구 UI
  - [ ] 도구별 승인 UI 커스터마이징
  - [ ] 파라미터 입력 폼
- [ ] 플러그인 마켓 (장기)
  - [ ] 커뮤니티 플러그인
  - [ ] 원클릭 설치

**예상 기간**: 2주

---

### 8️⃣ 성능 및 확장성

**현재 (POC)**: 기본 최적화만

**고도화**:
- [ ] 메시지 가상화
  - [ ] react-window로 10,000+ 메시지 처리
  - [ ] 오래된 메시지 페이징
- [ ] 이미지 최적화
  - [ ] WebP 변환
  - [ ] Lazy loading
  - [ ] Thumbnail 생성
- [ ] 백그라운드 작업
  - [ ] Web Worker로 임베딩 생성
  - [ ] 인덱싱 백그라운드 실행
- [ ] 데이터베이스 최적화
  - [ ] SQLite로 대화 저장 (JSON 대체)
  - [ ] 인덱싱
  - [ ] 쿼리 최적화

**예상 기간**: 1주

---

### 9️⃣ 접근성 및 국제화

**현재 (POC)**: 영어만, 기본 접근성

**고도화**:
- [ ] 완전한 ARIA 지원
  - [ ] 스크린 리더 테스트
  - [ ] 키보드 네비게이션 100%
  - [ ] Focus trap
- [ ] 국제화 (i18n)
  - [ ] react-i18next 통합
  - [ ] 한국어 번역
  - [ ] 일본어, 중국어 (간체)
  - [ ] 날짜/시간 로케일
- [ ] 고대비 모드
  - [ ] Windows High Contrast
  - [ ] macOS Increase Contrast

**예상 기간**: 1주

---

### 🔟 고급 AI 기능

**현재 (POC)**: 기본 대화만

**고도화**:
- [ ] 대화 요약
  - [ ] 자동 제목 생성
  - [ ] 긴 대화 요약
  - [ ] 핵심 내용 추출
- [ ] 관련 대화 추천
  - [ ] 유사한 과거 대화 표시
  - [ ] "이 대화도 참고하세요"
- [ ] 코드 리뷰 모드
  - [ ] PR diff 자동 분석
  - [ ] 개선 제안
  - [ ] 버그 탐지
- [ ] 통합 터미널 (장기)
  - [ ] 내장 터미널 에뮬레이터
  - [ ] 쉘 명령 결과 직접 표시
  - [ ] AI가 터미널 제어

**예상 기간**: 2주

---

## 📅 고도화 로드맵 (전체 10주)

### Phase 2a: 핵심 고도화 (4주)

**Week 4-5**:
- 대화 관리 고도화
- 파일 시스템 고급 기능
- 키보드 단축키 시스템

**Week 6-7**:
- 고급 코드 편집 (Monaco Editor)
- RAG 고급 기능 (LanceDB)

### Phase 2b: 확장 기능 (3주)

**Week 8**:
- 협업 및 공유 기능
- 성능 최적화

**Week 9-10**:
- 플러그인 시스템
- 접근성 및 국제화

### Phase 3: 프로덕션 준비 (3주)

**Week 11**:
- 크로스 플랫폼 패키징 (Windows, Linux)
- 자동 업데이트 시스템

**Week 12**:
- 종합 테스트
- 보안 감사

**Week 13**:
- 베타 릴리스
- 문서화

---

## 🎯 우선순위 가이드

### 즉시 고도화 (POC 직후)

1. **대화 관리** - 사용성의 핵심
2. **LanceDB** - RAG 성능 개선
3. **Monaco Editor** - 전문성 향상

### 중기 고도화 (2-3개월)

4. **키보드 단축키** - 생산성
5. **파일 검색** - 편의성
6. **협업 기능** - 팀 사용

### 장기 고도화 (6개월+)

7. **플러그인 시스템** - 확장성
8. **국제화** - 글로벌 진출
9. **고급 AI 기능** - 차별화

---

## ✅ 매일 체크리스트 템플릿

```markdown
### Day X: [작업명]

**시작 시간**: 09:00
**목표 완료 시간**: 18:00

#### 오전 체크리스트
- [ ] Task 1
- [ ] Task 2
- [ ] Task 3

**12:00 체크포인트**:
- 완료: X/3
- 블로커: [없음 / 있음 → 설명]
- 오후 계획 조정: [필요 없음 / 필요 → 설명]

#### 오후 체크리스트
- [ ] Task 4
- [ ] Task 5
- [ ] Task 6

**17:00 체크포인트**:
- 완료: X/6
- 내일 우선순위: [...]
- 리스크: [...]

**WOW 요소 체크**: 오늘 작업이 임원 데모에 미치는 임팩트은?
- [ ] 높음 - 데모에서 바로 보임
- [ ] 중간 - 간접적으로 영향
- [ ] 낮음 - 인프라 작업
```

---

## 🚀 시작 전 준비사항

### 개발 환경 셋업

```bash
# Node.js 버전 확인 (18 이상 필요)
node --version

# Rust 백엔드 빌드
cd codex-rs
cargo build --release

# 백엔드 실행 테스트
./target/release/codex-app-server --help
```

### 필요한 계정/API

- [ ] OpenAI API 키 (Embeddings + ChatGPT)
- [ ] GitHub 계정 (코드 관리)
- [ ] Apple Developer 계정 (코드 서명, 선택적)

### 도구 설치

- [ ] VS Code (또는 선호하는 IDE)
- [ ] Git
- [ ] Node.js 18+
- [ ] Rust toolchain
- [ ] Electron Fiddle (테스트용, 선택적)

---

**이제 Day 1을 시작할 준비가 되었습니다! 🎉**

**다음 단계**:
1. 의사결정 문서 업데이트
2. GitHub Issues 생성 (각 Day별)
3. Day 1 시작! 🚀
