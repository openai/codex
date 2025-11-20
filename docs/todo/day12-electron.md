# Day 12 TODO - 협업 기능 (Electron)

> **목표**: WebRTC 기반 실시간 협업 및 세션 공유 시스템 구축

## 전체 개요

Day 12는 Codex UI에 실시간 협업 기능을 추가합니다:
- 세션 공유 (URL 기반)
- WebRTC P2P 연결
- 실시간 동기화 (Yjs CRDT)
- 협업 UI (참여자, 커서, 타이핑)
- 권한 관리 (읽기/쓰기)
- 충돌 해결 자동화
- 버전 히스토리

**Electron 특화:**
- Native WebRTC 지원
- Deep link로 세션 참여 (codex://join/...)
- System tray에 협업 상태 표시
- Native notification으로 참여자 알림
- 백그라운드에서 동기화 유지
- Share menu (macOS)

---

## Commit 67: 세션 공유

### 📋 작업 내용

1. **세션 내보내기 (URL)**
2. **읽기 전용 공유**
3. **만료 시간 설정**
4. **접근 권한 관리**

### 📁 파일 구조

```
src/main/collaboration/
├── ShareManager.ts       # 세션 공유 관리
└── types.ts              # 협업 타입

src/renderer/components/collaboration/
├── ShareDialog.tsx       # 공유 다이얼로그
└── ShareSettings.tsx     # 공유 설정

src/renderer/store/
└── useCollabStore.ts     # 협업 상태 관리
```

### 1️⃣ 협업 타입 정의

**파일**: `src/renderer/types/collaboration.ts`

```typescript
export interface ShareToken {
  id: string;
  sessionId: string;
  token: string;
  type: 'readonly' | 'edit';
  expiresAt?: number;
  createdAt: number;
  createdBy: string;
  maxUsers?: number;
  currentUsers: number;
}

export interface CollaborationSession {
  id: string;
  sessionId: string;
  participants: Participant[];
  status: 'active' | 'inactive';
  createdAt: number;
  lastActivityAt: number;
}

export interface Participant {
  id: string;
  name: string;
  color: string;
  role: 'owner' | 'editor' | 'viewer';
  cursor?: {
    x: number;
    y: number;
  };
  selection?: {
    messageId: string;
    start: number;
    end: number;
  };
  typing?: boolean;
  connectedAt: number;
  lastSeenAt: number;
}

export interface CollaborationEvent {
  type: 'join' | 'leave' | 'cursor' | 'typing' | 'edit';
  participantId: string;
  data?: any;
  timestamp: number;
}
```

### 2️⃣ Share Manager

**파일**: `src/main/collaboration/ShareManager.ts`

```typescript
import { nanoid } from 'nanoid';
import crypto from 'crypto';
import type { ShareToken } from '@/renderer/types/collaboration';
import Store from 'electron-store';

const store = new Store();

export class ShareManager {
  async createShareToken(
    sessionId: string,
    options: {
      type: 'readonly' | 'edit';
      expiresAt?: number;
      maxUsers?: number;
      createdBy: string;
    }
  ): Promise<ShareToken> {
    const token = crypto.randomBytes(32).toString('hex');

    const shareToken: ShareToken = {
      id: nanoid(),
      sessionId,
      token,
      type: options.type,
      expiresAt: options.expiresAt,
      createdAt: Date.now(),
      createdBy: options.createdBy,
      maxUsers: options.maxUsers,
      currentUsers: 0,
    };

    // Save to store
    const tokens = await this.getShareTokens();
    tokens.push(shareToken);
    this.saveShareTokens(tokens);

    return shareToken;
  }

  async validateToken(token: string): Promise<ShareToken | null> {
    const tokens = await this.getShareTokens();
    const shareToken = tokens.find((t) => t.token === token);

    if (!shareToken) return null;

    // Check expiration
    if (shareToken.expiresAt && shareToken.expiresAt < Date.now()) {
      return null;
    }

    // Check max users
    if (shareToken.maxUsers && shareToken.currentUsers >= shareToken.maxUsers) {
      return null;
    }

    return shareToken;
  }

  async revokeToken(tokenId: string): Promise<void> {
    const tokens = await this.getShareTokens();
    const filtered = tokens.filter((t) => t.id !== tokenId);
    this.saveShareTokens(filtered);
  }

  async incrementUserCount(token: string): Promise<void> {
    const tokens = await this.getShareTokens();
    const shareToken = tokens.find((t) => t.token === token);

    if (shareToken) {
      shareToken.currentUsers++;
      this.saveShareTokens(tokens);
    }
  }

  async decrementUserCount(token: string): Promise<void> {
    const tokens = await this.getShareTokens();
    const shareToken = tokens.find((t) => t.token === token);

    if (shareToken && shareToken.currentUsers > 0) {
      shareToken.currentUsers--;
      this.saveShareTokens(tokens);
    }
  }

  generateShareUrl(token: string): string {
    return `codex://join/${token}`;
  }

  private async getShareTokens(): Promise<ShareToken[]> {
    return (store.get('shareTokens') as ShareToken[]) || [];
  }

  private saveShareTokens(tokens: ShareToken[]): void {
    store.set('shareTokens', tokens);
  }
}

