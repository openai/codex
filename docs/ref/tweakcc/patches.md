# tweakcc Patch System

> Complete Documentation of All 27 Patch Modules

## Overview

The patch system is the core of tweakcc. It applies regex-based transformations to Claude Code's minified JavaScript to customize behavior and appearance.

## Patch Architecture

### Orchestration Flow

```
patches/index.ts (Orchestrator)
        │
        ├── Restore from backup
        │
        ├── Apply patches in order:
        │   │
        │   ├── 1. systemPrompts.ts
        │   ├── 2. themes.ts
        │   ├── 3. thinkerVerbs.ts
        │   ├── 4. thinkerFormat.ts
        │   ├── 5. thinkerSymbolChars.ts
        │   ├── 6. thinkerSymbolSpeed.ts
        │   ├── 7. thinkerSymbolWidth.ts
        │   ├── 8. thinkerMirrorOption.ts
        │   ├── 9. userMessageDisplay.ts
        │   ├── 10. inputBorderBox.ts
        │   ├── 11. verboseProperty.ts
        │   ├── 12. spinnerNoFreeze.ts
        │   ├── 13. contextLimit.ts
        │   ├── 14. modelSelector.ts
        │   ├── 15. showMoreItemsInSelectMenus.ts
        │   ├── 16. thinkingVisibility.ts
        │   ├── 17. patchesAppliedIndication.ts
        │   ├── 18. fixLspSupport.ts
        │   ├── 19. toolsets.ts
        │   ├── 20. conversationTitle.ts
        │   ├── 21. hideStartupBanner.ts
        │   ├── 22. hideCtrlGToEditPrompt.ts
        │   ├── 23. hideStartupClawd.ts
        │   ├── 24. increaseFileReadLimit.ts
        │   └── 25. slashCommands.ts
        │
        └── Write patched content
```

### Helper Functions

The orchestrator (`patches/index.ts`) provides common utilities:

```typescript
// Find minified chalk library variable
function findChalkVar(fileContents: string): string | undefined

// Find module loader function (CommonJS/ESM)
function getModuleLoaderFunction(fileContents: string): string | undefined

// Find React module (handles Node.js and Bun)
function getReactVar(fileContents: string): string | undefined

// Get require function name (esbuild vs Bun)
function getRequireFuncName(fileContents: string): string

// Find Ink components
function findTextComponent(fileContents: string): string | undefined
function findBoxComponent(fileContents: string): string | undefined

// Escape $ in variable names for regex
function escapeIdent(ident: string): string

// Debug output for patch changes
function showDiff(oldFile, newFile, injectedText, startIndex, endIndex): void
```

---

## Patch Modules

### 1. `themes.ts` - Theme Color Replacement

**Purpose:** Replace theme color definitions with custom colors.

**File:** `src/patches/themes.ts`

**Targets:**
- Theme switch statement
- Color mapping objects
- Theme selector options

**Pattern Example:**
```javascript
// Original (minified)
switch(e){case"dark":return{primary:"#fff",...}

// Patched
switch(e){case"dark":return{primary:"rgb(255,100,100)",...}
```

**Configuration Used:**
```typescript
config.themes[config.themeId].colors
```

**Key Operations:**
1. Find theme switch statement
2. Extract theme cases
3. Replace color values per theme
4. Add custom themes to selector

---

### 2. `thinkerVerbs.ts` - Thinking Verb Replacement

**Purpose:** Replace the list of thinking action verbs.

**File:** `src/patches/thinkerVerbs.ts`

**Pattern:**
```regex
[, ]([$\w]+)=\{words:\[(?:"[^"{}()]+ing",)+"[^"{}()]+ing"\]\}
```

**Example:**
```javascript
// Original
const e={words:["Thinking","Pondering","Computing"]}

// Patched
const e={words:["Actualizing","Baking","Razzle-dazzling",...]}
```

**Configuration Used:**
```typescript
config.thinkingVerbs.verbs  // string[]
```

---

### 3. `thinkerFormat.ts` - Thinking Format Template

**Purpose:** Change the format string for thinking display.

**File:** `src/patches/thinkerFormat.ts`

**Pattern:**
```regex
format:\s*"\{\}\.\.\.\s*"
```

**Example:**
```javascript
// Original
format: "{}... "

// Patched
format: "{}~ "
```

**Configuration Used:**
```typescript
config.thinkingVerbs.format  // e.g., "{}~ "
```

---

