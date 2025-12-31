# tweakcc Testing

> Test Coverage, Patterns, and Best Practices

## Overview

tweakcc uses Vitest as its test framework with comprehensive coverage across installation detection, configuration management, system prompt handling, and patch logic.

## Test Statistics

| Metric | Value |
|--------|-------|
| Total Test Files | 8 |
| Total Test Lines | 2,600+ |
| Test Framework | Vitest 3.2.4 |
| Coverage Tool | Vitest built-in |

## Test Configuration

### vitest.config.ts

```typescript
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,  // describe, it, expect available globally
  },
});
```

### Running Tests

```bash
# Run all tests
npm test

# Run tests in watch mode
npm test -- --watch

# Run specific test file
npm test -- src/tests/config.test.ts

# Run with coverage
npm test -- --coverage
```

---

## Test Files

### 1. `config.test.ts` - Configuration & Detection

**Location:** `src/tests/config.test.ts`
**Lines:** ~1,368

**Test Coverage:**

#### Configuration Management

```typescript
describe('warnAboutMultipleConfigs', () => {
  it('should warn when multiple config locations exist');
  it('should not warn for single location');
});

describe('ensureConfigDir', () => {
  it('should create config directory with recursive flag');
});

describe('readConfigFile', () => {
  it('should return default config if file missing');
  it('should parse existing config');
  it('should merge missing properties from defaults');
});

describe('updateConfigFile', () => {
  it('should update config with callback');
  it('should persist changes');
  it('should update lastModified timestamp');
});
```

#### Installation Detection

```typescript
describe('findClaudeCodeInstallation', () => {
  // Environment variable detection
  it('should use TWEAKCC_CC_INSTALLATION_PATH when set');

  // Config-based detection
  it('should use ccInstallationPath from config');

  // PATH lookup
  it('should find claude via which command');
  it('should resolve symlinks');
  it('should handle Windows .cmd shims');

  // Search path scanning
  it('should scan npm global paths');
  it('should scan nvm paths');
  it('should scan fnm paths');
  it('should scan volta paths');
  it('should scan Homebrew paths');
  it('should scan native binary locations');

  // Type detection
  it('should detect npm-based installations');
  it('should detect native binary installations');
  it('should use WASMagic for MIME detection');

  // Version extraction
  it('should extract version from path');
  it('should extract version from content');
  it('should handle multiple VERSION strings');

  // Error handling
  it('should handle ENOENT errors');
  it('should handle ENOTDIR errors');
  it('should handle EACCES errors');
  it('should handle EPERM errors');

  // Multiple installations
  it('should collect all candidates');
  it('should sort by version descending');
  it('should deduplicate paths');
});
```

**Mock Dependencies:**

```typescript
vi.mock('wasmagic');
vi.mock('node:fs/promises');
vi.mock('node:child_process');
vi.mock('node:fs');
vi.mock('which');
vi.mock('../nativeInstallationLoader.js');
```

---

### 2. `systemPromptSync.test.ts` - Prompt Synchronization

**Location:** `src/tests/systemPromptSync.test.ts`
**Lines:** ~1,071

**Test Coverage:**

#### Markdown Parsing

```typescript
describe('parseMarkdownPrompt', () => {
  it('should extract frontmatter fields');
  it('should parse variables array');
  it('should calculate content line offset');
  it('should handle missing optional fields');
  it('should handle empty variables');
});
```

#### Content Reconstruction

```typescript
describe('reconstructPromptContent', () => {
  it('should join pieces with variable placeholders');
  it('should handle empty pieces array');
  it('should preserve whitespace');
});

describe('buildRegexFromPieces', () => {
  it('should escape special regex characters');
  it('should create capture groups for identifiers');
  it('should handle complex patterns');
});

describe('extractUserCustomizations', () => {
  it('should extract customized content');
  it('should preserve variable references');
});
```

#### Sync Logic

```typescript
describe('syncSystemPrompts', () => {
  it('should create new files if missing');
  it('should skip if versions match');
  it('should update when upstream changes');
  it('should detect conflicts');
  it('should generate HTML diffs for conflicts');
  it('should update variable frontmatter');
});
```

#### Edge Cases

```typescript
describe('edge cases', () => {
  it('should handle double dollar signs ($$)');
  it('should handle <<BUILD_TIME>> placeholder');
  it('should handle <<CCVERSION>> placeholder');
  it('should handle actual newlines vs \\n literals');
  it('should escape HTML entities in diffs');
});
```

**Example Test:**

```typescript
it('should handle double dollar signs in variables', () => {
  const input = 'Timeout: J$$() ms';
  const result = formatForReplacement(input);
  expect(result).toContain('J$$');  // Not J$
  expect(result).not.toContain('J$()');
});
```

---

### 3. `migration.test.ts` - Configuration Migrations

**Location:** `src/tests/migration.test.ts`
**Lines:** ~289

**Test Coverage:**

#### userMessageDisplay Migration

```typescript
describe('migrateUserMessageDisplayToV320', () => {
  it('should migrate old prefix/message format');
  it('should merge styling arrays');
  it('should convert rgb(0,0,0) to null');
  it('should preserve custom colors');
  it('should add default border properties');
  it('should be idempotent');
});
```