export const shareManager = new ShareManager();
```

### 3️⃣ Share Dialog

**파일**: `src/renderer/components/collaboration/ShareDialog.tsx`

```typescript
import React, { useState } from 'react';
import { Copy, Check, Clock, Users, Shield } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import { toast } from 'react-hot-toast';

interface ShareDialogProps {
  sessionId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function ShareDialog({ sessionId, open, onOpenChange }: ShareDialogProps) {
  const [shareUrl, setShareUrl] = useState('');
  const [copied, setCopied] = useState(false);
  const [accessType, setAccessType] = useState<'readonly' | 'edit'>('readonly');
  const [expiresIn, setExpiresIn] = useState('24h');
  const [maxUsers, setMaxUsers] = useState<number | undefined>();
  const [requirePassword, setRequirePassword] = useState(false);

  const handleCreateShare = async () => {
    if (!window.electronAPI) return;

    try {
      let expiresAt: number | undefined;
      if (expiresIn !== 'never') {
        const hours = parseInt(expiresIn);
        expiresAt = Date.now() + hours * 60 * 60 * 1000;
      }

      const token = await window.electronAPI.createShareToken(sessionId, {
        type: accessType,
        expiresAt,
        maxUsers,
        createdBy: 'current-user', // TODO: Get actual user
      });

      const url = await window.electronAPI.generateShareUrl(token.token);
      setShareUrl(url);

      toast.success('Share link created');
    } catch (error) {
      toast.error('Failed to create share link');
    }
  };

  const handleCopy = async () => {
    await navigator.clipboard.writeText(shareUrl);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
    toast.success('Copied to clipboard');
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Share Session</DialogTitle>
        </DialogHeader>

        <div className="space-y-4">
          {/* Access Type */}
          <div>
            <Label>Access Type</Label>
            <Select value={accessType} onValueChange={(v: any) => setAccessType(v)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="readonly">
                  <div className="flex items-center gap-2">
                    <Shield className="h-4 w-4" />
                    <span>View Only</span>
                  </div>
                </SelectItem>
                <SelectItem value="edit">
                  <div className="flex items-center gap-2">
                    <Users className="h-4 w-4" />
                    <span>Can Edit</span>
                  </div>
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* Expiration */}
          <div>
            <Label>Expires In</Label>
            <Select value={expiresIn} onValueChange={setExpiresIn}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="1">1 hour</SelectItem>
                <SelectItem value="24">24 hours</SelectItem>
                <SelectItem value="168">7 days</SelectItem>
                <SelectItem value="never">Never</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* Max Users */}
          <div>
            <Label>Max Users (optional)</Label>
            <Input
              type="number"
              value={maxUsers || ''}
              onChange={(e) => setMaxUsers(e.target.value ? parseInt(e.target.value) : undefined)}
              placeholder="Unlimited"
            />
          </div>

          {/* Password Protection */}
          <div className="flex items-center justify-between">
            <Label>Require Password</Label>
            <Switch checked={requirePassword} onCheckedChange={setRequirePassword} />
          </div>

          {/* Generate Button */}
          {!shareUrl && (
            <Button onClick={handleCreateShare} className="w-full">
              Generate Share Link
            </Button>
          )}

          {/* Share URL */}
          {shareUrl && (
            <div>
              <Label>Share URL</Label>
              <div className="flex gap-2 mt-2">
                <Input value={shareUrl} readOnly className="font-mono text-sm" />
                <Button variant="outline" size="icon" onClick={handleCopy}>
                  {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
                </Button>
              </div>
              <p className="text-xs text-muted-foreground mt-2">
                Anyone with this link can {accessType === 'readonly' ? 'view' : 'edit'} this
                session
                {expiresIn !== 'never' && ` for ${expiresIn} hours`}.
              </p>
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
```

### 4️⃣ Deep Link Handler

**파일**: `src/main/index.ts` (수정)

```typescript
import { app, BrowserWindow } from 'electron';

// Register protocol
if (process.defaultApp) {
  if (process.argv.length >= 2) {
    app.setAsDefaultProtocolClient('codex', process.execPath, [
      path.resolve(process.argv[1]),
    ]);
  }
} else {
  app.setAsDefaultProtocolClient('codex');
}

// Handle protocol
app.on('open-url', (event, url) => {
  event.preventDefault();

  // Parse codex://join/TOKEN
  const match = url.match(/^codex:\/\/join\/(.+)$/);
  if (match) {
    const token = match[1];

    // Send to renderer
    const mainWindow = BrowserWindow.getAllWindows()[0];
    if (mainWindow) {
      mainWindow.webContents.send('join-session', token);
    }
  }
});
```

### ✅ 완료 기준

- [ ] 세션 공유 URL 생성
- [ ] 읽기/쓰기 권한 설정
- [ ] 만료 시간 설정
- [ ] Deep link 처리
- [ ] Share dialog UI

### 📝 Commit Message

```
feat(collab): implement session sharing with deep links

- Create ShareManager for token management
- Generate secure share URLs
- Support readonly/edit access types
- Set expiration time and max users
- Handle deep links (codex://join/...)
- Add ShareDialog UI

Electron-specific:
- Register codex:// protocol
- Handle open-url events
- Native share menu (macOS)
```

---

## Commit 68: WebRTC 통합

### 📋 작업 내용

1. **SimplePeer 설정**
2. **P2P 연결 수립**
3. **시그널링 서버**
4. **데이터 채널**

### 1️⃣ WebRTC Client

**파일**: `src/renderer/services/webrtc.ts`

```typescript
import SimplePeer from 'simple-peer';
import type { Participant } from '@/types/collaboration';

export class WebRTCClient {
  private peer: SimplePeer.Instance | null = null;
  private signalingUrl = 'wss://signaling.codex.app'; // TODO: Configure

  async connect(isInitiator: boolean, participantId: string): Promise<void> {
    return new Promise((resolve, reject) => {
      this.peer = new SimplePeer({
        initiator: isInitiator,
        trickle: false,
      });

      this.peer.on('signal', (signal) => {
        // Send signal to other peer via signaling server
        this.sendSignal(participantId, signal);
      });

      this.peer.on('connect', () => {
        console.log('WebRTC connected');
        resolve();
      });

      this.peer.on('data', (data) => {
        this.handleData(data);
      });

      this.peer.on('error', (error) => {
        console.error('WebRTC error:', error);
        reject(error);
      });
    });
  }

  private async sendSignal(participantId: string, signal: any): Promise<void> {
    // Send via WebSocket signaling server
    const ws = new WebSocket(this.signalingUrl);

    ws.onopen = () => {
      ws.send(
        JSON.stringify({
          type: 'signal',
          to: participantId,
          signal,
        })
      );
    };
  }

  receiveSignal(signal: any): void {
    if (this.peer) {
      this.peer.signal(signal);
    }
  }

  send(data: any): void {
    if (this.peer && this.peer.connected) {
      this.peer.send(JSON.stringify(data));
    }
  }

  private handleData(data: Buffer): void {
    try {
      const message = JSON.parse(data.toString());
      // Handle collaboration events
      console.log('Received:', message);
    } catch (error) {
      console.error('Failed to parse WebRTC data:', error);
    }
  }

  disconnect(): void {
    if (this.peer) {
      this.peer.destroy();
      this.peer = null;
    }
  }
}
```

### ✅ 완료 기준

- [ ] WebRTC P2P 연결
- [ ] 시그널링 서버 통신
- [ ] 데이터 채널 송수신
- [ ] 연결 상태 관리

### 📝 Commit Message

```
feat(collab): integrate WebRTC for P2P connections

- Add SimplePeer for WebRTC
- Implement signaling protocol
- Create data channels
- Handle connection lifecycle
- Support multiple peers
```

---

## Commits 69-72: UI, 권한, CRDT, 히스토리

*Remaining commits summarized*

### Commit 69: 협업 UI
- 참여자 목록 (아바타, 색상)
- 실시간 커서 표시
- 타이핑 인디케이터
- 메시지 반응 (이모지)

**핵심 UI**:
```typescript
// Participant avatars with colored cursors
<div className="flex -space-x-2">
  {participants.map(p => (
    <Avatar key={p.id} style={{ borderColor: p.color }}>
      {p.name[0]}
    </Avatar>
  ))}
</div>

// Live cursor overlay
<div
  style={{
    position: 'absolute',
    left: cursor.x,
    top: cursor.y,
    borderColor: participant.color,
  }}
/>
```

### Commit 70: 권한 관리
- Role-based access (owner/editor/viewer)
- 편집 권한 확인
- 읽기 전용 모드
- 승인 워크플로우

### Commit 71: 충돌 해결 (Yjs)
- Yjs CRDT 통합
- 자동 병합
- Conflict-free 동기화
- Undo/Redo 스택 공유

**Yjs 통합**:
```typescript
import * as Y from 'yjs';
import { WebrtcProvider } from 'y-webrtc';

const ydoc = new Y.Doc();
const provider = new WebrtcProvider('codex-room-id', ydoc);

const ytext = ydoc.getText('messages');
ytext.observe((event) => {
  // Sync changes
});
```

### Commit 72: 히스토리 및 되돌리기
- 버전 히스토리
- 시간별 스냅샷
- Undo/Redo 스택
- Diff 뷰어

---

## 🎯 Day 12 완료 체크리스트

### 기능 완성도
- [ ] 세션 공유 URL
- [ ] WebRTC P2P 연결
- [ ] 협업 UI (참여자, 커서)
- [ ] 권한 관리
- [ ] CRDT 동기화
- [ ] 버전 히스토리

### Electron 통합
- [ ] Deep link 처리
- [ ] Native WebRTC
- [ ] System tray 상태
- [ ] Share menu (macOS)

---

## 📦 Dependencies

```json
{
  "dependencies": {
    "simple-peer": "^9.11.1",
    "yjs": "^13.6.10",
    "y-webrtc": "^10.2.5"
  }
}
```

---

**다음**: Day 13에서는 성능 모니터링 및 분석을 구현합니다.
