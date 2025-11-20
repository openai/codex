# Day 5 TODO - 설정 및 커스터마이징

## 목표
사용자 설정 시스템을 구축하여 API 키, 모델 선택, 테마, 외관 등을 커스터마이징할 수 있도록 합니다.

---

## 1. 설정 관리 (Commit 25)

### 요구사항
- 설정 데이터 구조 정의
- Zustand 설정 스토어
- localStorage 영속화
- 기본값 관리

### 작업 내용

#### 설정 타입 정의
- [ ] `src/types/settings.ts` 생성
  ```typescript
  export interface AuthSettings {
    method: 'chatgpt' | 'api_key' | 'none';
    apiKey?: string;
    chatgptToken?: string;
  }

  export interface ModelSettings {
    provider: 'openai' | 'anthropic' | 'local' | 'ollama';
    model: string;
    temperature: number;
    maxTokens: number;
    topP: number;
    frequencyPenalty: number;
    presencePenalty: number;
  }

  export interface AppearanceSettings {
    theme: 'light' | 'dark' | 'system';
    accentColor: string;
    fontSize: 'small' | 'medium' | 'large';
    fontFamily: string;
    terminalTheme: 'dark' | 'light' | 'monokai' | 'solarized';
    compactMode: boolean;
    showLineNumbers: boolean;
  }

  export interface EditorSettings {
    tabSize: number;
    insertSpaces: boolean;
    wordWrap: 'on' | 'off' | 'bounded';
    minimap: boolean;
    autoSave: boolean;
    autoSaveDelay: number;
  }

  export interface AdvancedSettings {
    sandbox: {
      enabled: boolean;
      networkDisabled: boolean;
    };
    mcp: {
      enabled: boolean;
      servers: Array<{
        name: string;
        command: string;
        args: string[];
      }>;
    };
    executionPolicy: 'ask' | 'auto' | 'never';
    alwaysAllowedTools: string[];
    debugMode: boolean;
    telemetry: boolean;
  }

  export interface Settings {
    auth: AuthSettings;
    model: ModelSettings;
    appearance: AppearanceSettings;
    editor: EditorSettings;
    advanced: AdvancedSettings;
  }

  export const DEFAULT_SETTINGS: Settings = {
    auth: {
      method: 'none',
    },
    model: {
      provider: 'openai',
      model: 'gpt-4',
      temperature: 0.7,
      maxTokens: 4000,
      topP: 1,
      frequencyPenalty: 0,
      presencePenalty: 0,
    },
    appearance: {
      theme: 'system',
      accentColor: '#0ea5e9',
      fontSize: 'medium',
      fontFamily: 'Inter, system-ui, sans-serif',
      terminalTheme: 'dark',
      compactMode: false,
      showLineNumbers: true,
    },
    editor: {
      tabSize: 2,
      insertSpaces: true,
      wordWrap: 'on',
      minimap: true,
      autoSave: true,
      autoSaveDelay: 1000,
    },
    advanced: {
      sandbox: {
        enabled: true,
        networkDisabled: false,
      },
      mcp: {
        enabled: false,
        servers: [],
      },
      executionPolicy: 'ask',
      alwaysAllowedTools: [],
      debugMode: false,
      telemetry: true,
    },
  };
  ```

#### 설정 스토어 구현
- [ ] `src/store/settings-store.ts` 생성
  ```typescript
  import { create } from 'zustand';
  import { persist } from 'zustand/middleware';
  import { Settings, DEFAULT_SETTINGS } from '@/types/settings';
  import { merge } from 'lodash-es';

  interface SettingsState extends Settings {
    // Actions
    updateAuth: (auth: Partial<Settings['auth']>) => void;
    updateModel: (model: Partial<Settings['model']>) => void;
    updateAppearance: (appearance: Partial<Settings['appearance']>) => void;
    updateEditor: (editor: Partial<Settings['editor']>) => void;
    updateAdvanced: (advanced: Partial<Settings['advanced']>) => void;
    resetSettings: () => void;
    exportSettings: () => string;
    importSettings: (json: string) => void;
  }

  export const useSettingsStore = create<SettingsState>()(
    persist(
      (set, get) => ({
        ...DEFAULT_SETTINGS,

        updateAuth: (auth) => {
          set((state) => ({
            auth: { ...state.auth, ...auth },
          }));
        },

        updateModel: (model) => {
          set((state) => ({
            model: { ...state.model, ...model },
          }));
        },

        updateAppearance: (appearance) => {
          set((state) => ({
            appearance: { ...state.appearance, ...appearance },
          }));
        },

        updateEditor: (editor) => {
          set((state) => ({
            editor: { ...state.editor, ...editor },
          }));
        },

        updateAdvanced: (advanced) => {
          set((state) => ({
            advanced: merge({}, state.advanced, advanced),
          }));
        },

        resetSettings: () => {
          set(DEFAULT_SETTINGS);
        },

        exportSettings: () => {
          const state = get();
          const settings: Settings = {
            auth: state.auth,
            model: state.model,
            appearance: state.appearance,
            editor: state.editor,
            advanced: state.advanced,
          };
          return JSON.stringify(settings, null, 2);
        },

        importSettings: (json) => {
          try {
            const imported = JSON.parse(json) as Settings;
            set(merge({}, DEFAULT_SETTINGS, imported));
          } catch (error) {
            console.error('Failed to import settings', error);
            throw new Error('Invalid settings file');
          }
        },
      }),
      {
        name: 'codex-settings',
      }
    )
  );
  ```

