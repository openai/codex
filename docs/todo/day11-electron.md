# Day 11 TODO - 플러그인 시스템 (Electron)

> **목표**: 확장 가능한 플러그인 아키텍처로 커뮤니티 생태계 구축

## 전체 개요

Day 11은 Codex UI에 플러그인 시스템을 추가합니다:
- 플러그인 API 및 Manifest
- Dynamic loading (ESM)
- 플러그인 마켓플레이스 UI
- 샘플 플러그인 (Theme, Tool, Data Source)
- 개발자 도구 (CLI, Debugger)
- 배포 시스템 (Registry, Auto-update)

**Electron 특화:**
- VM sandbox로 플러그인 격리
- Native module 로딩 지원
- electron-store로 플러그인 설정 저장
- Native notification으로 업데이트 알림
- Menu bar에 플러그인 메뉴 추가
- Code signing verification

---

## Commit 61: 플러그인 API 설계

### 📋 작업 내용

1. **Plugin Manifest 정의**
2. **Lifecycle hooks**
3. **API surface**
4. **Sandbox 환경**

### 📁 파일 구조

```
src/main/plugin/
├── PluginAPI.ts          # 플러그인 API
├── PluginContext.ts      # 플러그인 컨텍스트
└── types.ts              # 플러그인 타입

src/renderer/types/
└── plugin.ts             # 플러그인 인터페이스

plugins/
└── README.md             # 플러그인 개발 가이드
```

### 1️⃣ 플러그인 타입 정의

**파일**: `src/renderer/types/plugin.ts`

```typescript
export interface PluginManifest {
  id: string;
  name: string;
  version: string;
  description: string;
  author: {
    name: string;
    email?: string;
    url?: string;
  };
  repository?: {
    type: 'git';
    url: string;
  };
  license: string;
  main: string; // Entry point
  icon?: string;
  keywords?: string[];
  engines: {
    codex: string; // Semver range
  };
  dependencies?: Record<string, string>;
  activationEvents?: string[]; // When to activate
  contributes?: {
    commands?: CommandContribution[];
    themes?: ThemeContribution[];
    tools?: ToolContribution[];
    views?: ViewContribution[];
    settings?: SettingContribution[];
  };
  permissions?: PluginPermission[];
}

export interface CommandContribution {
  command: string;
  title: string;
  category?: string;
  icon?: string;
}

export interface ThemeContribution {
  id: string;
  label: string;
  uiTheme: 'vs' | 'vs-dark';
  path: string;
}

export interface ToolContribution {
  name: string;
  description: string;
  handler: string; // Function name in plugin
}

export interface ViewContribution {
  id: string;
  name: string;
  location: 'sidebar' | 'panel' | 'modal';
}

export interface SettingContribution {
  key: string;
  type: 'string' | 'number' | 'boolean' | 'object';
  default: any;
  description: string;
}

export type PluginPermission =
  | 'filesystem'
  | 'network'
  | 'clipboard'
  | 'notifications'
  | 'shell'
  | 'mcp';

export interface PluginContext {
  // Plugin info
  id: string;
  extensionPath: string;

  // API
  commands: {
    registerCommand: (command: string, handler: Function) => void;
    executeCommand: (command: string, ...args: any[]) => Promise<any>;
  };

  ui: {
    showMessage: (message: string, type?: 'info' | 'warning' | 'error') => void;
    showInputBox: (options: { prompt: string; placeholder?: string }) => Promise<string | undefined>;
    showQuickPick: (items: string[], options?: { placeHolder?: string }) => Promise<string | undefined>;
  };

  workspace: {
    getConfiguration: (section?: string) => any;
    updateConfiguration: (section: string, value: any) => Promise<void>;
  };

  storage: {
    get: <T>(key: string, defaultValue?: T) => T | undefined;
    set: (key: string, value: any) => Promise<void>;
    delete: (key: string) => Promise<void>;
  };

  // Event emitters
  onDidActivate: (callback: () => void) => void;
  onDidDeactivate: (callback: () => void) => void;
}

export interface Plugin {
  manifest: PluginManifest;
  activate: (context: PluginContext) => Promise<void> | void;
  deactivate?: () => Promise<void> | void;
}

export interface InstalledPlugin {
  manifest: PluginManifest;
  path: string;
  enabled: boolean;
  installedAt: number;
  updatedAt?: number;
}
```

