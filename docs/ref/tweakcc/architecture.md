# tweakcc Architecture

> System Design, Data Flow, and Design Decisions

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              tweakcc CLI                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐                   │
│  │   CLI Entry  │───>│   Startup    │───>│    Config    │                   │
│  │  (index.tsx) │    │  Detection   │    │  Management  │                   │
│  └──────────────┘    └──────────────┘    └──────────────┘                   │
│         │                   │                   │                            │
│         │                   ▼                   ▼                            │
│         │           ┌──────────────┐    ┌──────────────┐                    │
│         │           │ Installation │    │   Default    │                    │
│         │           │  Detection   │    │   Settings   │                    │
│         │           └──────────────┘    └──────────────┘                    │
│         │                   │                                                │
│         ▼                   ▼                                                │
│  ┌──────────────────────────────────────────────────────────────────┐       │
│  │                      Interactive UI (Ink/React)                   │       │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐    │       │
│  │  │ Themes  │ │Thinking │ │ User Msg│ │Toolsets │ │  Misc   │    │       │
│  │  │  View   │ │  Views  │ │ Display │ │  View   │ │  View   │    │       │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘    │       │
│  └──────────────────────────────────────────────────────────────────┘       │
│         │                                                                    │
│         ▼                                                                    │
│  ┌──────────────────────────────────────────────────────────────────┐       │
│  │                        Patch Engine                               │       │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐    │       │
│  │  │ Themes  │ │ Verbs   │ │ Prompts │ │Toolsets │ │  Misc   │    │       │
│  │  │ Patch   │ │ Patch   │ │ Patch   │ │ Patch   │ │ Patches │    │       │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘    │       │
│  └──────────────────────────────────────────────────────────────────┘       │
│         │                                                                    │
│         ▼                                                                    │
│  ┌──────────────────────────────────────────────────────────────────┐       │
│  │                     Claude Code Installation                      │       │
│  │  ┌──────────────────┐         ┌──────────────────┐               │       │
│  │  │   cli.js (npm)   │   OR    │ Native Binary    │               │       │
│  │  └──────────────────┘         └──────────────────┘               │       │
│  └──────────────────────────────────────────────────────────────────┘       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### Startup Flow

```
User runs `tweakcc`
        │
        ▼
┌───────────────────┐
│  Parse CLI Args   │  (Commander.js)
│  --debug, --apply │
└───────────────────┘
        │
        ▼
┌───────────────────┐
│  Read Config File │  (config.ts)
│  with migrations  │
└───────────────────┘
        │
        ▼
┌───────────────────┐
│ Find Claude Code  │  (installationDetection.ts)
│   Installation    │
└───────────────────┘
        │
        ├─────────────────────┐
        │                     │
        ▼                     ▼
┌───────────────────┐  ┌───────────────────┐
│  Single Install   │  │ Multiple Installs │
│  Found            │  │   Found           │
└───────────────────┘  └───────────────────┘
        │                     │
        │                     ▼
        │              ┌───────────────────┐
        │              │ Show Installation │
        │              │     Picker UI     │
        │              └───────────────────┘
        │                     │
        ├─────────────────────┘
        ▼
┌───────────────────┐
│  Create Backup    │  (installationBackup.ts)
│  if missing       │
└───────────────────┘
        │
        ▼
┌───────────────────┐
│ Sync System       │  (systemPromptSync.ts)
│ Prompts (async)   │
└───────────────────┘
        │
        ├─────────────────────┐
        │                     │
        ▼                     ▼
┌───────────────────┐  ┌───────────────────┐
│  --apply flag     │  │ Interactive Mode  │
│  Non-interactive  │  │ Show Main Menu    │
└───────────────────┘  └───────────────────┘
        │                     │
        ▼                     ▼
┌───────────────────┐  ┌───────────────────┐
│ Apply Patches     │  │  User Configures  │
│ Immediately       │  │  Settings in UI   │
└───────────────────┘  └───────────────────┘
```

### Patch Application Flow

