# Day 5 TODO - 설정 및 Native 통합 (Electron)

> **목표**: 설정 관리, 인증, 테마 시스템, Native 통합 완성

## 전체 개요

Day 5는 앱 설정과 Native 기능을 완성합니다:
- 설정 관리 (electron-store)
- API 키 암호화 (safeStorage)
- Keychain 통합 (macOS)
- OAuth flow
- 테마 시스템 (nativeTheme)
- Native menus
- Global shortcuts

**Electron 특화:**
- safeStorage로 API 키 암호화
- Keychain/Credential Manager 통합
- nativeTheme API
- Native application menu
- Context menus
- System theme 감지

---

## Commit 25: 설정 관리 시스템

### 📋 작업 내용

1. **Settings 타입 정의**
2. **Settings Store 구현**
3. **Settings UI**
4. **electron-store 통합**

### 📁 파일 구조

```
src/renderer/types/
└── settings.ts           # Settings types

src/renderer/store/
└── useSettingsStore.ts   # Settings store

src/renderer/pages/
└── Settings.tsx          # Settings page

src/main/handlers/
└── settings.ts           # Settings IPC
```

### 1️⃣ Settings Types

**파일**: `src/renderer/types/settings.ts`

```typescript
export interface AppearanceSettings {
  theme: 'light' | 'dark' | 'system';
  accentColor?: string;
  fontSize: number;
  fontFamily: string;
  compactMode: boolean;
}

export interface ModelSettings {
  provider: 'anthropic' | 'openai';
  model: string;
  temperature: number;
  maxTokens: number;
  topP: number;
  presencePenalty: number;
  frequencyPenalty: number;
}

export interface PrivacySettings {
  telemetry: boolean;
  crashReports: boolean;
  saveHistory: boolean;
  clearHistoryOnExit: boolean;
}

export interface AdvancedSettings {
  mcpServers: MCPServerConfig[];
  sandboxMode: boolean;
  executionPolicy: 'always-ask' | 'auto-approve' | 'deny';
  debugMode: boolean;
  logLevel: 'debug' | 'info' | 'warn' | 'error';
}

export interface Settings {
  appearance: AppearanceSettings;
  model: ModelSettings;
  privacy: PrivacySettings;
  advanced: AdvancedSettings;
}
```

### 2️⃣ Settings Store

**파일**: `src/renderer/store/useSettingsStore.ts`

```typescript
import { create } from 'zustand';
import { devtools } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';

interface SettingsState {
  settings: Settings;
  isLoading: boolean;
}

interface SettingsActions {
  loadSettings: () => Promise<void>;
  updateSettings: (updates: Partial<Settings>) => Promise<void>;
  updateAppearance: (updates: Partial<AppearanceSettings>) => Promise<void>;
  updateModel: (updates: Partial<ModelSettings>) => Promise<void>;
  updatePrivacy: (updates: Partial<PrivacySettings>) => Promise<void>;
  updateAdvanced: (updates: Partial<AdvancedSettings>) => Promise<void>;
  resetSettings: () => Promise<void>;
}

export const useSettingsStore = create<SettingsState & SettingsActions>()(
  devtools(
    immer((set, get) => ({
      settings: getDefaultSettings(),
      isLoading: false,

      loadSettings: async () => {
        if (!window.electronAPI) return;

        set({ isLoading: true });
        try {
          const settings = await window.electronAPI.getSetting('appSettings');
          if (settings) {
            set({ settings });
          }
        } catch (error) {
          console.error('Failed to load settings:', error);
        } finally {
          set({ isLoading: false });
        }
      },

      updateSettings: async (updates) => {
        set((state) => {
          Object.assign(state.settings, updates);
        });

        if (window.electronAPI) {
          await window.electronAPI.setSetting('appSettings', get().settings);
        }
      },

      updateAppearance: async (updates) => {
        set((state) => {
          Object.assign(state.settings.appearance, updates);
        });

        // Apply theme immediately
        if (updates.theme && window.electronAPI) {
          await window.electronAPI.setTheme(updates.theme);
        }

        if (window.electronAPI) {
          await window.electronAPI.setSetting('appSettings', get().settings);
        }
      },

      updateModel: async (updates) => {
        set((state) => {
          Object.assign(state.settings.model, updates);
        });

        if (window.electronAPI) {
          await window.electronAPI.setSetting('appSettings', get().settings);
        }
      },

      updatePrivacy: async (updates) => {
        set((state) => {
          Object.assign(state.settings.privacy, updates);
        });

        if (window.electronAPI) {
          await window.electronAPI.setSetting('appSettings', get().settings);
        }
      },

      updateAdvanced: async (updates) => {
        set((state) => {
          Object.assign(state.settings.advanced, updates);
        });

        if (window.electronAPI) {
          await window.electronAPI.setSetting('appSettings', get().settings);
        }
      },

      resetSettings: async () => {
        set({ settings: getDefaultSettings() });

        if (window.electronAPI) {
          await window.electronAPI.resetSettings();
        }
      },
    }))
  )
);

function getDefaultSettings(): Settings {
  return {
    appearance: {
      theme: 'system',
      fontSize: 14,
      fontFamily: 'system-ui',
      compactMode: false,
    },
    model: {
      provider: 'anthropic',
      model: 'claude-3-5-sonnet-20241022',
      temperature: 0.7,
      maxTokens: 4096,
      topP: 1,
      presencePenalty: 0,
      frequencyPenalty: 0,
    },
    privacy: {
      telemetry: false,
      crashReports: true,
      saveHistory: true,
      clearHistoryOnExit: false,
    },
    advanced: {
      mcpServers: [],
      sandboxMode: true,
      executionPolicy: 'always-ask',
      debugMode: false,
      logLevel: 'info',
    },
  };
}
```