### 2️⃣ Plugin API

**파일**: `src/main/plugin/PluginAPI.ts`

```typescript
import { app, dialog, shell } from 'electron';
import path from 'path';
import fs from 'fs/promises';
import type { PluginManifest, PluginContext, Plugin } from '@/renderer/types/plugin';

export class PluginAPI {
  private pluginsDir: string;

  constructor() {
    this.pluginsDir = path.join(app.getPath('userData'), 'plugins');
    this.ensurePluginsDir();
  }

  private async ensurePluginsDir() {
    try {
      await fs.mkdir(this.pluginsDir, { recursive: true });
    } catch (error) {
      console.error('Failed to create plugins directory:', error);
    }
  }

  async loadManifest(pluginPath: string): Promise<PluginManifest> {
    const manifestPath = path.join(pluginPath, 'package.json');
    const content = await fs.readFile(manifestPath, 'utf-8');
    return JSON.parse(content);
  }

  async loadPlugin(pluginPath: string): Promise<Plugin> {
    const manifest = await this.loadManifest(pluginPath);

    // Validate engines
    const codexVersion = app.getVersion();
    // TODO: Validate semver range

    // Load main file
    const mainPath = path.join(pluginPath, manifest.main);
    const pluginModule = await import(mainPath);

    return {
      manifest,
      activate: pluginModule.activate,
      deactivate: pluginModule.deactivate,
    };
  }

  createContext(manifest: PluginManifest, pluginPath: string): PluginContext {
    const context: PluginContext = {
      id: manifest.id,
      extensionPath: pluginPath,

      commands: {
        registerCommand: (command: string, handler: Function) => {
          // Register command globally
          console.log(`Registered command: ${command}`);
        },
        executeCommand: async (command: string, ...args: any[]) => {
          // Execute command
          return null;
        },
      },

      ui: {
        showMessage: (message: string, type = 'info') => {
          dialog.showMessageBox({
            type: type as any,
            message,
          });
        },
        showInputBox: async (options) => {
          // Show input dialog
          return undefined;
        },
        showQuickPick: async (items, options) => {
          // Show selection dialog
          return undefined;
        },
      },

      workspace: {
        getConfiguration: (section?: string) => {
          // Get configuration
          return {};
        },
        updateConfiguration: async (section: string, value: any) => {
          // Update configuration
        },
      },

      storage: {
        get: <T>(key: string, defaultValue?: T) => {
          // Get from plugin storage
          return defaultValue;
        },
        set: async (key: string, value: any) => {
          // Save to plugin storage
        },
        delete: async (key: string) => {
          // Delete from plugin storage
        },
      },

      onDidActivate: (callback: () => void) => {
        callback();
      },
      onDidDeactivate: (callback: () => void) => {
        // Store callback
      },
    };

    return context;
  }

  async installPlugin(pluginPackage: string): Promise<void> {
    // TODO: Download and extract plugin
    // For now, just copy from local path
    const pluginName = path.basename(pluginPackage);
    const targetPath = path.join(this.pluginsDir, pluginName);

    await fs.cp(pluginPackage, targetPath, { recursive: true });
  }

  async uninstallPlugin(pluginId: string): Promise<void> {
    const pluginPath = path.join(this.pluginsDir, pluginId);
    await fs.rm(pluginPath, { recursive: true, force: true });
  }

  async getInstalledPlugins(): Promise<string[]> {
    try {
      const entries = await fs.readdir(this.pluginsDir, { withFileTypes: true });
      return entries.filter((e) => e.isDirectory()).map((e) => e.name);
    } catch (error) {
      return [];
    }
  }

  getPluginPath(pluginId: string): string {
    return path.join(this.pluginsDir, pluginId);
  }
}
```

### 3️⃣ Plugin Manager

**파일**: `src/main/plugin/PluginManager.ts`

