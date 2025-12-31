# tweakcc Modules Reference

> Complete Documentation of All 89 TypeScript Modules

## Module Overview

```
src/
├── index.tsx                       # CLI entry point
├── config.ts                       # Configuration management
├── types.ts                        # Type definitions
├── utils.ts                        # Utility functions
├── defaultSettings.ts              # Built-in defaults
├── startup.ts                      # Startup detection logic
├── migration.ts                    # Config migrations
├── installationDetection.ts        # Claude Code detection
├── installationPaths.ts            # Search path definitions
├── installationBackup.ts           # Backup/restore
├── nativeInstallation.ts           # Binary manipulation
├── nativeInstallationLoader.ts     # Dynamic loader
├── systemPromptSync.ts             # Prompt synchronization
├── systemPromptDownload.ts         # GitHub downloads
├── systemPromptHashIndex.ts        # Change tracking
├── patches/                        # 27 patch modules
│   ├── index.ts                    # Orchestrator
│   └── ...                         # Individual patches
├── ui/                             # UI layer
│   ├── App.tsx                     # Main application
│   ├── components/                 # 15 components
│   └── hooks/                      # Custom hooks
└── tests/                          # 8 test files
```

---

## Core Modules

### `index.tsx` - CLI Entry Point

**Purpose:** Main CLI entry point and orchestration.

**Location:** `src/index.tsx`

**Key Exports:**
```typescript
async function main(): Promise<void>
```

**Key Functions:**

| Function | Purpose |
|----------|---------|
| `main()` | CLI setup with Commander, parse options |
| `handleApplyMode()` | Non-interactive patch application |
| `handleInteractiveMode()` | Full interactive UI startup |
| `handleInstallationSelection()` | Multi-candidate picker UI |
| `startApp()` | Initialize React/Ink application |

**CLI Options:**
```typescript
program
  .option('-d, --debug', 'Enable debug mode')
  .option('-a, --apply', 'Apply saved customizations')
```

**Dependencies:**
- `commander` - CLI argument parsing
- `ink` - React renderer for terminal
- `./startup` - Startup checks
- `./config` - Configuration loading
- `./ui/App` - Main UI component

---

### `config.ts` - Configuration Management

**Purpose:** Read, write, and manage tweakcc configuration.

**Location:** `src/config.ts`

**Key Exports:**
```typescript
function getConfigDir(): string
async function readConfigFile(): Promise<TweakccConfig>
async function updateConfigFile(updateFn: (config: TweakccConfig) => void): Promise<TweakccConfig>
async function ensureConfigDir(): Promise<void>
function warnAboutMultipleConfigs(): void

const CONFIG_FILE: string
const CLIJS_BACKUP_FILE: string
const NATIVE_BINARY_BACKUP_FILE: string
const SYSTEM_PROMPTS_DIR: string
const PROMPT_CACHE_DIR: string
```

**Configuration Directory Resolution:**

| Priority | Path | Condition |
|----------|------|-----------|
| 1 | `$TWEAKCC_CONFIG_DIR` | Env var set and non-empty |
| 2 | `~/.tweakcc` | Directory exists (backward compat) |
| 3 | `~/.claude/tweakcc` | Claude ecosystem alignment |
| 4 | `$XDG_CONFIG_HOME/tweakcc` | XDG spec |
| 5 | `~/.tweakcc` | Default fallback |

**Key Operations:**

| Operation | Behavior |
|-----------|----------|
| `readConfigFile()` | Load config with migrations, merge defaults |
| `updateConfigFile()` | Callback-based update with persistence |
| `ensureConfigDir()` | Create directories and .gitignore |

**File Paths:**
```
CONFIG_FILE          = {configDir}/config.json
CLIJS_BACKUP_FILE    = {configDir}/cli.js.backup
NATIVE_BINARY_BACKUP = {configDir}/native-binary.backup
SYSTEM_PROMPTS_DIR   = {configDir}/system-prompts/
PROMPT_CACHE_DIR     = {configDir}/prompt-data-cache/
```

---

### `types.ts` - Type Definitions

**Purpose:** Core TypeScript interfaces and types.

**Location:** `src/types.ts`

**Key Types:**

#### Configuration Types

```typescript
interface TweakccConfig {
  version: string
  lastModified: string
  changesApplied: boolean
  ccInstallationPath?: string
  themeId: string
  themes: Theme[]
  thinkingVerbs: ThinkingVerbsConfig
  thinkingStyle: ThinkingStyleConfig
  userMessageDisplay: UserMessageDisplayConfig
  inputBox: InputBoxConfig
  misc: MiscConfig
  toolsets: Toolset[]
  selectedToolset?: string
}

interface Theme {
  name: string
  id: string
  colors: ThemeColors
}

interface ThemeColors {
  autoAccept: string
  bashBorder: string
  claude: string
  claudeShimmer: string
  // ... 62+ color properties
}
```