### 4. `thinkerSymbolChars.ts` - Spinner Animation Phases

**Purpose:** Replace spinner animation characters.

**File:** `src/patches/thinkerSymbolChars.ts`

**Pattern:**
```regex
phases:\s*\["[^"]+",\s*"[^"]+",\s*"[^"]+",\s*"[^"]+"\]
```

**Example:**
```javascript
// Original
phases: ["·", "✢", "*", "✦"]

// Patched
phases: ["◐", "◓", "◑", "◒"]
```

**Configuration Used:**
```typescript
config.thinkingStyle.phases  // string[]
```

---

### 5. `thinkerSymbolSpeed.ts` - Animation Update Interval

**Purpose:** Adjust spinner animation speed.

**File:** `src/patches/thinkerSymbolSpeed.ts`

**Pattern:**
```regex
updateInterval:\s*\d+
```

**Example:**
```javascript
// Original
updateInterval: 120

// Patched
updateInterval: 80
```

**Configuration Used:**
```typescript
config.thinkingStyle.updateInterval  // number (milliseconds)
```

---

### 6. `thinkerSymbolWidth.ts` - Spinner Width Calculation

**Purpose:** Adjust spinner glyph width calculation.

**File:** `src/patches/thinkerSymbolWidth.ts`

**Purpose:** Fixes width calculation for custom spinner characters that may have different Unicode widths.

---

### 7. `thinkerMirrorOption.ts` - Reverse Animation Toggle

**Purpose:** Toggle reverse/mirror animation direction.

**File:** `src/patches/thinkerMirrorOption.ts`

**Pattern:**
```regex
reverseMirror:\s*(true|false)
```

**Example:**
```javascript
// Original
reverseMirror: true

// Patched
reverseMirror: false
```

**Configuration Used:**
```typescript
config.thinkingStyle.reverseMirror  // boolean
```

---

### 8. `userMessageDisplay.ts` - User Message Styling

**Purpose:** Customize how user messages are displayed.

**File:** `src/patches/userMessageDisplay.ts`

**Customizations:**
- Format string with `{}` placeholder
- Text styling (bold, italic, underline, strikethrough, inverse)
- Foreground and background colors
- Border style (none, single, double, round, bold, etc.)
- Border color
- Padding (X and Y)
- Box-to-content fitting

**Pattern Example:**
```javascript
// Original user message rendering
return <Text>{message}</Text>

// Patched with styling
return <Box borderStyle="round" borderColor="cyan" paddingX={1}>
  <Text bold color="green">{format.replace("{}", message)}</Text>
</Box>
```

**Configuration Used:**
```typescript
config.userMessageDisplay: {
  format: " > {} ",
  styling: ['bold', 'italic'],
  foregroundColor: 'rgb(0,255,0)',
  backgroundColor: null,
  borderStyle: 'round',
  borderColor: 'rgb(0,255,255)',
  paddingX: 1,
  paddingY: 0,
  fitBoxToContent: true
}
```

---

### 9. `inputBorderBox.ts` - Input Border Toggle

**Purpose:** Remove or modify input box border.

**File:** `src/patches/inputBorderBox.ts`

**Configuration Used:**
```typescript
config.inputBox.removeBorder  // boolean
```

---

### 10. `systemPrompts.ts` - System Prompt Injection

**Purpose:** Replace system prompt fragments with user-customized versions.

**File:** `src/patches/systemPrompts.ts`

**How It Works:**

1. Load markdown prompts from `~/.tweakcc/system-prompts/`
2. Parse frontmatter for metadata and variables
3. Build regex patterns from prompt pieces
4. Find and replace in minified code
5. Handle variable substitution (`${VARIABLE}`)

**Pattern Building:**
```typescript
// From pieces: ["Start ", " middle ", " end"]
// Identifiers: [0, 1]
// IdentifierMap: {0: "VAR1", 1: "VAR2"}

// Generates regex:
/Start ([$\w]+) middle ([$\w]+) end/

// Replacement captures VAR1 and VAR2 positions
```

**Special Handling:**
- Double dollar signs (`$$`) preserved
- Newlines converted to `\n` in strings
- Quotes escaped appropriately
- Non-ASCII optionally escaped

**Test Coverage:** `systemPrompts.test.ts`

---

### 11. `toolsets.ts` - Tool Restrictions

**Purpose:** Restrict available tools based on toolset configuration.

**File:** `src/patches/toolsets.ts`