#### lodash-es 설치
- [ ] 유틸리티 라이브러리 설치
  ```bash
  pnpm add lodash-es
  pnpm add -D @types/lodash-es
  ```

#### 테마 적용 훅
- [ ] `src/hooks/useTheme.ts` 생성
  ```typescript
  import { useEffect } from 'react';
  import { useSettingsStore } from '@/store/settings-store';

  export function useTheme() {
    const { theme, accentColor } = useSettingsStore((state) => state.appearance);

    useEffect(() => {
      const root = document.documentElement;

      // Apply theme
      if (theme === 'system') {
        const systemTheme = window.matchMedia('(prefers-color-scheme: dark)')
          .matches
          ? 'dark'
          : 'light';
        root.classList.toggle('dark', systemTheme === 'dark');
      } else {
        root.classList.toggle('dark', theme === 'dark');
      }

      // Apply accent color
      root.style.setProperty('--primary', accentColor);
    }, [theme, accentColor]);

    // Listen for system theme changes
    useEffect(() => {
      if (theme !== 'system') return;

      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      const handleChange = (e: MediaQueryListEvent) => {
        document.documentElement.classList.toggle('dark', e.matches);
      };

      mediaQuery.addEventListener('change', handleChange);
      return () => mediaQuery.removeEventListener('change', handleChange);
    }, [theme]);
  }
  ```

#### 설정 유효성 검증
- [ ] `src/lib/settings-validation.ts` 생성
  ```typescript
  import { Settings } from '@/types/settings';

  export interface ValidationError {
    field: string;
    message: string;
  }

  export function validateSettings(settings: Settings): ValidationError[] {
    const errors: ValidationError[] = [];

    // Validate auth
    if (settings.auth.method === 'api_key' && !settings.auth.apiKey) {
      errors.push({
        field: 'auth.apiKey',
        message: 'API key is required',
      });
    }

    // Validate model
    if (settings.model.temperature < 0 || settings.model.temperature > 2) {
      errors.push({
        field: 'model.temperature',
        message: 'Temperature must be between 0 and 2',
      });
    }

    if (settings.model.maxTokens < 1 || settings.model.maxTokens > 128000) {
      errors.push({
        field: 'model.maxTokens',
        message: 'Max tokens must be between 1 and 128000',
      });
    }

    if (settings.model.topP < 0 || settings.model.topP > 1) {
      errors.push({
        field: 'model.topP',
        message: 'Top P must be between 0 and 1',
      });
    }

    // Validate editor
    if (settings.editor.tabSize < 1 || settings.editor.tabSize > 8) {
      errors.push({
        field: 'editor.tabSize',
        message: 'Tab size must be between 1 and 8',
      });
    }

    return errors;
  }

  export function isValidSettings(settings: Settings): boolean {
    return validateSettings(settings).length === 0;
  }
  ```

### 예상 결과물
- 완전한 설정 타입 정의
- 영속화된 설정 스토어
- 테마 자동 적용
- 설정 검증 시스템

### Commit 메시지
```
feat(web-ui): implement settings management

- Define comprehensive settings types
- Create settings store with persistence
- Add default settings configuration
- Implement theme application hook
- Add settings validation utilities
- Install lodash-es for deep merge
```

---

## 2. 인증 설정 UI (Commit 26)

### 요구사항
- API 키 입력 필드
- ChatGPT 로그인 버튼
- 인증 상태 표시
- 로그아웃 기능

### 작업 내용