```
User selects "Apply customizations"
        │
        ▼
┌───────────────────┐
│ Restore from      │  Start from clean backup
│ Backup            │
└───────────────────┘
        │
        ▼
┌───────────────────┐
│ Load System       │  Load user-edited prompts
│ Prompts           │
└───────────────────┘
        │
        ▼
┌────────────────────────────────────────────────┐
│              Apply Patches in Order             │
├────────────────────────────────────────────────┤
│  1. System Prompts                              │
│  2. Themes (colors, switch statements)          │
│  3. Thinking Verbs + Format                     │
│  4. Thinking Style (phases, speed, mirror)      │
│  5. User Message Display                        │
│  6. Input Box Border                            │
│  7. Verbose Property (always)                   │
│  8. Spinner No-Freeze (always)                  │
│  9. Context Limit (always)                      │
│ 10. Model Selector (always)                     │
│ 11. Show More Items (always)                    │
│ 12. Thinking Visibility (always)                │
│ 13. Patches Applied Indication                  │
│ 14. LSP Support Fixes (always)                  │
│ 15. Toolsets (if configured)                    │
│ 16. Conversation Title (if enabled, v < 2.0.64)│
│ 17. Hide Startup Banner (if enabled)            │
│ 18. Hide Ctrl-G (if enabled)                    │
│ 19. Hide Clawd (if enabled)                     │
│ 20. Increase File Read Limit (if enabled)       │
└────────────────────────────────────────────────┘
        │
        ▼
┌───────────────────┐
│ Write Modified    │  (with hard-link handling)
│ Content Back      │
└───────────────────┘
        │
        ▼
┌───────────────────┐
│ Update Config     │  changesApplied = true
│ changesApplied    │
└───────────────────┘
```

### System Prompt Sync Flow

```
On startup or explicit sync
        │
        ▼
┌───────────────────┐
│ Get CC Version    │  From installation detection
└───────────────────┘
        │
        ▼
┌───────────────────┐
│ Check Local Cache │  ~/.tweakcc/prompt-data-cache/
└───────────────────┘
        │
        ├─── Cache Hit ──────────────┐
        │                            │
        ▼                            ▼
┌───────────────────┐        ┌───────────────────┐
│ Download from     │        │  Use Cached       │
│ GitHub            │        │  prompts-X.Y.Z.json│
└───────────────────┘        └───────────────────┘
        │                            │
        ├────────────────────────────┘
        ▼
┌───────────────────┐
│ Parse StringsFile │  Extract pieces + identifiers
└───────────────────┘
        │
        ▼
┌───────────────────┐
│ Reconstruct       │  Join pieces with variable placeholders
│ Full Prompts      │
└───────────────────┘
        │
        ▼
┌───────────────────────────────────────────────────┐
│  For each prompt in StringsFile                    │
├───────────────────────────────────────────────────┤
│  ┌─────────────────┐                              │
│  │ Check if .md    │                              │
│  │ file exists     │                              │
│  └─────────────────┘                              │
│          │                                         │
│          ├── No ──> Create new .md file           │
│          │                                         │
│          ▼                                         │
│  ┌─────────────────┐                              │
│  │ Compare hashes  │  original vs applied vs user │
│  └─────────────────┘                              │
│          │                                         │
│          ├── Unchanged ──> Skip                   │
│          │                                         │
│          ├── User modified only ──> Keep user     │
│          │                                         │
│          ├── Upstream changed ──> Update          │
│          │                                         │
│          └── Both changed ──> CONFLICT            │
│                      │                             │
│                      ▼                             │
│              Generate HTML diff file              │
└───────────────────────────────────────────────────┘
```

## Module Dependency Graph