**Example:**

```typescript
it('should migrate old format to new format', () => {
  const oldConfig = {
    userMessageDisplay: {
      prefix: {
        format: '$',
        styling: ['bold'],
        foregroundColor: 'rgb(255,0,0)'
      },
      message: {
        format: '{}',
        styling: ['italic'],
        foregroundColor: 'rgb(0,255,0)'
      }
    }
  };

  migrateUserMessageDisplayToV320(oldConfig);

  expect(oldConfig.userMessageDisplay).toEqual({
    format: '${}',
    styling: ['bold', 'italic'],
    foregroundColor: 'rgb(0,255,0)',
    backgroundColor: null,
    borderStyle: 'none',
    borderColor: 'rgb(255,255,255)',
    paddingX: 0,
    paddingY: 0,
    fitBoxToContent: false
  });
});
```

#### ccInstallationDir Migration

```typescript
describe('migrateConfigIfNeeded', () => {
  it('should convert ccInstallationDir to ccInstallationPath');
  it('should append cli.js to directory paths');
  it('should return false on second call (idempotent)');
});
```

---

### 4. `searchPaths.test.ts` - Path Search Errors

**Location:** `src/tests/searchPaths.test.ts`
**Lines:** ~349

**Test Coverage:**

```typescript
describe('expandSearchPaths', () => {
  // Error handling
  it('should handle EACCES gracefully');
  it('should handle EPERM gracefully');
  it('should handle other errors gracefully');
  it('should continue processing after errors');

  // Platform-specific tests
  it.skipIf(win32)('should expand Unix paths');
  it.skipIf(!win32)('should expand Windows paths');
});
```

**Patterns Tested:**

| Path | Purpose |
|------|---------|
| `/usr/local/n/versions/node/` | n manager |
| `/usr/local/nvm/versions/` | nvm |
| `~/.nvm/versions/` | nvm home |
| `AppData/Roaming/nvm/` | Windows nvm |
| `AppData/Local/pnpm/global/` | Windows pnpm |
| `AppData/Local/fnm_multishells/` | Windows fnm |

---

### 5. `systemPrompts.test.ts` - Prompt Patching

**Location:** `src/patches/systemPrompts.test.ts`
**Lines:** ~235

**Test Coverage:**

#### Dollar Sign Handling

```typescript
describe('dollar sign handling', () => {
  it('should preserve J$$ as J$$');
  it('should not convert J$$ to J$');
  it('should handle multiple $$ occurrences');
});
```

#### String Formatting

```typescript
describe('formatStringForJs', () => {
  it('should convert newlines to \\n in double-quoted strings');
  it('should keep actual newlines in backtick templates');
  it('should escape double quotes');
  it('should escape single quotes');
});
```

**Example:**

```typescript
it('should convert newlines for double-quoted strings', () => {
  const input = 'Line 1\nLine 2\nLine 3';
  const result = formatForDoubleQuotes(input);
  expect(result).toBe('Line 1\\nLine 2\\nLine 3');
});

it('should preserve newlines for backtick templates', () => {
  const input = 'Line 1\nLine 2';
  const result = formatForBacktick(input);
  expect(result).toBe('Line 1\nLine 2');
});
```

---

### 6. `tweakccConfigDir.test.ts` - Config Directory

**Location:** `src/tests/tweakccConfigDir.test.ts`
**Lines:** ~145

**Test Coverage:**

```typescript
describe('getConfigDir', () => {
  // Priority order
  it('should use TWEAKCC_CONFIG_DIR when set');
  it('should expand tilde in env var');
  it('should use ~/.tweakcc if exists');
  it('should use ~/.claude/tweakcc second');
  it('should use XDG_CONFIG_HOME/tweakcc third');
  it('should fallback to ~/.tweakcc');

  // Edge cases
  it('should ignore empty env var');
  it('should trim whitespace');
});
```

**Priority Order Tested:**

| Priority | Condition | Path |
|----------|-----------|------|
| 1 | `TWEAKCC_CONFIG_DIR` set | `$TWEAKCC_CONFIG_DIR` |
| 2 | `~/.tweakcc` exists | `~/.tweakcc` |
| 3 | Default | `~/.claude/tweakcc` |
| 4 | `XDG_CONFIG_HOME` set | `$XDG_CONFIG_HOME/tweakcc` |
| 5 | Fallback | `~/.tweakcc` |

---

### 7. `xdgConfigHome.test.ts` - XDG Compliance

**Location:** `src/tests/xdgConfigHome.test.ts`
**Lines:** ~143

**Test Coverage:**

```typescript
describe('XDG Base Directory compliance', () => {
  it('should prefer existing ~/.tweakcc over XDG');
  it('should use XDG_CONFIG_HOME for new users');
  it('should fallback to ~/.tweakcc without XDG');
  it('should use ~/.config/tweakcc as standard XDG path');
});

describe('derived paths', () => {
  it('should derive config.json path');
  it('should derive cli.js.backup path');
  it('should derive system-prompts directory');
});
```

---