#### Thinking Configuration

```typescript
interface ThinkingVerbsConfig {
  format: string              // e.g., "{}… "
  verbs: string[]            // e.g., ["Thinking", "Pondering", ...]
}

interface ThinkingStyleConfig {
  reverseMirror: boolean     // Reverse animation direction
  updateInterval: number     // Milliseconds between phases
  phases: string[]           // Animation characters
}
```

#### User Message Display

```typescript
interface UserMessageDisplayConfig {
  format: string             // e.g., " > {} "
  styling: string[]          // 'bold', 'italic', etc.
  foregroundColor: string | 'default'
  backgroundColor: string | 'default' | null
  borderStyle: BorderStyle
  borderColor: string
  paddingX: number
  paddingY: number
  fitBoxToContent: boolean
}

type BorderStyle =
  | 'none' | 'single' | 'double'
  | 'round' | 'bold' | 'singleDouble'
  | 'doubleSingle' | 'classic' | 'arrow'
```

#### Installation Types

```typescript
type InstallationKind = 'npm-based' | 'native-binary'
type InstallationSource = 'env-var' | 'config' | 'path' | 'search-paths'

interface InstallationCandidate {
  path: string
  kind: InstallationKind
  version: string
}

interface ClaudeCodeInstallationInfo {
  cliPath?: string
  version: string
  nativeInstallationPath?: string
  source: InstallationSource
}
```

#### Toolsets

```typescript
interface Toolset {
  name: string
  allowedTools: string[] | '*'
}
```

#### Misc Configuration

```typescript
interface MiscConfig {
  showVersion: boolean
  showPatchesApplied: boolean
  expandThinkingBlocks: boolean
  enableConversationTitle: boolean
  hideStartupBanner: boolean
  hideCtrlGToEditPrompt: boolean
  hideStartupClawd: boolean
  increaseFileReadLimit: boolean
}
```

---

### `utils.ts` - Utility Functions

**Purpose:** Shared utility functions used across modules.

**Location:** `src/utils.ts`

**Color Utilities:**

```typescript
function isValidColorFormat(color: string): boolean
// Validates hex (#fff, #ffffff), rgb(), hsl() formats

function normalizeColorToRgb(color: string): string
// Converts hex/hsl to rgb(r,g,b) format

function getColorKeys(theme: Theme): string[]
// Returns all color property names from theme
```

**File Operations:**

```typescript
async function replaceFileBreakingHardLinks(
  filePath: string,
  newContent: string | Buffer,
  operation?: 'text' | 'binary'
): Promise<void>
// Critical for Bun-installed packages
// 1. Read original permissions
// 2. Unlink file (break hard links)
// 3. Write new content
// 4. Restore permissions

async function doesFileExist(filePath: string): Promise<boolean>
// Error-safe file existence check

async function hashFileInChunks(
  filePath: string,
  algorithm: string,
  chunkSize: number
): Promise<string>
// Streaming hash for large files (SHA256/MD5)
```

**System Integration:**

```typescript
function openInExplorer(filePath: string): void
// Opens file browser (Windows/macOS/Linux)

function revealFileInExplorer(filePath: string): void
// Reveals file in file manager

function getCurrentClaudeCodeTheme(): string | null
// Reads ~/.claude.json

function getClaudeSubscriptionType(): string | null
// Reads ~/.claude/.credentials.json

function getSelectedModel(): string | null
// Reads ~/.claude/settings.json
```

**Version Management:**

```typescript
function compareSemverVersions(a: string, b: string): number
// Returns: positive (a > b), negative (a < b), 0 (equal)
```

**Debug Utilities:**

```typescript
function enableDebug(): void
function debug(message: string, ...params: any[]): void
function buildChalkChain(...styles: string[]): chalk.Chalk
function expandTilde(filepath: string): string
```

---

### `defaultSettings.ts` - Built-in Defaults

**Purpose:** Default themes, verbs, and settings.

**Location:** `src/defaultSettings.ts`

**Built-in Themes:**

| ID | Name | Description |
|----|------|-------------|
| `dark` | Dark mode | RGB colors for dark terminals |
| `light` | Light mode | RGB colors for light terminals |
| `light-ansi` | Light mode (ANSI only) | 16-color ANSI support |
| `dark-ansi` | Dark mode (ANSI only) | 16-color ANSI support |
| `light-daltonized` | Light (colorblind-friendly) | Daltonized colors |
| `dark-daltonized` | Dark (colorblind-friendly) | Daltonized colors |
| `monochrome` | Monochrome | Accessibility theme |