```
                    ┌─────────────┐
                    │  index.tsx  │  (CLI Entry)
                    └──────┬──────┘
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
        ▼                  ▼                  ▼
┌───────────────┐  ┌───────────────┐  ┌───────────────┐
│   startup.ts  │  │   config.ts   │  │   ui/App.tsx  │
└───────┬───────┘  └───────┬───────┘  └───────┬───────┘
        │                  │                  │
        │                  ▼                  │
        │          ┌───────────────┐          │
        │          │   types.ts    │<─────────┤
        │          └───────────────┘          │
        │                  ▲                  │
        │                  │                  │
        ▼                  │                  ▼
┌─────────────────────┐    │         ┌───────────────────┐
│ installationDetect. │    │         │  ui/components/*  │
└─────────┬───────────┘    │         └─────────┬─────────┘
          │                │                   │
          ▼                │                   │
┌─────────────────────┐    │                   │
│ installationPaths.ts│    │                   │
└─────────────────────┘    │                   │
          │                │                   │
          ▼                │                   │
┌─────────────────────┐    │                   │
│installationBackup.ts│    │                   │
└─────────────────────┘    │                   │
          │                │                   │
          │                │                   │
          ▼                ▼                   ▼
┌──────────────────────────────────────────────────────┐
│                    patches/index.ts                   │
│   ┌──────────────┬──────────────┬──────────────┐     │
│   │  themes.ts   │ thinker*.ts  │ systemPrompts│     │
│   ├──────────────┼──────────────┼──────────────┤     │
│   │  toolsets.ts │ userMessage  │ misc patches │     │
│   └──────────────┴──────────────┴──────────────┘     │
└──────────────────────────────────────────────────────┘
          │                          ▲
          │                          │
          ▼                          │
┌─────────────────────┐              │
│ systemPromptSync.ts │──────────────┘
└─────────┬───────────┘
          │
          ▼
┌─────────────────────┐
│systemPromptDownload │
└─────────┬───────────┘
          │
          ▼
┌─────────────────────┐
│systemPromptHashIndex│
└─────────────────────┘
```

## Design Decisions

### 1. Regex-Based Patching (vs AST-Based)

**Decision:** Use regex patterns to find and replace code sections.

**Rationale:**
- Claude Code's minified JavaScript changes frequently with updates
- AST parsing would require updating complex transformations for each minification style
- Regex patterns can adapt to variable name changes through pattern matching
- Faster execution (no full parse required)
- Easier to debug and visualize changes

**Trade-offs:**
- Less precise than AST manipulation
- Requires careful pattern design to avoid false matches
- Must handle edge cases (e.g., `$$` in strings)

### 2. Backup-First Approach

**Decision:** Always restore from backup before applying patches.

**Rationale:**
- Ensures clean state regardless of previous patch state
- Eliminates issues with cumulative patches
- Simplifies rollback (just restore backup)
- Allows patches to be idempotent

**Implementation:**
```typescript
// Always start from backup
const originalContent = await readBackupFile();
let patchedContent = originalContent;

// Apply all patches
patchedContent = applyThemes(patchedContent, config);
patchedContent = applyVerbs(patchedContent, config);
// ... more patches

// Write result
await writeFile(installPath, patchedContent);
```

### 3. XDG Base Directory Compliance

**Decision:** Support XDG specification while maintaining backward compatibility.

**Priority Order:**
1. Explicit env var (`TWEAKCC_CONFIG_DIR`)
2. Legacy location (`~/.tweakcc`) if exists
3. Claude ecosystem (`~/.claude/tweakcc`)
4. XDG standard (`$XDG_CONFIG_HOME/tweakcc`)
5. Fallback (`~/.tweakcc`)

**Rationale:**
- Respects user preferences
- Maintains backward compatibility for existing users
- Aligns with Claude ecosystem conventions
- Follows Linux standards

### 4. Hash-Based Conflict Detection

**Decision:** Use MD5 hashes to detect prompt modifications.

**Three-Way Comparison:**
1. **Original Hash:** Hash of prompt when downloaded from GitHub
2. **Applied Hash:** Hash of prompt when last applied to installation
3. **Current Content:** Current content of user's markdown file

**Conflict Matrix:**

| Original | Applied | Current | Action |
|----------|---------|---------|--------|
| A | A | A | Skip (unchanged) |
| A | A | B | Keep user change |
| A | B | A | Update (upstream changed) |
| A | B | B | Update (already applied) |
| A | B | C | **CONFLICT** |