#### AuthSettings 컴포넌트
- [ ] `src/components/settings/AuthSettings.tsx` 생성
  ```typescript
  import { useState } from 'react';
  import { useSettingsStore } from '@/store/settings-store';
  import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
  import { Label } from '@/components/ui/label';
  import { Input } from '@/components/ui/input';
  import { Button } from '@/components/ui/button';
  import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group';
  import { Alert, AlertDescription } from '@/components/ui/alert';
  import { Eye, EyeOff, LogIn, LogOut, CheckCircle } from 'lucide-react';
  import { toast } from '@/lib/toast';

  export function AuthSettings() {
    const { auth, updateAuth } = useSettingsStore();
    const [showApiKey, setShowApiKey] = useState(false);
    const [apiKey, setApiKey] = useState(auth.apiKey || '');
    const [isAuthenticating, setIsAuthenticating] = useState(false);

    const handleMethodChange = (method: 'chatgpt' | 'api_key' | 'none') => {
      updateAuth({ method });
    };

    const handleSaveApiKey = () => {
      if (!apiKey.trim()) {
        toast.error('Please enter an API key');
        return;
      }

      updateAuth({ apiKey: apiKey.trim() });
      toast.success('API key saved');
    };

    const handleChatGPTLogin = async () => {
      setIsAuthenticating(true);
      try {
        // TODO: Implement ChatGPT OAuth flow
        await new Promise((resolve) => setTimeout(resolve, 1000));
        updateAuth({ chatgptToken: 'mock-token' });
        toast.success('Logged in with ChatGPT');
      } catch (error) {
        toast.error('Failed to log in');
      } finally {
        setIsAuthenticating(false);
      }
    };

    const handleLogout = () => {
      updateAuth({ chatgptToken: undefined, apiKey: undefined });
      setApiKey('');
      toast.success('Logged out');
    };

    const isAuthenticated =
      (auth.method === 'chatgpt' && auth.chatgptToken) ||
      (auth.method === 'api_key' && auth.apiKey);

    return (
      <div className="space-y-4">
        <Card>
          <CardHeader>
            <CardTitle>Authentication</CardTitle>
            <CardDescription>
              Choose how you want to authenticate with Codex
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <RadioGroup value={auth.method} onValueChange={handleMethodChange}>
              <div className="flex items-center space-x-2">
                <RadioGroupItem value="chatgpt" id="chatgpt" />
                <Label htmlFor="chatgpt" className="flex-1">
                  <div className="font-medium">ChatGPT Account</div>
                  <div className="text-sm text-muted-foreground">
                    Use your ChatGPT Plus, Pro, Team, or Enterprise plan
                  </div>
                </Label>
              </div>

              <div className="flex items-center space-x-2">
                <RadioGroupItem value="api_key" id="api_key" />
                <Label htmlFor="api_key" className="flex-1">
                  <div className="font-medium">API Key</div>
                  <div className="text-sm text-muted-foreground">
                    Use an OpenAI API key for usage-based billing
                  </div>
                </Label>
              </div>

              <div className="flex items-center space-x-2">
                <RadioGroupItem value="none" id="none" />
                <Label htmlFor="none" className="flex-1">
                  <div className="font-medium">None</div>
                  <div className="text-sm text-muted-foreground">
                    Don't authenticate (limited functionality)
                  </div>
                </Label>
              </div>
            </RadioGroup>

            {auth.method === 'chatgpt' && (
              <div className="space-y-2">
                {auth.chatgptToken ? (
                  <Alert>
                    <CheckCircle className="h-4 w-4" />
                    <AlertDescription className="flex items-center justify-between">
                      <span>You are logged in with ChatGPT</span>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={handleLogout}
                      >
                        <LogOut className="h-4 w-4 mr-2" />
                        Logout
                      </Button>
                    </AlertDescription>
                  </Alert>
                ) : (
                  <Button
                    onClick={handleChatGPTLogin}
                    disabled={isAuthenticating}
                    className="w-full"
                  >
                    <LogIn className="h-4 w-4 mr-2" />
                    {isAuthenticating ? 'Logging in...' : 'Login with ChatGPT'}
                  </Button>
                )}
              </div>
            )}

            {auth.method === 'api_key' && (
              <div className="space-y-2">
                <Label htmlFor="api-key">OpenAI API Key</Label>
                <div className="flex gap-2">
                  <div className="relative flex-1">
                    <Input
                      id="api-key"
                      type={showApiKey ? 'text' : 'password'}
                      value={apiKey}
                      onChange={(e) => setApiKey(e.target.value)}
                      placeholder="sk-..."
                    />
                    <Button
                      size="icon"
                      variant="ghost"
                      className="absolute right-0 top-0 h-full"
                      onClick={() => setShowApiKey(!showApiKey)}
                    >
                      {showApiKey ? (
                        <EyeOff className="h-4 w-4" />
                      ) : (
                        <Eye className="h-4 w-4" />
                      )}
                    </Button>
                  </div>
                  <Button onClick={handleSaveApiKey}>Save</Button>
                </div>
                <p className="text-xs text-muted-foreground">
                  Your API key is stored locally and never sent to any server
                  except OpenAI.
                </p>
              </div>
            )}
          </CardContent>
        </Card>

        {isAuthenticated && (
          <Alert>
            <CheckCircle className="h-4 w-4 text-green-500" />
            <AlertDescription>
              Authentication configured successfully
            </AlertDescription>
          </Alert>
        )}
      </div>
    );
  }
  ```