### ✅ 완료 기준

- [ ] Settings store 완성
- [ ] electron-store 통합
- [ ] 설정 UI 구현
- [ ] 테마 자동 적용

### 📝 Commit Message

```
feat(settings): implement comprehensive settings management

- Add Settings types and store
- Integrate electron-store for persistence
- Support appearance, model, privacy, advanced settings
- Auto-apply theme changes
- Add reset to defaults

Electron-specific:
- Persist settings via electron-store
- IPC for settings sync
```

---

## Commit 26: 인증 및 API 키 관리

### 📋 작업 내용

1. **API 키 암호화 (safeStorage)**
2. **Keychain 통합 (macOS)**
3. **OAuth flow**
4. **Credentials UI**

### 1️⃣ Secure Storage Handler

**파일**: `src/main/handlers/credentials.ts`

```typescript
import { ipcMain, safeStorage } from 'electron';
import keytar from 'keytar';

const SERVICE_NAME = 'Codex UI';

export function registerCredentialsHandlers() {
  // Save API key (encrypted)
  ipcMain.handle('credentials:setApiKey', async (_event, key: string) => {
    if (process.platform === 'darwin') {
      // Use Keychain on macOS
      await keytar.setPassword(SERVICE_NAME, 'api-key', key);
    } else {
      // Use safeStorage on other platforms
      const encrypted = safeStorage.encryptString(key);
      // Store encrypted buffer in electron-store
      const { store } = await import('./store');
      store.set('credentials.apiKey', encrypted.toString('base64'));
    }
  });

  // Get API key (decrypt)
  ipcMain.handle('credentials:getApiKey', async () => {
    if (process.platform === 'darwin') {
      const key = await keytar.getPassword(SERVICE_NAME, 'api-key');
      return key;
    } else {
      const { store } = await import('./store');
      const encrypted = store.get('credentials.apiKey') as string;
      if (!encrypted) return null;

      const buffer = Buffer.from(encrypted, 'base64');
      return safeStorage.decryptString(buffer);
    }
  });

  // Delete API key
  ipcMain.handle('credentials:deleteApiKey', async () => {
    if (process.platform === 'darwin') {
      await keytar.deletePassword(SERVICE_NAME, 'api-key');
    } else {
      const { store } = await import('./store');
      store.delete('credentials.apiKey');
    }
  });

  // OAuth flow
  ipcMain.handle('credentials:oauth', async (_event, provider: string) => {
    const { shell } = await import('electron');

    // Open OAuth URL in default browser
    const oauthUrl = getOAuthUrl(provider);
    await shell.openExternal(oauthUrl);

    // TODO: Set up local server to receive callback
    return null;
  });
}

function getOAuthUrl(provider: string): string {
  // Generate OAuth URL based on provider
  const redirectUri = 'codex://oauth/callback';
  const clientId = process.env.OAUTH_CLIENT_ID || '';

  if (provider === 'anthropic') {
    return `https://console.anthropic.com/oauth/authorize?client_id=${clientId}&redirect_uri=${redirectUri}`;
  }

  return '';
}
```

### 2️⃣ API Key UI

**파일**: `src/renderer/components/settings/ApiKeySection.tsx`

```typescript
import React, { useState, useEffect } from 'react';
import { Eye, EyeOff, Key } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { toast } from 'react-hot-toast';