### 5. Hard Link Handling (Bun Support)

**Decision:** Use unlink/write/chmod pattern for file modifications.

**Problem:** Bun caches packages using hard links. Modifying a hard-linked file affects all links.

**Solution:**
```typescript
async function replaceFileBreakingHardLinks(filePath, newContent) {
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

### 6. Native Binary Support

**Decision:** Use `node-lief` for binary manipulation with graceful fallback.

**Approach:**
- Dynamically load `node-lief` at runtime
- If unavailable (NixOS, missing libraries), gracefully disable native support
- Extract embedded JavaScript from Mach-O/ELF binaries
- Patch JavaScript, then repack binary

**Trade-offs:**
- Adds optional dependency
- Increases complexity
- Enables support for non-npm installations

### 7. React/Ink for Terminal UI

**Decision:** Use React with Ink framework for terminal rendering.

**Rationale:**
- Component-based architecture for complex UI
- State management with React Context
- Familiar patterns for web developers
- Rich ecosystem of terminal UI components
- Easy to test and maintain

**Key Patterns:**
- `SettingsContext` for global state
- Controlled components for inputs
- Keyboard event handling via `useInput`
- Custom hooks for side effects

### 8. Version-Specific Prompt Data

**Decision:** Maintain separate prompt JSON files per Claude Code version.

**Structure:**
```
data/prompts/
├── prompts-2.0.14.json
├── prompts-2.0.15.json
├── ...
└── prompts-2.0.76.json
```

**Rationale:**
- Prompts change between versions
- Enables precise matching
- Allows patching older installations
- Simplifies conflict detection

### 9. Extension Pattern for Code Organization

**Decision:** Keep patches modular and self-contained.

**Pattern:**
```
patches/
├── index.ts          # Orchestrator
├── themes.ts         # Theme-related patches
├── thinkerVerbs.ts   # Verb patches
├── thinkerFormat.ts  # Format patches
└── ...               # More specialized patches
```

**Each patch module:**
- Exports a single patch function
- Returns null if pattern not found
- Handles its own regex patterns
- Is independently testable

## Error Handling Strategy

### Graceful Degradation

```
┌─────────────────────────────────────────┐
│           Error Handling Levels          │
├─────────────────────────────────────────┤
│  Level 1: Warn and continue             │
│  - Missing optional dependency          │
│  - Non-critical patch failure           │
│                                          │
│  Level 2: Fall back to alternative      │
│  - Native binary support unavailable    │
│  - Config migration failure             │
│                                          │
│  Level 3: Prompt user for action        │
│  - Multiple installations found         │
│  - Conflict detected in prompts         │
│                                          │
│  Level 4: Fail with clear message       │
│  - Installation not found               │
│  - Backup corruption                    │
│  - Permission denied                    │
└─────────────────────────────────────────┘
```

### Error Messages

All errors include:
1. Clear description of what went wrong
2. Likely cause
3. Suggested fix (if applicable)
4. Relevant file paths or values

## Performance Considerations

### Startup Optimization

1. **Lazy Loading:** `node-lief` loaded only when needed
2. **Cached Prompts:** Downloaded prompts cached locally
3. **Async Prompt Sync:** Doesn't block UI startup
4. **Parallel Search:** Multiple search paths checked concurrently

### Patch Optimization

1. **Single Pass:** All patches applied in one file read/write cycle
2. **Reverse Order:** Replacements done in reverse index order to avoid shifting
3. **Early Exit:** Patches skip if pattern not found
4. **Minimal Regex:** Patterns optimized for speed

## Security Considerations

### File Permissions

- Backups preserve original permissions
- Config directory created with user-only access
- `.gitignore` excludes sensitive files

### Input Validation

- Color formats validated before use
- Paths resolved and checked before access
- Variable names sanitized in prompts

### No Network on Apply

- Patches applied from local cache only
- Network only used for initial prompt download
- User can work offline after first sync