```typescript
import { PluginAPI } from './PluginAPI';
import type { Plugin, PluginManifest, InstalledPlugin } from '@/renderer/types/plugin';
import Store from 'electron-store';

const store = new Store();

export class PluginManager {
  private api: PluginAPI;
  private plugins: Map<string, Plugin> = new Map();
  private contexts: Map<string, any> = new Map();

  constructor() {
    this.api = new PluginAPI();
  }

  async initialize(): Promise<void> {
    const installedPlugins = await this.getInstalledPlugins();

    for (const pluginInfo of installedPlugins) {
      if (pluginInfo.enabled) {
        await this.activatePlugin(pluginInfo.manifest.id);
      }
    }
  }

  async activatePlugin(pluginId: string): Promise<void> {
    try {
      const pluginPath = this.api.getPluginPath(pluginId);
      const plugin = await this.api.loadPlugin(pluginPath);

      // Create context
      const context = this.api.createContext(plugin.manifest, pluginPath);

      // Activate plugin
      await plugin.activate(context);

      this.plugins.set(pluginId, plugin);
      this.contexts.set(pluginId, context);

      console.log(`Activated plugin: ${pluginId}`);
    } catch (error) {
      console.error(`Failed to activate plugin ${pluginId}:`, error);
      throw error;
    }
  }

  async deactivatePlugin(pluginId: string): Promise<void> {
    const plugin = this.plugins.get(pluginId);
    if (!plugin) return;

    if (plugin.deactivate) {
      await plugin.deactivate();
    }

    this.plugins.delete(pluginId);
    this.contexts.delete(pluginId);

    console.log(`Deactivated plugin: ${pluginId}`);
  }

  async installPlugin(pluginPackage: string): Promise<void> {
    await this.api.installPlugin(pluginPackage);

    // Load manifest
    const pluginName = require('path').basename(pluginPackage);
    const pluginPath = this.api.getPluginPath(pluginName);
    const manifest = await this.api.loadManifest(pluginPath);

    // Save to installed plugins
    const installedPlugins = await this.getInstalledPlugins();
    installedPlugins.push({
      manifest,
      path: pluginPath,
      enabled: true,
      installedAt: Date.now(),
    });

    this.saveInstalledPlugins(installedPlugins);

    // Activate
    await this.activatePlugin(manifest.id);
  }

  async uninstallPlugin(pluginId: string): Promise<void> {
    // Deactivate first
    await this.deactivatePlugin(pluginId);

    // Remove from disk
    await this.api.uninstallPlugin(pluginId);

    // Remove from installed list
    const installedPlugins = await this.getInstalledPlugins();
    const filtered = installedPlugins.filter((p) => p.manifest.id !== pluginId);
    this.saveInstalledPlugins(filtered);
  }

  async getInstalledPlugins(): Promise<InstalledPlugin[]> {
    return (store.get('installedPlugins') as InstalledPlugin[]) || [];
  }

  private saveInstalledPlugins(plugins: InstalledPlugin[]): void {
    store.set('installedPlugins', plugins);
  }

  getActivePlugins(): Plugin[] {
    return Array.from(this.plugins.values());
  }
}

export const pluginManager = new PluginManager();
```

### 4️⃣ 샘플 플러그인 구조

**파일**: `plugins/sample-theme/package.json`

```json
{
  "id": "sample-theme",
  "name": "Sample Theme",
  "version": "1.0.0",
  "description": "A sample theme plugin",
  "author": {
    "name": "Your Name"
  },
  "license": "MIT",
  "main": "dist/index.js",
  "engines": {
    "codex": "^1.0.0"
  },
  "contributes": {
    "themes": [
      {
        "id": "sample-dark",
        "label": "Sample Dark",
        "uiTheme": "vs-dark",
        "path": "./themes/dark.json"
      }
    ]
  }
}
```

**파일**: `plugins/sample-theme/src/index.ts`

```typescript
import type { PluginContext } from '@codex/plugin-api';

export async function activate(context: PluginContext) {
  console.log('Sample Theme activated');

  // Register a command
  context.commands.registerCommand('sampleTheme.hello', () => {
    context.ui.showMessage('Hello from Sample Theme!');
  });
}

export async function deactivate() {
  console.log('Sample Theme deactivated');
}
```

### ✅ 완료 기준