**Default Verbs (100+):**
```typescript
const defaultVerbs = [
  "Actualizing", "Baking", "Computing", "Decoding",
  "Elevating", "Formulating", "Generating", "Hypothesizing",
  "Innovating", "Journeying", "Kindling", "Leveraging",
  "Mapping", "Navigating", "Optimizing", "Pondering",
  "Questioning", "Razzle-dazzling", "Synthesizing", "Thinking",
  // ... 80+ more
]
```

**Default Thinking Style:**
```typescript
const defaultThinkingStyle = {
  reverseMirror: true,
  updateInterval: 120,
  phases: ["·", "✢", "*", "✦"]  // Platform-specific
}
```

**Default User Message Display:**
```typescript
const defaultUserMessageDisplay = {
  format: " > {} ",
  styling: [],
  foregroundColor: "default",
  backgroundColor: null,
  borderStyle: "none",
  borderColor: "rgb(255,255,255)",
  paddingX: 0,
  paddingY: 0,
  fitBoxToContent: false
}
```

---

### `startup.ts` - Startup Sequence

**Purpose:** Initialize tweakcc and detect Claude Code installation.

**Location:** `src/startup.ts`

**Key Exports:**

```typescript
interface StartupCheckResult {
  info?: StartupCheckInfo
  candidates?: InstallationCandidate[]
  error?: Error
}

interface StartupCheckInfo {
  wasUpdated: boolean
  oldVersion: string | null
  newVersion: string | null
  ccInstInfo: ClaudeCodeInstallationInfo
}

async function startupCheck(options: StartupOptions): Promise<StartupCheckResult>
async function completeStartupCheck(
  config: TweakccConfig,
  ccInstInfo: ClaudeCodeInstallationInfo
): Promise<StartupCheckInfo | null>
```

**Startup Sequence:**

1. Find Claude Code installation
2. Check for pending installation selection
3. Sync system prompts (async, non-blocking)
4. Create backups if missing
5. Detect version changes
6. Return startup info or candidates for picker

---

### `migration.ts` - Configuration Migrations

**Purpose:** Migrate old configuration formats to current schema.

**Location:** `src/migration.ts`

**Key Exports:**

```typescript
function migrateUserMessageDisplayToV320(config: TweakccConfig): void
// v3.2.0: Restructured userMessageDisplay format

function migrateConfigIfNeeded(): boolean
// Converts ccInstallationDir → ccInstallationPath
```

**Migration Examples:**

```typescript
// Old format (pre-v3.2.0)
userMessageDisplay: {
  prefix: { format: '$', styling: ['bold'], foregroundColor: 'rgb(255,0,0)' },
  message: { format: '{}', styling: ['italic'], foregroundColor: 'rgb(0,255,0)' }
}

// New format (v3.2.0+)
userMessageDisplay: {
  format: '${}',
  styling: ['bold', 'italic'],
  foregroundColor: 'rgb(0,255,0)',
  backgroundColor: null,
  borderStyle: 'none',
  borderColor: 'rgb(255,255,255)',
  paddingX: 0,
  paddingY: 0,
  fitBoxToContent: false
}
```

---

## Installation Modules

### `installationDetection.ts` - Claude Code Detection

**Purpose:** Find Claude Code installations on the system.

**Location:** `src/installationDetection.ts`

**Key Exports:**

```typescript
async function findClaudeCodeInstallation(
  config: TweakccConfig,
  options: FindInstallationOptions
): Promise<ClaudeCodeInstallationInfo>

async function collectCandidates(): Promise<InstallationCandidate[]>

class InstallationDetectionError extends Error
```

**Detection Priority:**

| Priority | Source | Method |
|----------|--------|--------|
| 1 | Environment | `TWEAKCC_CC_INSTALLATION_PATH` |
| 2 | Config | `ccInstallationPath` field |
| 3 | PATH | `which claude` command |
| 4 | Search | 40+ hardcoded paths |

**Installation Type Detection:**

```typescript
async function resolvePathToInstallationType(filePath: string): Promise<{
  kind: InstallationKind
  resolvedPath: string
} | null>
// Uses WASMagic to detect file type (JS vs binary)
```

**Version Extraction:**

```typescript
async function extractVersion(
  filePath: string,
  kind: InstallationKind
): Promise<string>
// 1. Check filename pattern (e.g., versions/2.0.65)
// 2. Search for VERSION:"X.Y.Z" in content
// 3. Extract from native binary
```