### 예상 결과물
- 인증 방법 선택
- API 키 입력 및 저장
- ChatGPT 로그인 UI
- 인증 상태 표시

### Commit 메시지
```
feat(web-ui): create authentication settings UI

- Build AuthSettings component
- Support ChatGPT and API key authentication
- Add API key input with show/hide toggle
- Implement login/logout functionality
- Show authentication status
```

---

## 3. 모델 설정 UI (Commit 27)

### 요구사항
- 모델 선택 드롭다운
- 모델 파라미터 조정
- 프리셋 저장/로드
- 실시간 미리보기

### 작업 내용

#### ModelSettings 컴포넌트
- [ ] `src/components/settings/ModelSettings.tsx` 생성
  ```typescript
  import { useSettingsStore } from '@/store/settings-store';
  import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
  import { Label } from '@/components/ui/label';
  import { Slider } from '@/components/ui/slider';
  import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
  } from '@/components/ui/select';
  import { Input } from '@/components/ui/input';

  const MODELS = {
    openai: [
      { id: 'gpt-4-turbo', name: 'GPT-4 Turbo', maxTokens: 128000 },
      { id: 'gpt-4', name: 'GPT-4', maxTokens: 8192 },
      { id: 'gpt-3.5-turbo', name: 'GPT-3.5 Turbo', maxTokens: 16385 },
    ],
    anthropic: [
      { id: 'claude-3-opus', name: 'Claude 3 Opus', maxTokens: 200000 },
      { id: 'claude-3-sonnet', name: 'Claude 3 Sonnet', maxTokens: 200000 },
      { id: 'claude-3-haiku', name: 'Claude 3 Haiku', maxTokens: 200000 },
    ],
  };

  export function ModelSettings() {
    const { model, updateModel } = useSettingsStore();

    const availableModels = MODELS[model.provider] || [];
    const selectedModel = availableModels.find((m) => m.id === model.model);

    return (
      <div className="space-y-4">
        <Card>
          <CardHeader>
            <CardTitle>Model Configuration</CardTitle>
            <CardDescription>
              Choose and configure the AI model
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="space-y-2">
              <Label>Provider</Label>
              <Select
                value={model.provider}
                onValueChange={(value: any) => {
                  updateModel({ provider: value });
                  // Reset model to first available
                  const newModels = MODELS[value as keyof typeof MODELS] || [];
                  if (newModels.length > 0) {
                    updateModel({ model: newModels[0].id });
                  }
                }}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="openai">OpenAI</SelectItem>
                  <SelectItem value="anthropic">Anthropic</SelectItem>
                  <SelectItem value="local">Local</SelectItem>
                  <SelectItem value="ollama">Ollama</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <Label>Model</Label>
              <Select
                value={model.model}
                onValueChange={(value) => updateModel({ model: value })}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {availableModels.map((m) => (
                    <SelectItem key={m.id} value={m.id}>
                      {m.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {selectedModel && (
                <p className="text-xs text-muted-foreground">
                  Max tokens: {selectedModel.maxTokens.toLocaleString()}
                </p>
              )}
            </div>

            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <Label>Temperature</Label>
                <span className="text-sm text-muted-foreground">
                  {model.temperature}
                </span>
              </div>
              <Slider
                value={[model.temperature]}
                onValueChange={([value]) => updateModel({ temperature: value })}
                min={0}
                max={2}
                step={0.1}
              />
              <p className="text-xs text-muted-foreground">
                Controls randomness. Higher values make output more random.
              </p>
            </div>

            <div className="space-y-2">
              <Label>Max Tokens</Label>
              <Input
                type="number"
                value={model.maxTokens}
                onChange={(e) =>
                  updateModel({ maxTokens: parseInt(e.target.value) })
                }
                min={1}
                max={selectedModel?.maxTokens || 128000}
              />
              <p className="text-xs text-muted-foreground">
                Maximum length of the response
              </p>
            </div>

            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <Label>Top P</Label>
                <span className="text-sm text-muted-foreground">
                  {model.topP}
                </span>
              </div>
              <Slider
                value={[model.topP]}
                onValueChange={([value]) => updateModel({ topP: value })}
                min={0}
                max={1}
                step={0.05}
              />
              <p className="text-xs text-muted-foreground">
                Nucleus sampling threshold
              </p>
            </div>

            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <Label>Frequency Penalty</Label>
                <span className="text-sm text-muted-foreground">
                  {model.frequencyPenalty}
                </span>
              </div>
              <Slider
                value={[model.frequencyPenalty]}
                onValueChange={([value]) =>
                  updateModel({ frequencyPenalty: value })
                }
                min={-2}
                max={2}
                step={0.1}
              />
            </div>

            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <Label>Presence Penalty</Label>
                <span className="text-sm text-muted-foreground">
                  {model.presencePenalty}
                </span>
              </div>
              <Slider
                value={[model.presencePenalty]}
                onValueChange={([value]) =>
                  updateModel({ presencePenalty: value })
                }
                min={-2}
                max={2}
                step={0.1}
              />
            </div>
          </CardContent>
        </Card>
      </div>
    );
  }
  ```

