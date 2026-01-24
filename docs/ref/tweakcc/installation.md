# tweakcc Installation Detection

> Finding and Managing Claude Code Installations

## Overview

tweakcc automatically detects Claude Code installations across multiple package managers, node version managers, and installation methods.

## Detection Priority

```
┌─────────────────────────────────────────────────────────────────┐
│                    Detection Priority Order                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. TWEAKCC_CC_INSTALLATION_PATH environment variable           │
│     └── Direct path to cli.js or native binary                  │
│                                                                  │
│  2. ccInstallationPath from config.json                         │
│     └── Previously saved installation path                       │
│                                                                  │
│  3. PATH lookup via `which claude`                              │
│     └── Resolves symlinks to actual installation                │
│                                                                  │
│  4. Hardcoded search paths (40+ locations)                      │
│     └── Platform-specific package manager locations              │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Installation Types

### npm-based Installation

Standard npm/yarn/pnpm/Bun installation:

```
/path/to/node_modules/@anthropic-ai/claude-code/dist/cli.js
```

**Detection Method:** File type is JavaScript (text/javascript MIME type)

**Patching:** Direct modification of `cli.js`

### Native Binary Installation

Standalone executable for Mac/Linux/Windows:

```
~/.local/bin/claude                    # Linux
/usr/local/bin/claude                  # macOS
C:\Users\<user>\AppData\Local\...      # Windows
```

**Detection Method:** Binary magic number detection using WASMagic library

**Patching:**
1. Extract embedded `claude.js` from binary
2. Patch JavaScript
3. Repack binary with modified JavaScript

## Search Paths

### NPM Global Locations

| Platform | Path |
|----------|------|
| All | `~/.claude/local/node_modules/@anthropic-ai/claude-code` |
| All | `$NPM_PREFIX/lib/node_modules/@anthropic-ai/claude-code` |
| macOS | `/usr/local/lib/node_modules/@anthropic-ai/claude-code` |
| macOS | `/opt/homebrew/lib/node_modules/@anthropic-ai/claude-code` |
| Linux | `/usr/lib/node_modules/@anthropic-ai/claude-code` |
| Linux | `/usr/local/lib/node_modules/@anthropic-ai/claude-code` |
| Windows | `%APPDATA%\npm\node_modules\@anthropic-ai\claude-code` |

### Yarn Global Locations

| Platform | Path |
|----------|------|
| macOS/Linux | `~/.config/yarn/global/node_modules/@anthropic-ai/claude-code` |
| macOS/Linux | `~/.yarn/global/node_modules/@anthropic-ai/claude-code` |
| Windows | `%LOCALAPPDATA%\Yarn\Data\global\node_modules\...` |

### pnpm Locations

| Platform | Path |
|----------|------|
| macOS/Linux | `~/.local/share/pnpm/global/5/node_modules/@anthropic-ai/claude-code` |
| macOS/Linux | `$PNPM_HOME/global/*/node_modules/@anthropic-ai/claude-code` |
| Windows | `%LOCALAPPDATA%\pnpm\global\*\node_modules\...` |

### Bun Locations

| Platform | Path |
|----------|------|
| macOS/Linux | `~/.bun/install/global/node_modules/@anthropic-ai/claude-code` |
| macOS/Linux | `~/.cache/bun/install/cache/@anthropic-ai/claude-code@*` |
| Windows | `%USERPROFILE%\.bun\install\global\node_modules\...` |

### Node Version Managers

#### nvm (Node Version Manager)

```
~/.nvm/versions/node/v*/lib/node_modules/@anthropic-ai/claude-code
$NVM_DIR/versions/node/v*/lib/node_modules/@anthropic-ai/claude-code
```

#### fnm (Fast Node Manager)

```
~/.local/share/fnm/node-versions/v*/installation/lib/node_modules/@anthropic-ai/claude-code
$FNM_DIR/node-versions/v*/installation/lib/node_modules/@anthropic-ai/claude-code
~/.fnm/node-versions/v*/installation/lib/node_modules/@anthropic-ai/claude-code
```

#### volta

```
~/.volta/tools/image/node/*/lib/node_modules/@anthropic-ai/claude-code
$VOLTA_HOME/tools/image/node/*/lib/node_modules/@anthropic-ai/claude-code
```

#### asdf

```
~/.asdf/installs/nodejs/*/lib/node_modules/@anthropic-ai/claude-code
```

#### mise (formerly rtx)

```
~/.local/share/mise/installs/node/*/lib/node_modules/@anthropic-ai/claude-code
```

#### nodenv

```
~/.nodenv/versions/*/lib/node_modules/@anthropic-ai/claude-code
```

#### nvs (Node Version Switcher)

```
~/.nvs/node/*/*/lib/node_modules/@anthropic-ai/claude-code
$NVS_HOME/node/*/*/lib/node_modules/@anthropic-ai/claude-code
```

#### n

```
/usr/local/n/versions/node/*/lib/node_modules/@anthropic-ai/claude-code
$N_PREFIX/n/versions/node/*/lib/node_modules/@anthropic-ai/claude-code
```

### Native Binary Locations

| Platform | Path |
|----------|------|
| macOS/Linux | `~/.local/bin/claude` |
| macOS/Linux | `~/.local/share/claude/versions/*` |
| macOS | `/usr/local/bin/claude` |
| Linux | `/usr/bin/claude` |
| Windows | `%LOCALAPPDATA%\Programs\claude\claude.exe` |

### Windows-Specific Paths

```
%APPDATA%\npm\node_modules\@anthropic-ai\claude-code
%LOCALAPPDATA%\pnpm\global\*\node_modules\@anthropic-ai\claude-code
%LOCALAPPDATA%\fnm_multishells\*\installation\lib\node_modules\...
%APPDATA%\nvm\v*\node_modules\@anthropic-ai\claude-code
```

## Detection Process

### Phase 1: Environment Variable

```typescript
const envPath = process.env.TWEAKCC_CC_INSTALLATION_PATH;
if (envPath) {
  // Validate path exists and is correct type
  return resolveInstallation(envPath);
}
```

### Phase 2: Config File

```typescript
const configPath = config.ccInstallationPath;
if (configPath) {
  // Validate path exists and is correct type
  return resolveInstallation(configPath);
}
```

### Phase 3: PATH Lookup

```typescript
import which from 'which';

const claudePath = await which('claude');
if (claudePath) {
  // Resolve symlinks
  const realPath = await fs.realpath(claudePath);
  return resolveInstallation(realPath);
}
```

### Phase 4: Search Paths

```typescript
const candidates: InstallationCandidate[] = [];

for (const searchPath of getSearchPaths()) {
  // Expand globs (e.g., ~/.nvm/versions/node/*)
  const paths = await globby(searchPath);

  for (const path of paths) {
    const resolved = await resolveInstallation(path);
    if (resolved) {
      candidates.push(resolved);
    }
  }
}

// Sort by version descending
return candidates.sort((a, b) =>
  compareSemverVersions(b.version, a.version)
);
```

## Installation Resolution

### Type Detection

Using WASMagic (WASM-based libmagic):

```typescript
import { WASMagic } from 'wasmagic';

async function resolvePathToInstallationType(filePath: string) {
  const magic = await WASMagic.create();
  const buffer = await fs.readFile(filePath, { length: 4096 });
  const mimeType = magic.getMime(buffer);

  if (mimeType.includes('javascript') || mimeType.includes('text')) {
    return { kind: 'npm-based', resolvedPath: filePath };
  }

  if (mimeType.includes('executable') || mimeType.includes('binary')) {
    return { kind: 'native-binary', resolvedPath: filePath };
  }

  return null;
}
```

### Symlink Resolution

```typescript
async function resolveSymlinks(path: string): Promise<string> {
  try {
    return await fs.realpath(path);
  } catch {
    return path;
  }
}
```

### Version Extraction

```typescript
async function extractVersion(
  filePath: string,
  kind: InstallationKind
): Promise<string> {
  // Method 1: Version from path
  // ~/.local/share/claude/versions/2.0.65 → "2.0.65"
  const pathMatch = filePath.match(/versions\/(\d+\.\d+\.\d+)/);
  if (pathMatch) return pathMatch[1];

  // Method 2: VERSION string in content
  const content = await readFileContent(filePath, kind);
  const versionMatches = content.matchAll(/VERSION:"(\d+\.\d+\.\d+)"/g);

  // Return most frequent version (handles minified code)
  const versions = countVersions(versionMatches);
  return getMostFrequent(versions);
}
```

## Multiple Installation Handling

### Interactive Mode

When multiple installations are found:

```
┌─────────────────────────────────────────────────────────────────┐
│              Multiple Claude Code Installations Found            │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Select an installation:                                        │
│                                                                  │
│  > v2.0.76 (npm-based) - ~/.nvm/versions/node/v20/...          │
│    v2.0.70 (npm-based) - /usr/local/lib/node_modules/...        │
│    v2.0.65 (native)    - ~/.local/bin/claude                    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Non-Interactive Mode

In `--apply` mode with multiple installations:

```
Error: Multiple Claude Code installations found.
Please set TWEAKCC_CC_INSTALLATION_PATH or add ccInstallationPath to config.

Found:
  - v2.0.76 at ~/.nvm/versions/node/v20/lib/node_modules/...
  - v2.0.70 at /usr/local/lib/node_modules/...
```

## Candidate Collection

### InstallationCandidate Interface

```typescript
interface InstallationCandidate {
  path: string           // Full path to cli.js or binary
  kind: InstallationKind // 'npm-based' | 'native-binary'
  version: string        // Semantic version (e.g., "2.0.76")
}
```

### Deduplication

```typescript
function deduplicateCandidates(
  candidates: InstallationCandidate[]
): InstallationCandidate[] {
  const seen = new Set<string>();
  return candidates.filter(c => {
    const key = c.path;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
```

### Sorting

```typescript
function sortCandidates(
  candidates: InstallationCandidate[]
): InstallationCandidate[] {
  return candidates.sort((a, b) =>
    compareSemverVersions(b.version, a.version)  // Descending
  );
}
```

## Native Binary Support

### Extraction

```typescript
import { extractClaudeJsFromNativeInstallation } from './nativeInstallationLoader';

const jsContent = await extractClaudeJsFromNativeInstallation(binaryPath);
if (!jsContent) {
  // node-lief not available, skip native support
  return null;
}
```

### Repacking

```typescript
import { repackNativeInstallation } from './nativeInstallationLoader';

await repackNativeInstallation(
  binaryPath,
  modifiedJsBuffer,
  binaryPath  // Overwrite original
);
```

### Platform Support

| Platform | Binary Format | node-lief Support |
|----------|---------------|-------------------|
| macOS (Intel) | Mach-O x86_64 | Yes |
| macOS (ARM) | Mach-O ARM64 | Yes |
| Linux (x86_64) | ELF | Yes |
| Linux (ARM64) | ELF | Yes |
| Windows | PE | Partial |

### Graceful Fallback

```typescript
try {
  // Dynamically import node-lief
  const nodeLief = await import('node-lief');
  // Use native support
} catch {
  // node-lief unavailable (NixOS, missing libraries)
  console.warn('Native binary support disabled');
  return null;
}
```

## Backup Management

### Creating Backups

```typescript
import { backupClijs, backupNativeBinary } from './installationBackup';

if (ccInstInfo.kind === 'npm-based') {
  await backupClijs(ccInstInfo);
  // Creates ~/.tweakcc/cli.js.backup
}

if (ccInstInfo.kind === 'native-binary') {
  await backupNativeBinary(ccInstInfo);
  // Creates ~/.tweakcc/native-binary.backup
}
```

### Restoring Backups

```typescript
import {
  restoreClijsFromBackup,
  restoreNativeBinaryFromBackup
} from './installationBackup';

if (ccInstInfo.kind === 'npm-based') {
  await restoreClijsFromBackup(ccInstInfo);
}

if (ccInstInfo.kind === 'native-binary') {
  await restoreNativeBinaryFromBackup(ccInstInfo);
}
```

### Version Change Detection

```typescript
if (currentVersion !== backupVersion) {
  // Claude Code was updated
  // Create new backup before patching
  await createBackup(ccInstInfo);
}
```

## Hard Link Handling

### The Problem

Bun caches packages using hard links. Modifying a hard-linked file affects all links.

### The Solution

```typescript
async function replaceFileBreakingHardLinks(
  filePath: string,
  newContent: string | Buffer
): Promise<void> {
  // 1. Read original permissions
  const stats = await fs.stat(filePath);
  const originalMode = stats.mode;

  // 2. Unlink file (breaks hard link)
  await fs.unlink(filePath);

  // 3. Write new content (creates new inode)
  await fs.writeFile(filePath, newContent);

  // 4. Restore permissions
  await fs.chmod(filePath, originalMode);
}
```

## Error Handling

### InstallationDetectionError

```typescript
class InstallationDetectionError extends Error {
  constructor(
    message: string,
    public candidates?: InstallationCandidate[],
    public searchedPaths?: string[]
  ) {
    super(message);
    this.name = 'InstallationDetectionError';
  }
}
```

### Error Messages

**No installation found:**
```
Error: Could not find Claude Code installation.

Searched locations:
  - ~/.nvm/versions/node/*/lib/node_modules/@anthropic-ai/claude-code
  - /usr/local/lib/node_modules/@anthropic-ai/claude-code
  - ...