---

### `installationPaths.ts` - Search Paths

**Purpose:** Define all possible Claude Code installation locations.

**Location:** `src/installationPaths.ts`

**Key Exports:**

```typescript
function getSearchPaths(): string[]
```

**Search Path Categories:**

| Category | Example Paths |
|----------|---------------|
| Claude Local | `~/.claude/local/node_modules/@anthropic-ai/claude-code` |
| NPM Global | `/usr/local/lib/node_modules/@anthropic-ai/claude-code` |
| Yarn Global | `~/.config/yarn/global/node_modules/...` |
| pnpm | `~/.local/share/pnpm/global/...` |
| Bun | `~/.bun/install/global/node_modules/...` |
| Homebrew | `/opt/homebrew/lib/node_modules/...` |
| nvm | `~/.nvm/versions/node/*/lib/node_modules/...` |
| fnm | `~/.local/share/fnm/node-versions/*/...` |
| volta | `~/.volta/tools/image/node/*/lib/...` |
| asdf | `~/.asdf/installs/nodejs/*/lib/...` |
| mise | `~/.local/share/mise/installs/node/*/...` |
| Native | `~/.local/bin/claude`, `~/.local/share/claude/versions/*` |

**Platform-Specific Paths:**

- **Windows:** AppData/Roaming, AppData/Local, nvm4w
- **macOS:** /opt/homebrew, Library paths
- **Linux:** /usr/local, /opt, XDG paths

---

### `installationBackup.ts` - Backup Management

**Purpose:** Create and restore installation backups.

**Location:** `src/installationBackup.ts`

**Key Exports:**

```typescript
async function backupClijs(ccInstInfo: ClaudeCodeInstallationInfo): Promise<void>
async function backupNativeBinary(ccInstInfo: ClaudeCodeInstallationInfo): Promise<void>
async function restoreClijsFromBackup(ccInstInfo: ClaudeCodeInstallationInfo): Promise<boolean>
async function restoreNativeBinaryFromBackup(ccInstInfo: ClaudeCodeInstallationInfo): Promise<boolean>
```

**Backup Strategy:**

| Type | Source | Backup Location |
|------|--------|-----------------|
| npm-based | `cli.js` | `~/.tweakcc/cli.js.backup` |
| native-binary | Executable | `~/.tweakcc/native-binary.backup` |

**Restore Operations:**

1. Read backup content
2. Use `replaceFileBreakingHardLinks()` for safe writing
3. Clear applied system prompt hashes
4. Set `changesApplied = false`

---

## Native Binary Support

### `nativeInstallation.ts` - Binary Manipulation

**Purpose:** Extract and repack native Claude Code binaries.

**Location:** `src/nativeInstallation.ts`

**Key Exports:**

```typescript
async function extractClaudeJsFromBinary(binaryPath: string): Promise<Buffer>
async function repackNativeBinary(
  binaryPath: string,
  modifiedJs: Buffer,
  outputPath: string
): Promise<void>
```

**Implementation:**

- Uses `node-lief` library for Mach-O/ELF manipulation
- Finds embedded `claude.js` in binary
- Patches extracted JavaScript
- Repacks binary with modified JavaScript
- Handles ARM64/x86 alignment

---

### `nativeInstallationLoader.ts` - Dynamic Loader

**Purpose:** Safely load native binary support with fallback.

**Location:** `src/nativeInstallationLoader.ts`

**Key Exports:**

```typescript
async function extractClaudeJsFromNativeInstallation(
  binaryPath: string
): Promise<Buffer | null>

async function repackNativeInstallation(
  binPath: string,
  modifiedClaudeJs: Buffer,
  outputPath: string
): Promise<void>
```

**Graceful Degradation:**

```typescript
// Dynamically load node-lief
try {
  const nodeLief = await import('node-lief');
  // Use native support
} catch {
  // Return null - native support disabled
  return null;
}
```

---

## System Prompt Modules

### `systemPromptSync.ts` - Prompt Synchronization

**Purpose:** Sync, parse, and manage system prompts.

**Location:** `src/systemPromptSync.ts`

**Key Exports:**