### 8. `bunxVersionSorting.test.ts` - Version Comparison

**Location:** `src/tests/bunxVersionSorting.test.ts`
**Lines:** ~50

**Test Coverage:**

```typescript
describe('compareSemverVersions', () => {
  it('should return positive when a > b');
  it('should return negative when a < b');
  it('should return 0 when equal');
  it('should handle major version differences');
  it('should handle minor version differences');
  it('should handle patch version differences');
  it('should handle partial versions');
});

describe('sorting', () => {
  it('should sort ascending correctly');
  it('should sort descending correctly');
});
```

**Example:**

```typescript
it('should compare versions correctly', () => {
  expect(compareSemverVersions('2.0.67', '2.0.60')).toBeGreaterThan(0);
  expect(compareSemverVersions('2.0.60', '2.0.67')).toBeLessThan(0);
  expect(compareSemverVersions('2.0.67', '2.0.67')).toBe(0);
  expect(compareSemverVersions('10.20.300', '10.20.299')).toBeGreaterThan(0);
});
```

---

## Testing Patterns

### Mocking Strategy

#### Module Mocking

```typescript
vi.mock('wasmagic', () => ({
  WASMagic: {
    create: vi.fn().mockResolvedValue({
      getMime: vi.fn().mockReturnValue('application/javascript')
    })
  }
}));
```

#### Function Spying

```typescript
const writeSpy = vi.spyOn(fs, 'writeFile');
// ... test code ...
expect(writeSpy).toHaveBeenCalledWith('/path/to/file', expectedContent);
```

#### Implementation Mocking

```typescript
vi.mocked(fs.readFile).mockImplementation(async (path) => {
  if (path === '/valid/path') {
    return Buffer.from('content');
  }
  throw new Error('ENOENT');
});
```

### Setup and Teardown

```typescript
describe('myTest', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset state
  });

  afterEach(() => {
    // Cleanup
  });

  beforeAll(() => {
    // One-time setup
  });

  afterAll(() => {
    // One-time cleanup
  });
});
```

### Platform-Specific Tests

```typescript
import { platform } from 'node:os';

const isWindows = platform() === 'win32';

describe('platform-specific', () => {
  it.skipIf(isWindows)('Unix-only test', () => {
    // Only runs on Unix
  });

  it.skipIf(!isWindows)('Windows-only test', () => {
    // Only runs on Windows
  });
});
```

### Assertion Patterns

```typescript
// Comparison
expect(value).toBeGreaterThan(0);
expect(value).toBeLessThan(0);
expect(value).toBe(0);

// Equality
expect(result).toEqual(expected);
expect(result).toEqual(expect.objectContaining({ key: value }));

// Array
expect(array.map(x => x.version)).toEqual(['2.0.67', '2.0.65', '2.0.60']);

// Function calls
expect(mockFn).toHaveBeenCalled();
expect(mockFn).toHaveBeenCalledWith(arg1, arg2);
expect(mockFn).toHaveBeenCalledTimes(3);

// Async
await expect(asyncFn()).resolves.toBe(value);
await expect(asyncFn()).rejects.toThrow('error');

// Truthiness
expect(value).toBeTruthy();
expect(value).toBeFalsy();
expect(value).toBeNull();
expect(value).toBeDefined();
```

---

## Test Helpers

### `testHelpers.ts`

```typescript
// Create mock config
function createMockConfig(overrides?: Partial<TweakccConfig>): TweakccConfig {
  return {
    version: '3.2.2',
    lastModified: new Date().toISOString(),
    changesApplied: false,
    themeId: 'dark',
    themes: [],
    thinkingVerbs: { format: '{}', verbs: [] },
    thinkingStyle: { reverseMirror: true, updateInterval: 120, phases: [] },
    userMessageDisplay: { format: '{}', styling: [], ... },
    inputBox: { removeBorder: false },
    misc: { ... },
    toolsets: [],
    ...overrides
  };
}

// Create mock installation info
function createMockInstallation(
  kind: 'npm-based' | 'native-binary' = 'npm-based'
): ClaudeCodeInstallationInfo {
  return {
    cliPath: '/path/to/cli.js',
    version: '2.0.76',
    source: 'search-paths'
  };
}
```

---

## Running Specific Tests

```bash
# Run single test file
npm test -- src/tests/config.test.ts

# Run tests matching pattern
npm test -- -t "should handle"

# Run in watch mode
npm test -- --watch

# Run with verbose output
npm test -- --reporter=verbose

# Generate coverage report
npm test -- --coverage
```

---

## Best Practices

### Test Organization

1. Group related tests with `describe()`
2. Use clear, descriptive test names
3. Follow AAA pattern (Arrange, Act, Assert)
4. Keep tests focused and independent

### Mocking

1. Mock at the module boundary
2. Reset mocks between tests
3. Use realistic mock data
4. Avoid over-mocking

### Assertions

1. Use specific matchers
2. Test both success and failure cases
3. Verify side effects
4. Check error messages

### Coverage

1. Aim for high coverage on critical paths
2. Don't sacrifice readability for coverage
3. Test edge cases explicitly
4. Include integration tests