#### shadcn Slider와 Select 설치
- [ ] 필요한 컴포넌트 설치
  ```bash
  npx shadcn@latest add slider
  npx shadcn@latest add select
  ```

### 예상 결과물
- 모델 선택 UI
- 파라미터 슬라이더
- 실시간 값 표시
- 설명 텍스트

### Commit 메시지
```
feat(web-ui): add model configuration UI

- Create ModelSettings component
- Support provider and model selection
- Add sliders for temperature, top_p, penalties
- Include parameter descriptions
- Install slider and select components
```

---

## 4. 테마 및 외관 설정 (Commit 28)

### 요구사항
- 라이트/다크 모드 전환
- 터미널 색상 스키마
- 폰트 크기 조정
- 컴팩트 모드

### 작업 내용

#### AppearanceSettings 컴포넌트
- [ ] `src/components/settings/AppearanceSettings.tsx` 생성
  ```typescript
  import { useSettingsStore } from '@/store/settings-store';
  import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
  import { Label } from '@/components/ui/label';
  import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group';
  import { Switch } from '@/components/ui/switch';
  import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
  } from '@/components/ui/select';
  import { Sun, Moon, Monitor } from 'lucide-react';

  export function AppearanceSettings() {
    const { appearance, updateAppearance } = useSettingsStore();

    return (
      <div className="space-y-4">
        <Card>
          <CardHeader>
            <CardTitle>Theme</CardTitle>
            <CardDescription>
              Customize the appearance of the application
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="space-y-2">
              <Label>Color Mode</Label>
              <RadioGroup
                value={appearance.theme}
                onValueChange={(value: any) =>
                  updateAppearance({ theme: value })
                }
              >
                <div className="flex items-center space-x-2">
                  <RadioGroupItem value="light" id="light" />
                  <Label htmlFor="light" className="flex items-center gap-2 flex-1">
                    <Sun className="h-4 w-4" />
                    Light
                  </Label>
                </div>

                <div className="flex items-center space-x-2">
                  <RadioGroupItem value="dark" id="dark" />
                  <Label htmlFor="dark" className="flex items-center gap-2 flex-1">
                    <Moon className="h-4 w-4" />
                    Dark
                  </Label>
                </div>

                <div className="flex items-center space-x-2">
                  <RadioGroupItem value="system" id="system" />
                  <Label htmlFor="system" className="flex items-center gap-2 flex-1">
                    <Monitor className="h-4 w-4" />
                    System
                  </Label>
                </div>
              </RadioGroup>
            </div>

            <div className="space-y-2">
              <Label htmlFor="accent-color">Accent Color</Label>
              <div className="flex items-center gap-2">
                <input
                  id="accent-color"
                  type="color"
                  value={appearance.accentColor}
                  onChange={(e) =>
                    updateAppearance({ accentColor: e.target.value })
                  }
                  className="h-10 w-20 rounded border cursor-pointer"
                />
                <span className="text-sm font-mono">{appearance.accentColor}</span>
              </div>
            </div>

            <div className="space-y-2">
              <Label>Font Size</Label>
              <Select
                value={appearance.fontSize}
                onValueChange={(value: any) =>
                  updateAppearance({ fontSize: value })
                }
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="small">Small</SelectItem>
                  <SelectItem value="medium">Medium</SelectItem>
                  <SelectItem value="large">Large</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <Label>Terminal Theme</Label>
              <Select
                value={appearance.terminalTheme}
                onValueChange={(value: any) =>
                  updateAppearance({ terminalTheme: value })
                }
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="dark">Dark</SelectItem>
                  <SelectItem value="light">Light</SelectItem>
                  <SelectItem value="monokai">Monokai</SelectItem>
                  <SelectItem value="solarized">Solarized</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label>Compact Mode</Label>
                <p className="text-sm text-muted-foreground">
                  Reduce spacing and padding
                </p>
              </div>
              <Switch
                checked={appearance.compactMode}
                onCheckedChange={(checked) =>
                  updateAppearance({ compactMode: checked })
                }
              />
            </div>

            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label>Show Line Numbers</Label>
                <p className="text-sm text-muted-foreground">
                  Display line numbers in code blocks
                </p>
              </div>
              <Switch
                checked={appearance.showLineNumbers}
                onCheckedChange={(checked) =>
                  updateAppearance({ showLineNumbers: checked })
                }
              />
            </div>
          </CardContent>
        </Card>
      </div>
    );
  }
  ```