- [ ] Plugin API 설계 완료
- [ ] Manifest 스펙 정의
- [ ] Lifecycle hooks 구현
- [ ] Plugin context 제공
- [ ] 샘플 플러그인 작동

### 📝 Commit Message

```
feat(plugin): design plugin API and manifest system

- Define PluginManifest schema
- Create PluginAPI for loading/managing plugins
- Implement PluginContext with API surface
- Add PluginManager for lifecycle management
- Create sample theme plugin structure

API features:
- Commands registration
- UI interactions
- Workspace configuration
- Storage API
- Event hooks
```

---

## Commit 62: 플러그인 로더

### 📋 작업 내용

1. **Dynamic ESM loading**
2. **Dependency resolution**
3. **버전 호환성 체크**
4. **Hot reload**

### 핵심 코드

**파일**: `src/main/plugin/PluginLoader.ts`

```typescript
import { app } from 'electron';
import semver from 'semver';
import type { PluginManifest } from '@/renderer/types/plugin';

export class PluginLoader {
  private loadedModules: Map<string, any> = new Map();

  async validateCompatibility(manifest: PluginManifest): Promise<boolean> {
    const codexVersion = app.getVersion();
    const requiredVersion = manifest.engines.codex;

    if (!semver.satisfies(codexVersion, requiredVersion)) {
      throw new Error(
        `Plugin ${manifest.name} requires Codex ${requiredVersion}, but ${codexVersion} is installed`
      );
    }

    return true;
  }

  async loadModule(modulePath: string): Promise<any> {
    // Check cache
    if (this.loadedModules.has(modulePath)) {
      return this.loadedModules.get(modulePath);
    }

    // Dynamic import
    const module = await import(modulePath);

    // Cache
    this.loadedModules.set(modulePath, module);

    return module;
  }

  async reloadModule(modulePath: string): Promise<any> {
    // Clear cache
    this.loadedModules.delete(modulePath);

    // Clear require cache
    delete require.cache[require.resolve(modulePath)];

    // Reload
    return await this.loadModule(modulePath);
  }

  clearCache(): void {
    this.loadedModules.clear();
  }
}
```

### ✅ 완료 기준

- [ ] Dynamic loading 작동
- [ ] Dependency resolution
- [ ] Semver validation
- [ ] Hot reload 지원

### 📝 Commit Message

```
feat(plugin): implement plugin loader with hot reload

- Add dynamic ESM module loading
- Validate version compatibility with semver
- Implement module caching
- Support hot reload for development
- Clear require cache on reload
```

---

## Commits 63-66: UI, 샘플, 개발도구, 배포

*Remaining commits summarized*

### Commit 63: 플러그인 마켓플레이스 UI
- 플러그인 목록 (Grid/List 뷰)
- 검색 및 필터링
- 설치/제거/활성화
- 플러그인 상세 페이지

### Commit 64: 샘플 플러그인들
- Theme 플러그인
- Custom Tool 플러그인
- Data Source 플러그인 (GitHub, Notion)
- UI Extension 플러그인

### Commit 65: 플러그인 개발 도구
- Plugin CLI (`codex-plugin create`)
- TypeScript definitions
- 디버깅 도구
- 플러그인 테스트 러너

**Plugin CLI**:
```bash
# Create new plugin
codex-plugin create my-plugin --template=tool

# Build plugin
codex-plugin build

# Package for distribution
codex-plugin package

# Publish to registry
codex-plugin publish
```

### Commit 66: 배포 시스템
- 플러그인 레지스트리 (npm-like)
- 자동 업데이트 확인
- Code signing verification
- 리뷰 시스템

---

## 🎯 Day 11 완료 체크리스트

### 기능 완성도
- [ ] Plugin API 완성
- [ ] Dynamic loading
- [ ] Marketplace UI
- [ ] 샘플 플러그인 3개 이상
- [ ] Plugin CLI
- [ ] 배포 시스템

### Electron 통합
- [ ] VM sandbox
- [ ] Native module 지원
- [ ] electron-store 저장
- [ ] Code signing verification

---

## 📦 Dependencies

```json
{
  "dependencies": {
    "semver": "^7.5.4",
    "vm2": "^3.9.19"
  }
}
```

---

**다음**: Day 12에서는 실시간 협업 기능을 구현합니다.