**Features:**
- Define named tool groups
- Restrict to specific tools or allow all (`'*'`)
- Plan mode toolset switching
- Automatic mode-change detection

**Configuration Used:**
```typescript
config.toolsets: [{
  name: "safe-mode",
  allowedTools: ["read", "write", "bash"]
}]
config.selectedToolset: "safe-mode"
```

**Implementation:**
1. Find tool registration code
2. Inject filter logic
3. Add toolset selection handling

---

### 12. `conversationTitle.ts` - Title Commands

**Purpose:** Add `/title` and `/rename` slash commands.

**File:** `src/patches/conversationTitle.ts`

**Enabled:** Only for versions < 2.0.64 (built-in after)

**Commands Added:**
- `/title <name>` - Set conversation title
- `/rename <name>` - Alias for /title

**Configuration Used:**
```typescript
config.misc.enableConversationTitle  // boolean
```

---

### 13. `contextLimit.ts` - Context Window Override

**Purpose:** Adjust context window size limits.

**File:** `src/patches/contextLimit.ts`

**Applied:** Always (performance optimization)

---

### 14. `modelSelector.ts` - Model Customizations

**Purpose:** Customize model selection and options.

**File:** `src/patches/modelSelector.ts`

**Applied:** Always

---

### 15. `spinnerNoFreeze.ts` - Fix Spinner Animation

**Purpose:** Prevent spinner from freezing during heavy operations.

**File:** `src/patches/spinnerNoFreeze.ts`

**Applied:** Always

**Problem:** Original spinner uses `setInterval` which can freeze during blocking operations.

**Solution:** Patches to ensure animation continues smoothly.

---

### 16. `verboseProperty.ts` - Verbose Logging

**Purpose:** Enable verbose logging for debugging.

**File:** `src/patches/verboseProperty.ts`

**Applied:** Always

---

### 17. `showMoreItemsInSelectMenus.ts` - Menu Item Limit

**Purpose:** Increase visible items in select menus.

**File:** `src/patches/showMoreItemsInSelectMenus.ts`

**Applied:** Always

**Change:**
```javascript
// Original
limit: 10

// Patched
limit: 25
```

---

### 18. `thinkingVisibility.ts` - Expand Thinking Blocks

**Purpose:** Control default visibility of thinking blocks.

**File:** `src/patches/thinkingVisibility.ts`

**Applied:** Always

**Configuration Used:**
```typescript
config.misc.expandThinkingBlocks  // boolean
```

---

### 19. `patchesAppliedIndication.ts` - Version Indicator

**Purpose:** Show tweakcc version and applied patches in Claude Code banner.

**File:** `src/patches/patchesAppliedIndication.ts`

**Example Output:**
```
Claude Code v2.0.76 (tweakcc v3.2.2)
```

**Configuration Used:**
```typescript
config.misc.showVersion        // boolean
config.misc.showPatchesApplied // boolean
```

---

### 20. `hideStartupBanner.ts` - Hide Banner

**Purpose:** Hide Claude Code startup banner.

**File:** `src/patches/hideStartupBanner.ts`

**Configuration Used:**
```typescript
config.misc.hideStartupBanner  // boolean
```

---

### 21. `hideCtrlGToEditPrompt.ts` - Hide Hint

**Purpose:** Hide "Ctrl+G to edit" hint text.

**File:** `src/patches/hideCtrlGToEditPrompt.ts`

**Configuration Used:**
```typescript
config.misc.hideCtrlGToEditPrompt  // boolean
```

---

### 22. `hideStartupClawd.ts` - Hide Logo

**Purpose:** Hide Clawd ASCII art logo on startup.

**File:** `src/patches/hideStartupClawd.ts`

**Configuration Used:**
```typescript
config.misc.hideStartupClawd  // boolean
```

---

### 23. `increaseFileReadLimit.ts` - File Read Limit

**Purpose:** Increase maximum file read size limit.

**File:** `src/patches/increaseFileReadLimit.ts`

**Configuration Used:**
```typescript
config.misc.increaseFileReadLimit  // boolean
```

**Change:** Increases token limit for file reads.

---

### 24. `fixLspSupport.ts` - LSP Protocol Fixes

**Purpose:** Fix Language Server Protocol integration issues.

**File:** `src/patches/fixLspSupport.ts`

**Applied:** Always

---

### 25. `slashCommands.ts` - Slash Command Definitions

**Purpose:** Define and register slash commands.