#### shadcn Switch 설치
- [ ] Switch 컴포넌트 설치
  ```bash
  npx shadcn@latest add switch
  ```

### 예상 결과물
- 테마 전환 UI
- 색상 피커
- 폰트 크기 선택
- 토글 스위치

### Commit 메시지
```
feat(web-ui): implement theme and appearance settings

- Create AppearanceSettings component
- Add theme selector (light/dark/system)
- Implement color picker for accent color
- Add font size and terminal theme options
- Support compact mode and line numbers toggle
- Install switch component
```

---

## 5. 고급 설정 (Commit 29)

### 요구사항
- MCP 서버 설정
- 샌드박스 옵션
- 실행 정책
- 디버그 모드

### 작업 내용

#### AdvancedSettings 컴포넌트
- [ ] `src/components/settings/AdvancedSettings.tsx` 생성
  ```typescript
  import { useState } from 'react';
  import { useSettingsStore } from '@/store/settings-store';
  import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
  import { Label } from '@/components/ui/label';
  import { Switch } from '@/components/ui/switch';
  import { Input } from '@/components/ui/input';
  import { Button } from '@/components/ui/button';
  import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group';
  import { Alert, AlertDescription } from '@/components/ui/alert';
  import { Plus, Trash2, AlertTriangle } from 'lucide-react';

  export function AdvancedSettings() {
    const { advanced, updateAdvanced } = useSettingsStore();
    const [newServerName, setNewServerName] = useState('');
    const [newServerCommand, setNewServerCommand] = useState('');

    const handleAddMCPServer = () => {
      if (!newServerName || !newServerCommand) return;

      const newServers = [
        ...advanced.mcp.servers,
        {
          name: newServerName,
          command: newServerCommand,
          args: [],
        },
      ];

      updateAdvanced({
        mcp: {
          ...advanced.mcp,
          servers: newServers,
        },
      });

      setNewServerName('');
      setNewServerCommand('');
    };

    const handleRemoveMCPServer = (index: number) => {
      const newServers = advanced.mcp.servers.filter((_, i) => i !== index);
      updateAdvanced({
        mcp: {
          ...advanced.mcp,
          servers: newServers,
        },
      });
    };

    return (
      <div className="space-y-4">
        <Alert variant="destructive">
          <AlertTriangle className="h-4 w-4" />
          <AlertDescription>
            Advanced settings can affect system security and stability. Change with
            caution.
          </AlertDescription>
        </Alert>

        <Card>
          <CardHeader>
            <CardTitle>Sandbox</CardTitle>
            <CardDescription>
              Control code execution sandboxing
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label>Enable Sandbox</Label>
                <p className="text-sm text-muted-foreground">
                  Run code in an isolated environment
                </p>
              </div>
              <Switch
                checked={advanced.sandbox.enabled}
                onCheckedChange={(checked) =>
                  updateAdvanced({
                    sandbox: { ...advanced.sandbox, enabled: checked },
                  })
                }
              />
            </div>

            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label>Disable Network</Label>
                <p className="text-sm text-muted-foreground">
                  Prevent network access from sandboxed code
                </p>
              </div>
              <Switch
                checked={advanced.sandbox.networkDisabled}
                onCheckedChange={(checked) =>
                  updateAdvanced({
                    sandbox: { ...advanced.sandbox, networkDisabled: checked },
                  })
                }
                disabled={!advanced.sandbox.enabled}
              />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Execution Policy</CardTitle>
            <CardDescription>
              Control how tools are executed
            </CardDescription>
          </CardHeader>
          <CardContent>
            <RadioGroup
              value={advanced.executionPolicy}
              onValueChange={(value: any) =>
                updateAdvanced({ executionPolicy: value })
              }
            >
              <div className="flex items-center space-x-2">
                <RadioGroupItem value="ask" id="ask" />
                <Label htmlFor="ask" className="flex-1">
                  <div className="font-medium">Ask</div>
                  <div className="text-sm text-muted-foreground">
                    Prompt for approval before executing tools
                  </div>
                </Label>
              </div>

              <div className="flex items-center space-x-2">
                <RadioGroupItem value="auto" id="auto" />
                <Label htmlFor="auto" className="flex-1">
                  <div className="font-medium">Auto</div>
                  <div className="text-sm text-muted-foreground">
                    Automatically execute all tools
                  </div>
                </Label>
              </div>

              <div className="flex items-center space-x-2">
                <RadioGroupItem value="never" id="never" />
                <Label htmlFor="never" className="flex-1">
                  <div className="font-medium">Never</div>
                  <div className="text-sm text-muted-foreground">
                    Never execute tools (read-only mode)
                  </div>
                </Label>
              </div>
            </RadioGroup>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>MCP Servers</CardTitle>
            <CardDescription>
              Configure Model Context Protocol servers
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label>Enable MCP</Label>
                <p className="text-sm text-muted-foreground">
                  Use Model Context Protocol servers
                </p>
              </div>
              <Switch
                checked={advanced.mcp.enabled}
                onCheckedChange={(checked) =>
                  updateAdvanced({
                    mcp: { ...advanced.mcp, enabled: checked },
                  })
                }
              />
            </div>

            {advanced.mcp.enabled && (
              <div className="space-y-3">
                <div className="space-y-2">
                  {advanced.mcp.servers.map((server, index) => (
                    <div
                      key={index}
                      className="flex items-center gap-2 p-2 rounded border"
                    >
                      <div className="flex-1">
                        <div className="font-medium text-sm">{server.name}</div>
                        <div className="text-xs text-muted-foreground font-mono">
                          {server.command}
                        </div>
                      </div>
                      <Button
                        size="icon"
                        variant="ghost"
                        onClick={() => handleRemoveMCPServer(index)}
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                  ))}
                </div>

                <div className="space-y-2">
                  <Input
                    placeholder="Server name"
                    value={newServerName}
                    onChange={(e) => setNewServerName(e.target.value)}
                  />
                  <Input
                    placeholder="Command"
                    value={newServerCommand}
                    onChange={(e) => setNewServerCommand(e.target.value)}
                  />
                  <Button onClick={handleAddMCPServer} className="w-full">
                    <Plus className="h-4 w-4 mr-2" />
                    Add Server
                  </Button>
                </div>
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Other</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label>Debug Mode</Label>
                <p className="text-sm text-muted-foreground">
                  Show detailed logs and debugging information
                </p>
              </div>
              <Switch
                checked={advanced.debugMode}
                onCheckedChange={(checked) =>
                  updateAdvanced({ debugMode: checked })
                }
              />
            </div>

            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label>Telemetry</Label>
                <p className="text-sm text-muted-foreground">
                  Help improve Codex by sending anonymous usage data
                </p>
              </div>
              <Switch
                checked={advanced.telemetry}
                onCheckedChange={(checked) =>
                  updateAdvanced({ telemetry: checked })
                }
              />
            </div>
          </CardContent>
        </Card>
      </div>
    );
  }
  ```