Solutions:
  1. Install Claude Code: npm install -g @anthropic-ai/claude-code
  2. Set TWEAKCC_CC_INSTALLATION_PATH environment variable
  3. Add ccInstallationPath to ~/.tweakcc/config.json
```

**Permission denied:**
```
Error: Permission denied accessing Claude Code installation.

Path: /usr/local/lib/node_modules/@anthropic-ai/claude-code/dist/cli.js

Solutions:
  1. Run with elevated permissions (sudo)
  2. Change file permissions
  3. Use a user-local installation (nvm, fnm, etc.)
```

## Environment Variables

| Variable | Purpose | Example |
|----------|---------|---------|
| `TWEAKCC_CC_INSTALLATION_PATH` | Direct installation path | `/path/to/cli.js` |
| `NPM_PREFIX` | npm global prefix | `/usr/local` |
| `NVM_DIR` | nvm directory | `~/.nvm` |
| `VOLTA_HOME` | Volta directory | `~/.volta` |
| `FNM_DIR` | fnm directory | `~/.local/share/fnm` |
| `PNPM_HOME` | pnpm home | `~/.local/share/pnpm` |
| `N_PREFIX` | n prefix | `/usr/local` |
| `NVS_HOME` | nvs home | `~/.nvs` |

## Troubleshooting

### Installation Not Found

1. Check if Claude Code is installed: `claude --version`
2. Find installation: `which claude` or `npm list -g @anthropic-ai/claude-code`
3. Set explicit path: `export TWEAKCC_CC_INSTALLATION_PATH=/path/to/cli.js`

### Wrong Version Detected

1. Multiple installations may exist
2. Use picker in interactive mode
3. Set explicit path in config

### Native Binary Issues

1. Ensure `node-lief` can be installed
2. Check for missing system libraries
3. Try npm-based installation instead

### Permission Errors

1. Check file ownership
2. Use user-local installation (nvm, fnm)
3. Avoid system-wide npm installations