export function ApiKeySection() {
  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [hasKey, setHasKey] = useState(false);

  useEffect(() => {
    loadApiKey();
  }, []);

  const loadApiKey = async () => {
    if (!window.electronAPI) return;

    const key = await window.electronAPI.getApiKey();
    if (key) {
      setApiKey(key);
      setHasKey(true);
    }
  };

  const handleSave = async () => {
    if (!window.electronAPI) return;

    try {
      await window.electronAPI.setApiKey(apiKey);
      setHasKey(true);
      toast.success('API key saved securely');
    } catch (error) {
      toast.error('Failed to save API key');
    }
  };

  const handleDelete = async () => {
    if (!window.electronAPI) return;

    const confirmed = confirm('Delete API key?');
    if (!confirmed) return;

    try {
      await window.electronAPI.deleteApiKey();
      setApiKey('');
      setHasKey(false);
      toast.success('API key deleted');
    } catch (error) {
      toast.error('Failed to delete API key');
    }
  };

  return (
    <div className="space-y-4">
      <div>
        <Label>Anthropic API Key</Label>
        <div className="flex gap-2 mt-2">
          <div className="relative flex-1">
            <Input
              type={showKey ? 'text' : 'password'}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="sk-ant-..."
              className="pr-10"
            />
            <Button
              variant="ghost"
              size="icon"
              className="absolute right-0 top-0 h-full"
              onClick={() => setShowKey(!showKey)}
            >
              {showKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
            </Button>
          </div>
          <Button onClick={handleSave}>Save</Button>
          {hasKey && (
            <Button variant="destructive" onClick={handleDelete}>
              Delete
            </Button>
          )}
        </div>
        <p className="text-xs text-muted-foreground mt-2">
          {process.platform === 'darwin'
            ? 'Stored securely in macOS Keychain'
            : 'Encrypted using system secure storage'}
        </p>
      </div>
    </div>
  );
}
```

### ✅ 완료 기준

- [ ] API 키 암호화 저장
- [ ] Keychain 통합 (macOS)
- [ ] safeStorage 사용 (Windows/Linux)
- [ ] OAuth flow 준비

### 📝 Commit Message

```
feat(auth): implement secure API key storage

- Add safeStorage encryption for API keys
- Integrate macOS Keychain via keytar
- Implement API key UI with show/hide
- Support OAuth flow preparation
- Secure credentials management

Electron-specific:
- safeStorage for encryption
- Keychain on macOS
- Credential Manager on Windows
```

---

## Commits 27-30: 모델 설정, 테마, 고급 설정, Native Menu

*Consolidated for brevity*

### 핵심 기능

**Commit 27: 모델 설정**
- 모델 파라미터 UI (temperature, max tokens, etc.)
- 프리셋 저장
- electron-store 영속화

**Commit 28: 테마 및 외관**
- nativeTheme API 활용
- System theme 자동 감지
- 다크/라이트 모드 전환
- Custom accent color

**Commit 29: 고급 설정**
- MCP 서버 설정
- 샌드박스 옵션
- 실행 정책 (always-ask, auto-approve, deny)
- 디버그 모드 (DevTools 토글)

**Commit 30: Native Menu**
- Application menu
- Context menus (right-click)
- Keyboard shortcuts
- Dynamic menu updates

### 핵심 코드 - nativeTheme

**파일**: `src/main/theme.ts`

```typescript
import { ipcMain, nativeTheme } from 'electron';

export function registerThemeHandlers() {
  // Set theme
  ipcMain.handle('theme:set', (_event, theme: 'light' | 'dark' | 'system') => {
    nativeTheme.themeSource = theme;
  });

  // Get current theme
  ipcMain.handle('theme:get', () => {
    return {
      source: nativeTheme.themeSource,
      shouldUseDarkColors: nativeTheme.shouldUseDarkColors,
    };
  });

  // Listen for system theme changes
  nativeTheme.on('updated', () => {
    // Notify renderer
    BrowserWindow.getAllWindows().forEach((window) => {
      window.webContents.send('theme:updated', {
        shouldUseDarkColors: nativeTheme.shouldUseDarkColors,
      });
    });
  });
}
```

### ✅ Day 5 완료 기준

- [ ] 설정 관리 시스템 완성
- [ ] API 키 안전하게 저장
- [ ] Keychain 통합 (macOS)
- [ ] 테마 시스템 작동
- [ ] System theme 자동 감지
- [ ] Native menu 완성
- [ ] Context menus 구현

### 📦 Dependencies

```json
{
  "dependencies": {
    "keytar": "^7.9.0"
  }
}
```

---

**다음**: Day 6에서는 키보드 단축키, 명령 팔레트, 성능 최적화, Native 통합을 구현합니다.