### 예상 결과물
- 샌드박스 토글
- 실행 정책 선택
- MCP 서버 관리
- 디버그 모드

### Commit 메시지
```
feat(web-ui): add advanced settings panel

- Create AdvancedSettings component
- Add sandbox configuration options
- Implement execution policy selection
- Support MCP server management
- Add debug mode and telemetry toggles
- Include warning for advanced settings
```

---

## 6. 설정 검증 및 백업 (Commit 30)

### 요구사항
- 설정 값 유효성 검사
- 잘못된 설정 경고
- 기본값으로 재설정
- 설정 내보내기/가져오기

### 작업 내용

#### SettingsPage 통합
- [ ] `src/pages/SettingsPage.tsx` 생성
  ```typescript
  import { useState } from 'react';
  import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
  import { Button } from '@/components/ui/button';
  import { AuthSettings } from '@/components/settings/AuthSettings';
  import { ModelSettings } from '@/components/settings/ModelSettings';
  import { AppearanceSettings } from '@/components/settings/AppearanceSettings';
  import { AdvancedSettings } from '@/components/settings/AdvancedSettings';
  import { SettingsBackup } from '@/components/settings/SettingsBackup';
  import { useSettingsStore } from '@/store/settings-store';
  import { validateSettings } from '@/lib/settings-validation';
  import { Alert, AlertDescription } from '@/components/ui/alert';
  import { AlertTriangle, RotateCcw } from 'lucide-react';
  import { toast } from '@/lib/toast';

  export function SettingsPage() {
    const { resetSettings, ...settings } = useSettingsStore();
    const [showResetDialog, setShowResetDialog] = useState(false);

    const errors = validateSettings(settings as any);

    const handleReset = () => {
      resetSettings();
      toast.success('Settings reset to defaults');
      setShowResetDialog(false);
    };

    return (
      <div className="container max-w-4xl mx-auto p-6">
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-3xl font-bold">Settings</h1>
            <p className="text-muted-foreground">
              Manage your application preferences
            </p>
          </div>

          <div className="flex gap-2">
            <SettingsBackup />
            <Button
              variant="outline"
              onClick={() => setShowResetDialog(true)}
            >
              <RotateCcw className="h-4 w-4 mr-2" />
              Reset
            </Button>
          </div>
        </div>

        {errors.length > 0 && (
          <Alert variant="destructive" className="mb-6">
            <AlertTriangle className="h-4 w-4" />
            <AlertDescription>
              <div className="font-medium mb-1">
                {errors.length} validation {errors.length === 1 ? 'error' : 'errors'}
              </div>
              <ul className="list-disc list-inside text-sm">
                {errors.map((error, i) => (
                  <li key={i}>{error.message}</li>
                ))}
              </ul>
            </AlertDescription>
          </Alert>
        )}

        <Tabs defaultValue="auth" className="space-y-4">
          <TabsList>
            <TabsTrigger value="auth">Authentication</TabsTrigger>
            <TabsTrigger value="model">Model</TabsTrigger>
            <TabsTrigger value="appearance">Appearance</TabsTrigger>
            <TabsTrigger value="advanced">Advanced</TabsTrigger>
          </TabsList>

          <TabsContent value="auth" className="space-y-4">
            <AuthSettings />
          </TabsContent>

          <TabsContent value="model" className="space-y-4">
            <ModelSettings />
          </TabsContent>

          <TabsContent value="appearance" className="space-y-4">
            <AppearanceSettings />
          </TabsContent>

          <TabsContent value="advanced" className="space-y-4">
            <AdvancedSettings />
          </TabsContent>
        </Tabs>

        {showResetDialog && (
          <AlertDialog open={showResetDialog} onOpenChange={setShowResetDialog}>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>Reset Settings?</AlertDialogTitle>
                <AlertDialogDescription>
                  This will reset all settings to their default values. This action
                  cannot be undone.
                </AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>Cancel</AlertDialogCancel>
                <AlertDialogAction onClick={handleReset}>
                  Reset
                </AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        )}
      </div>
    );
  }
  ```