```typescript
interface StringsPrompt {
  name: string
  id: string
  description: string
  pieces: string[]
  identifiers: number[]
  identifierMap: Record<string, string>
  version: string
}

interface MarkdownPrompt {
  name: string
  description: string
  ccVersion: string
  variables: string[]
  content: string
  contentLineOffset: number
}

interface SyncResult {
  id: string
  name: string
  description: string
  action: 'created' | 'updated' | 'skipped' | 'conflict'
  oldVersion?: string
  newVersion: string
  diffHtmlPath?: string
}

function parseMarkdownPrompt(markdown: string): MarkdownPrompt
async function loadSystemPromptsWithRegex(
  version: string,
  shouldEscapeNonAscii?: boolean,
  buildTime?: string
): Promise<Map<string, SystemPromptEntry>>
async function syncSystemPrompts(version: string): Promise<SyncSummary>
```

**Markdown Format:**

```markdown
<!--
name: System Prompt Name
description: Brief description
ccVersion: 2.0.76
variables:
  - SETTINGS
  - CONFIG
-->

This is the prompt content with ${SETTINGS.preferredName}.
```

---

### `systemPromptDownload.ts` - GitHub Downloads

**Purpose:** Download prompt data from GitHub.

**Location:** `src/systemPromptDownload.ts`

**Key Exports:**

```typescript
async function downloadStringsFile(version: string): Promise<StringsFile>
```

**Workflow:**

1. Check local cache (`~/.tweakcc/prompt-data-cache/prompts-{version}.json`)
2. If cache hit, return cached data
3. If cache miss, fetch from GitHub:
   ```
   https://raw.githubusercontent.com/Piebald-AI/tweakcc/.../prompts-{version}.json
   ```
4. Cache result for future use

**Error Handling:**

| HTTP Code | Action |
|-----------|--------|
| 429 | Rate limit - retry later |
| 404 | Version not found |
| 500+ | Server error |

---

### `systemPromptHashIndex.ts` - Change Tracking

**Purpose:** Track prompt modifications via hashes.

**Location:** `src/systemPromptHashIndex.ts`

**Key Exports:**

```typescript
function computeMD5Hash(content: string): string
function getHashKey(promptId: string, version: string): string
async function readHashIndex(): Promise<HashIndex>
async function writeHashIndex(index: HashIndex): Promise<void>
async function hasUnappliedSystemPromptChanges(
  systemPromptsDir: string
): Promise<boolean>
```

**Hash Files:**

| File | Purpose |
|------|---------|
| `systemPromptOriginalHashes.json` | Baseline hashes from download |
| `systemPromptAppliedHashes.json` | Hashes when last applied |

---

## Patch Modules

See [patches.md](./patches.md) for detailed documentation of all 27 patch modules.

---

## UI Modules

### `ui/App.tsx` - Main Application

**Purpose:** Root React component for the interactive UI.

**Location:** `src/ui/App.tsx`

**Key Exports:**

```typescript
interface AppProps {
  startupCheckInfo: StartupCheckInfo
  configMigrated: boolean
}

function App(props: AppProps): JSX.Element
```

**State Management:**

```typescript
const [config, setConfig] = useState<TweakccConfig>()
const [showPiebaldAnnouncement, setShowPiebaldAnnouncement] = useState(true)
const [currentView, setCurrentView] = useState<MainMenuItem | null>(null)
const [notification, setNotification] = useState<Notification | null>(null)
```

**Context Provider:**

```typescript
const SettingsContext = createContext<{
  settings: Settings
  updateSettings: (updates: Partial<Settings>) => void
}>()
```

**Keyboard Shortcuts:**

| Key | Action |
|-----|--------|
| Ctrl+C | Exit |
| Q / Escape | Exit (in main menu) |
| H | Hide announcement |

---

### UI Components

See [ui.md](./ui.md) for detailed documentation of all 15 UI components.

---

## Test Modules

See [testing.md](./testing.md) for detailed documentation of all 8 test files.

---

## Module Dependency Summary

| Module | Dependencies | Dependents |
|--------|--------------|------------|
| `types.ts` | None | All modules |
| `utils.ts` | `types.ts` | Most modules |
| `config.ts` | `types.ts`, `utils.ts` | `startup.ts`, `patches/index.ts` |
| `defaultSettings.ts` | `types.ts` | `config.ts` |
| `installationPaths.ts` | `utils.ts` | `installationDetection.ts` |
| `installationDetection.ts` | `installationPaths.ts`, `types.ts` | `startup.ts` |
| `installationBackup.ts` | `config.ts`, `utils.ts` | `patches/index.ts` |
| `systemPromptSync.ts` | `systemPromptDownload.ts`, `systemPromptHashIndex.ts` | `patches/systemPrompts.ts` |
| `patches/index.ts` | All patch modules | `ui/App.tsx` |
| `ui/App.tsx` | `config.ts`, `patches/index.ts`, all UI components | `index.tsx` |