**File:** `src/patches/slashCommands.ts`

---

## Patch Implementation Patterns

### Standard Patch Function Signature

```typescript
export function write<Feature>(
  content: string,
  config: TweakccConfig
): string | null {
  // Find pattern
  const match = content.match(PATTERN);
  if (!match) {
    return null;  // Pattern not found
  }

  // Build replacement
  const replacement = buildReplacement(config);

  // Apply replacement
  return content.replace(match[0], replacement);
}
```

### Reverse-Order Replacement

When making multiple replacements, process in reverse order to avoid index shifting:

```typescript
const matches = [...content.matchAll(pattern)];

// Sort by index descending
matches.sort((a, b) => b.index - a.index);

// Replace from end to beginning
for (const match of matches) {
  content = content.slice(0, match.index) +
            replacement +
            content.slice(match.index + match[0].length);
}
```

### Variable Capture and Substitution

```typescript
// Original pattern pieces: ["Start ", " middle ", " end"]
// Build regex with capture groups
const regex = /Start ([$\w]+) middle ([$\w]+) end/;

// Match extracts variable names
const match = content.match(regex);
// match[1] = "variableName1"
// match[2] = "variableName2"

// Substitute in replacement
const replacement = userContent
  .replace(/\$\{VAR1\}/g, match[1])
  .replace(/\$\{VAR2\}/g, match[2]);
```

### String Literal Handling

```typescript
function formatStringForJs(content: string, quoteType: 'double' | 'backtick'): string {
  if (quoteType === 'double') {
    // Convert newlines to \n
    content = content.replace(/\n/g, '\\n');
    // Escape quotes
    content = content.replace(/"/g, '\\"');
  }
  // Escape single quotes
  content = content.replace(/'/g, "\\'");
  // Preserve $$ (double dollar signs)
  // ...
  return content;
}
```

---

## Error Handling

### Graceful Failure

Each patch returns `null` if the target pattern is not found:

```typescript
const result = writeThemes(content, config);
if (result === null) {
  debug('Theme patch: pattern not found, skipping');
  // Continue with other patches
} else {
  content = result;
}
```

### Debug Output

Use `showDiff()` for debugging:

```typescript
if (debugEnabled) {
  showDiff(
    originalContent,
    patchedContent,
    injectedText,
    match.index,
    match.index + match[0].length
  );
}
```

---

## Testing Patches

### Unit Test Pattern

```typescript
// From systemPrompts.test.ts
describe('systemPrompts patch', () => {
  it('should preserve double dollar signs', () => {
    const input = 'Timeout: J$$() ms';
    const result = applyPatch(input, config);
    expect(result).toContain('J$$');  // Not J$
  });

  it('should escape newlines in double-quoted strings', () => {
    const input = 'Line 1\nLine 2';
    const result = formatForDoubleQuotes(input);
    expect(result).toBe('Line 1\\nLine 2');
  });
});
```

### Integration Testing

Test with actual Claude Code cli.js:

1. Create backup
2. Apply patches
3. Verify Claude Code still runs
4. Check customizations appear correctly

---

## Patch Compatibility

### Version Handling

Some patches are version-specific:

```typescript
// conversationTitle.ts
if (compareSemverVersions(version, '2.0.64') >= 0) {
  // Skip - built into this version
  return null;
}
```

### Minification Changes

Patches use flexible patterns to handle:
- Variable name changes
- Whitespace variations
- Module system differences (esbuild vs Bun)

---

## Adding New Patches

### Steps

1. Create `src/patches/myPatch.ts`:

```typescript
import { TweakccConfig } from '../types';

const PATTERN = /your pattern here/;

export function writeMyPatch(
  content: string,
  config: TweakccConfig
): string | null {
  const match = content.match(PATTERN);
  if (!match) return null;

  const replacement = buildReplacement(config);
  return content.replace(match[0], replacement);
}
```

2. Export from `patches/index.ts`:

```typescript
import { writeMyPatch } from './myPatch';

// In applyCustomization():
content = writeMyPatch(content, config) ?? content;
```

3. Add configuration type if needed (`types.ts`)

4. Add UI controls if needed (`ui/components/`)

5. Write tests (`tests/myPatch.test.ts`)

### Best Practices

- Use specific patterns to avoid false matches
- Return `null` instead of throwing on pattern not found
- Include debug logging
- Test with multiple Claude Code versions
- Handle edge cases (special characters, Unicode)