#### SettingsBackup 컴포넌트
- [ ] `src/components/settings/SettingsBackup.tsx` 생성
  ```typescript
  import { useState } from 'react';
  import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
  } from '@/components/ui/dropdown-group';
  import { Button } from '@/components/ui/button';
  import { Input } from '@/components/ui/input';
  import { useSettingsStore } from '@/store/settings-store';
  import { Download, Upload } from 'lucide-react';
  import { toast } from '@/lib/toast';

  export function SettingsBackup() {
    const { exportSettings, importSettings } = useSettingsStore();

    const handleExport = () => {
      const json = exportSettings();
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `codex-settings-${Date.now()}.json`;
      a.click();
      URL.revokeObjectURL(url);
      toast.success('Settings exported');
    };

    const handleImport = async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;

      try {
        const text = await file.text();
        importSettings(text);
        toast.success('Settings imported');
      } catch (error) {
        toast.error('Failed to import settings');
      }
    };

    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline">Backup</Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem onClick={handleExport}>
            <Download className="h-4 w-4 mr-2" />
            Export Settings
          </DropdownMenuItem>
          <DropdownMenuItem asChild>
            <label className="cursor-pointer">
              <Upload className="h-4 w-4 mr-2" />
              Import Settings
              <Input
                type="file"
                accept=".json"
                onChange={handleImport}
                className="hidden"
              />
            </label>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    );
  }
  ```

#### shadcn Tabs 설치
- [ ] Tabs 컴포넌트 설치
  ```bash
  npx shadcn@latest add tabs
  ```

### 예상 결과물
- 설정 유효성 검증
- 에러 표시
- 설정 재설정
- 내보내기/가져오기

### Commit 메시지
```
feat(web-ui): validate and backup settings

- Create SettingsPage with tabs
- Implement settings validation
- Show validation errors
- Add reset to defaults functionality
- Build SettingsBackup component
- Support export/import settings
- Install tabs component
```

---

## Day 5 완료 체크리스트

- [ ] 설정 관리 (타입, 스토어, 테마 적용, 검증)
- [ ] 인증 설정 UI (API 키, ChatGPT 로그인)
- [ ] 모델 설정 UI (선택, 파라미터 조정)
- [ ] 테마 및 외관 설정 (다크 모드, 색상, 폰트)
- [ ] 고급 설정 (샌드박스, MCP, 실행 정책)
- [ ] 설정 검증 및 백업 (유효성 검사, 내보내기/가져오기)
- [ ] 모든 커밋 메시지 명확하게 작성
- [ ] 기능 테스트 및 검증

---

## 다음 단계 (Day 6 예고)

1. 키보드 단축키 시스템
2. 명령 팔레트
3. 성능 최적화
4. 로딩 상태 개선
5. 접근성 개선
6. 반응형 디자인

---

## 참고 자료

- [Zustand Persist Middleware](https://docs.pmnd.rs/zustand/integrations/persisting-store-data)
- [CSS Variables for Theming](https://developer.mozilla.org/en-US/docs/Web/CSS/Using_CSS_custom_properties)
- [lodash-es Documentation](https://lodash.com/docs/)

---

**Last Updated**: 2025-11-20
**Version**: 1.0
**Day**: 5 / 7
