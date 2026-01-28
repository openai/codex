# Skill/Slash Command Unification

## Core Insight

**Skills are NOT a separate runtime system—they are prompt-type commands that participate in the unified slash-command pipeline.**

This document describes how skills and slash commands are unified into a single command system, based on Claude Code v2.1.7 reference implementation.

---

## Unified Command Architecture

```
                    ┌─────────────────────────────────────┐
                    │         SlashCommand Union          │
                    │  LocalCommand | PromptCommand       │
                    └─────────────────┬───────────────────┘
                                      │
              ┌───────────────────────┼───────────────────────┐
              │                       │                       │
      ┌───────▼───────┐       ┌───────▼───────┐       ┌───────▼───────┐
      │  LocalCommand │       │ PromptCommand │       │ LocalJsxCmd   │
      │  type:'local' │       │ type:'prompt' │       │ type:'local-  │
      │  /help, /clear│       │               │       │      jsx'     │
      └───────────────┘       └───────┬───────┘       └───────────────┘
                                      │
                              ┌───────▼───────┐
                              │ SkillPrompt   │
                              │ Command       │  ← All skills are this
                              │ /commit, etc  │
                              └───────────────┘
```

**Key insight:** All skills are `PromptCommand` variants. The skill system is not a separate subsystem—it's a classification within the unified command system.

---

## Type Hierarchy

### SlashCommand (Union Type)

```rust
/// Unified command type - all commands use this
pub enum SlashCommand {
    /// Built-in commands: /help, /clear, /compact, etc.
    Local(LocalCommand),
    /// Skills and custom commands (all prompt-based)
    Prompt(PromptCommand),
    /// JSX-rendered commands (UI components)
    LocalJsx(LocalJsxCommand),
}

impl SlashCommand {
    /// Get command name
    pub fn name(&self) -> &str {
        match self {
            Self::Local(c) => &c.name,
            Self::Prompt(c) => &c.name,
            Self::LocalJsx(c) => &c.name,
        }
    }

    /// Check if command is enabled
    pub fn is_enabled(&self) -> bool {
        match self {
            Self::Local(c) => c.is_enabled,
            Self::Prompt(c) => !c.is_hidden,
            Self::LocalJsx(c) => c.is_enabled,
        }
    }

    /// Get command type string
    pub fn command_type(&self) -> &str {
        match self {
            Self::Local(_) => "local",
            Self::Prompt(_) => "prompt",
            Self::LocalJsx(_) => "local-jsx",
        }
    }
}
```

### SkillPromptCommand (All Skills)

```rust
/// All skills are PromptCommands with additional metadata
/// This is the "skill" variant of PromptCommand
pub struct SkillPromptCommand {
    // ─────────────────────────────────────────────────────
    // Base fields (from PromptCommand)
    // ─────────────────────────────────────────────────────
    pub name: String,
    pub description: String,
    pub command_type: CommandType,  // Always CommandType::Prompt

    // ─────────────────────────────────────────────────────
    // Classification flags (KEY TO UNIFICATION)
    // ─────────────────────────────────────────────────────
    /// Can user invoke via /command
    pub user_invocable: bool,
    /// Block LLM from invoking via Skill tool
    pub disable_model_invocation: bool,
    /// Computed from user_invocable - controls /help visibility
    pub is_hidden: bool,
    /// Mark as skill (vs regular prompt command)
    pub is_skill: bool,

    // ─────────────────────────────────────────────────────
    // Source tracking
    // ─────────────────────────────────────────────────────
    /// Where the skill was defined (configuration source)
    pub source: SkillSource,
    /// How the skill was loaded (file format)
    pub loaded_from: LoadedFrom,

    // ─────────────────────────────────────────────────────
    // Execution configuration
    // ─────────────────────────────────────────────────────
    /// Execution context: main (inline) or fork (subagent)
    pub context: SkillContext,
    /// Agent type for fork context
    pub agent: Option<String>,
    /// Model override: haiku, sonnet, opus, inherit
    pub model: Option<String>,
    /// Tool restrictions
    pub allowed_tools: Option<Vec<String>>,
    /// Skill directory for relative paths
    pub base_dir: Option<PathBuf>,

    // ─────────────────────────────────────────────────────
    // Metadata
    // ─────────────────────────────────────────────────────
    /// Guidance for LLM on when to invoke
    pub when_to_use: Option<String>,
    /// Usage hint shown in /help
    pub argument_hint: Option<String>,
    /// Alternative command names
    pub aliases: Vec<String>,
    /// Skill-level hooks configuration
    pub hooks: Option<SkillHooksConfig>,

    // ─────────────────────────────────────────────────────
    // Content
    // ─────────────────────────────────────────────────────
    /// The prompt content (markdown)
    pub content: String,
}

/// Command type enum
pub enum CommandType {
    Local,     // Native handler
    Prompt,    // Prompt-based (all skills)
    LocalJsx,  // JSX-rendered
}
```

---

## Classification Flags

The classification flags determine how a skill can be invoked and where it appears:

| Flag | Default | Effect |
|------|---------|--------|
| `user_invocable` | `true` | When `false`, blocks `/skillname` invocation |
| `disable_model_invocation` | `false` | When `true`, LLM cannot invoke via Skill tool |
| `is_hidden` | computed | `!user_invocable` - controls /help visibility |
| `is_skill` | `true` | Marks as skill (vs regular prompt command) |

### Flag Interactions

```rust
impl SkillPromptCommand {
    /// Compute is_hidden from user_invocable
    pub fn compute_is_hidden(&mut self) {
        self.is_hidden = !self.user_invocable;
    }

    /// Check if user can invoke this skill
    pub fn is_user_invocable(&self) -> bool {
        self.user_invocable && !self.is_hidden
    }

    /// Check if LLM can invoke this skill
    pub fn is_llm_invocable(&self) -> bool {
        !self.disable_model_invocation
            && self.source != SkillSource::Builtin
            && (self.description.len() > 0 || self.when_to_use.is_some())
    }

    /// Check if visible in /help
    pub fn is_visible_in_help(&self) -> bool {
        !self.is_hidden
            && self.source != SkillSource::Builtin
            && (self.description.len() > 0 || self.when_to_use.is_some())
    }
}
```

### Common Flag Combinations

| Scenario | user_invocable | disable_model_invocation | Result |
|----------|----------------|--------------------------|--------|
| Normal skill | true | false | User + LLM can invoke |
| LLM-only skill | false | false | Only LLM can invoke |
| User-only skill | true | true | Only user can invoke |
| Disabled skill | false | true | No one can invoke |

---

## Source Tracking

### SkillSource (Configuration Source)

```rust
/// Where the skill was defined (configuration source)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSource {
    /// Hardcoded in binary (not user-modifiable)
    Builtin,
    /// Bundled skills: ~/.claude/skills-bundled/
    Bundled,
    /// Managed/policy settings: ~/.claude/skills/ (highest priority)
    PolicySettings,
    /// User config directory skills
    UserSettings,
    /// Project-local: ./.claude/skills/
    ProjectSettings,
    /// From installed plugin
    Plugin,
    /// From MCP server
    Mcp,
}

impl SkillSource {
    /// Get priority (higher = takes precedence)
    pub fn priority(&self) -> i32 {
        match self {
            Self::Builtin => 0,
            Self::Bundled => 1,
            Self::Mcp => 2,
            Self::Plugin => 3,
            Self::ProjectSettings => 4,
            Self::UserSettings => 5,
            Self::PolicySettings => 6,  // Highest
        }
    }
}
```

### LoadedFrom (File Format)

```rust
/// How the skill was loaded (file format)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadedFrom {
    /// Hardcoded in binary
    Builtin,
    /// SKILL.md in skill directory (modern format)
    Skills,
    /// Plugin manifest
    Plugin,
    /// Bundled skill files
    Bundled,
    /// Legacy .claude/commands/*.md (deprecated, not supported in cocode)
    CommandsDeprecated,
}
```

---

## Filtering Functions

### Unified Command Aggregation

```rust
/// Get all enabled commands (unified aggregation)
pub async fn get_all_commands(ctx: &CommandContext) -> Vec<SlashCommand> {
    let mut commands = Vec::new();

    // 1. Bundled skills (~/.claude/skills-bundled/)
    commands.extend(load_bundled_skills().await);

    // 2. Skill directory commands (priority order)
    commands.extend(load_skill_directory_commands(ctx).await);

    // 3. Plugin commands
    commands.extend(get_plugin_commands(ctx).await);

    // 4. Plugin skills
    commands.extend(get_plugin_skills(ctx).await);

    // 5. MCP prompts
    commands.extend(get_mcp_prompts(ctx).await);

    // 6. Built-in commands (local handlers)
    commands.extend(get_builtin_commands());

    // Filter enabled only and deduplicate
    commands
        .into_iter()
        .filter(|c| c.is_enabled())
        .collect::<Vec<_>>()
        .deduplicate_by_name()
}

/// Load skills from directory with priority
async fn load_skill_directory_commands(ctx: &CommandContext) -> Vec<SlashCommand> {
    let mut skills = Vec::new();
    let mut seen_inodes = HashSet::new();

    // Load in priority order (highest first)
    let dirs = [
        (ctx.policy_skills_dir(), SkillSource::PolicySettings),
        (ctx.user_skills_dir(), SkillSource::UserSettings),
        (ctx.project_skills_dir(), SkillSource::ProjectSettings),
    ];

    for (dir, source) in dirs {
        if let Some(dir) = dir {
            let loaded = load_skills_from_dir(&dir, source, &mut seen_inodes).await;
            skills.extend(loaded.into_iter().map(SlashCommand::Prompt));
        }
    }

    skills
}
```

### LLM-Invocable Skills Filter

```rust
/// Get skills that LLM can invoke via Skill tool
pub async fn get_llm_invocable_skills(ctx: &CommandContext) -> Vec<SkillPromptCommand> {
    get_all_commands(ctx)
        .await
        .into_iter()
        .filter_map(|c| match c {
            SlashCommand::Prompt(pc) => Some(pc),
            _ => None,
        })
        .filter(|pc| {
            // Must not be disabled for model invocation
            !pc.disable_model_invocation
            // Must not be builtin (those have dedicated tools)
            && pc.source != SkillSource::Builtin
            // Must have description or when_to_use for LLM context
            && (pc.description.len() > 0 || pc.when_to_use.is_some())
        })
        .collect()
}
```

### User-Visible Skills Filter

```rust
/// Get skills visible to user in /help
pub async fn get_user_skills(ctx: &CommandContext) -> Vec<SkillPromptCommand> {
    get_all_commands(ctx)
        .await
        .into_iter()
        .filter_map(|c| match c {
            SlashCommand::Prompt(pc) => Some(pc),
            _ => None,
        })
        .filter(|pc| {
            // Must not be builtin (those are shown separately)
            pc.source != SkillSource::Builtin
            // Must not be hidden
            && !pc.is_hidden
            // Must have description or when_to_use for display
            && (pc.description.len() > 0 || pc.when_to_use.is_some())
        })
        .collect()
}
```

### Skill Lookup

```rust
/// Find skill by name or alias
pub async fn find_skill(ctx: &CommandContext, name: &str) -> Option<SlashCommand> {
    let name_lower = name.to_lowercase();

    get_all_commands(ctx)
        .await
        .into_iter()
        .find(|c| {
            // Match by name
            c.name().to_lowercase() == name_lower
            // Or match by alias
            || match c {
                SlashCommand::Prompt(pc) => {
                    pc.aliases.iter().any(|a| a.to_lowercase() == name_lower)
                }
                _ => false,
            }
        })
}
```

---

## Unified Execution Pipeline

All commands (skills and local commands) flow through a single execution pipeline:

```
User Input: /commit -m "fix bug"
              │
              ▼
┌─────────────────────────────┐
│  1. Command Parser          │  Parse command name and args
│     - Extract: "commit"     │
│     - Args: "-m \"fix bug\"" │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│  2. Command Lookup          │  Find in get_all_commands()
│     - Search by name        │
│     - Search by alias       │
│     - Check is_enabled()    │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│  3. Permission Check        │  Verify invocation allowed
│     - User: user_invocable  │
│     - LLM: !disable_model_  │
│            invocation       │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│  4. Type Dispatch           │
│  ┌─────────────────────────┐│
│  │ local → execute_local() ││
│  │ prompt → execute_prompt()│← All skills
│  │ local-jsx → render_jsx()││
│  └─────────────────────────┘│
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│  5. Prompt Execution        │  (for skills)
│     - Build prompt content  │
│     - Inject $ARGUMENTS     │
│     - Apply base_dir prefix │
│     - Register skill hooks  │
│     - Choose context:       │
│       main → inline         │
│       fork → subagent       │
│     - Execute with LLM      │
└─────────────────────────────┘
```

### Execute Prompt Command

```rust
impl CommandExecutor {
    /// Execute a prompt command (skill)
    pub async fn execute_prompt(
        &mut self,
        command: &SkillPromptCommand,
        args: &str,
    ) -> Result<ExecutionResult, CommandError> {
        // 1. Build final prompt content
        let content = self.build_prompt_content(command, args);

        // 2. Apply base_dir prefix to relative paths
        let content = if let Some(base_dir) = &command.base_dir {
            self.resolve_relative_paths(&content, base_dir)
        } else {
            content
        };

        // 3. Register skill-level hooks
        if let Some(hooks_config) = &command.hooks {
            self.register_skill_hooks(hooks_config, &command.name);
        }

        // 4. Execute based on context
        match command.context {
            SkillContext::Main => {
                // Inline execution in current conversation
                self.execute_inline(&content, command).await
            }
            SkillContext::Fork => {
                // Subagent execution
                self.execute_forked(&content, command).await
            }
        }
    }

    /// Build prompt content with argument injection
    fn build_prompt_content(&self, command: &SkillPromptCommand, args: &str) -> String {
        let args = args.trim();

        if command.content.contains("$ARGUMENTS") {
            // Replace placeholder
            command.content.replace("$ARGUMENTS", args)
        } else if !args.is_empty() {
            // Append as ARGUMENTS section
            format!("{}\n\nARGUMENTS: {}", command.content, args)
        } else {
            command.content.clone()
        }
    }
}
```

---

## Skill Tool Integration

The Skill tool allows LLM to invoke skills programmatically:

```rust
/// Skill tool definition (for LLM invocation)
pub struct SkillTool {
    /// Available skills for this context
    available_skills: Vec<SkillPromptCommand>,
}

impl Tool for SkillTool {
    fn name(&self) -> &str {
        "Skill"
    }

    fn description(&self) -> String {
        let skill_list = self.available_skills
            .iter()
            .map(|s| format!("- {}: {}", s.name, s.description))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "Execute a skill within the main conversation.\n\n\
             Available skills:\n{}\n\n\
             Usage: skill: \"<skill-name>\", args: \"<arguments>\"",
            skill_list
        )
    }

    async fn execute(&self, input: SkillToolInput) -> Result<ToolResult, ToolError> {
        // 1. Find skill by name
        let skill = self.available_skills
            .iter()
            .find(|s| s.name == input.skill || s.aliases.contains(&input.skill))
            .ok_or_else(|| ToolError::skill_not_found(&input.skill))?;

        // 2. Check LLM can invoke
        if skill.disable_model_invocation {
            return Err(ToolError::skill_disabled_for_model(&input.skill));
        }

        // 3. Execute skill
        self.executor.execute_prompt(skill, &input.args.unwrap_or_default()).await
    }
}

#[derive(Debug, Deserialize)]
pub struct SkillToolInput {
    pub skill: String,
    pub args: Option<String>,
}
```

---

## Deduplication

Skills are deduplicated by inode to handle symlinks and avoid duplicate loading:

```rust
/// Inode-based deduplication for skill loading
pub struct SkillDeduplicator {
    seen_inodes: HashSet<u64>,
}

impl SkillDeduplicator {
    pub fn new() -> Self {
        Self {
            seen_inodes: HashSet::new(),
        }
    }

    /// Check if path already seen (by inode)
    pub fn is_duplicate(&mut self, path: &Path) -> bool {
        if let Ok(metadata) = std::fs::metadata(path) {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                let inode = metadata.ino();
                if self.seen_inodes.contains(&inode) {
                    return true;
                }
                self.seen_inodes.insert(inode);
            }
        }
        false
    }
}

/// Name-based deduplication for final command list
trait DeduplicateByName {
    fn deduplicate_by_name(self) -> Self;
}

impl DeduplicateByName for Vec<SlashCommand> {
    fn deduplicate_by_name(self) -> Self {
        let mut seen = HashSet::new();
        self.into_iter()
            .filter(|c| seen.insert(c.name().to_string()))
            .collect()
    }
}
```

---

---

## Safety Mechanisms

### Field Length Limits

```rust
// Field length limits (DoS protection)
pub const MAX_NAME_LEN: usize = 64;
pub const MAX_DESCRIPTION_LEN: usize = 1024;
pub const MAX_SHORT_DESCRIPTION_LEN: usize = 1024;
pub const MAX_WHEN_TO_USE_LEN: usize = 1024;
pub const MAX_ARGUMENT_HINT_LEN: usize = 256;

// Traversal limits (DoS protection)
pub const MAX_SCAN_DEPTH: usize = 6;
pub const MAX_SKILLS_DIRS_PER_ROOT: usize = 2000;

// Content limits (already defined in features.md)
pub const SKILL_PROMPT_MAX_CHARS: usize = 15000;
```

### Validation

```rust
/// Skill validation errors
#[derive(Debug, Clone)]
pub enum SkillValidationError {
    NameTooLong(usize),
    DescriptionTooLong,
    WhenToUseTooLong,
    ArgumentHintTooLong,
    ContentTooLong,
}

impl SkillPromptCommand {
    /// Validate field lengths before loading
    pub fn validate(&self) -> Result<(), SkillValidationError> {
        if self.name.len() > MAX_NAME_LEN {
            return Err(SkillValidationError::NameTooLong(self.name.len()));
        }
        if self.description.len() > MAX_DESCRIPTION_LEN {
            return Err(SkillValidationError::DescriptionTooLong);
        }
        if let Some(when) = &self.when_to_use {
            if when.len() > MAX_WHEN_TO_USE_LEN {
                return Err(SkillValidationError::WhenToUseTooLong);
            }
        }
        if let Some(hint) = &self.argument_hint {
            if hint.len() > MAX_ARGUMENT_HINT_LEN {
                return Err(SkillValidationError::ArgumentHintTooLong);
            }
        }
        if self.content.len() > SKILL_PROMPT_MAX_CHARS {
            return Err(SkillValidationError::ContentTooLong);
        }
        Ok(())
    }
}
```

### Symlink Safety and Cycle Detection

```rust
/// Safe directory traversal with cycle detection
pub struct SkillScanner {
    /// Visited canonical paths for cycle detection
    visited: HashSet<PathBuf>,
    /// Directory count for DoS protection
    dir_count: usize,
    /// Maximum scan depth
    max_depth: usize,
}

impl SkillScanner {
    pub fn new() -> Self {
        Self {
            visited: HashSet::new(),
            dir_count: 0,
            max_depth: MAX_SCAN_DEPTH,
        }
    }

    /// Scan directory for skills with safety checks
    pub fn scan_directory(&mut self, path: &Path, depth: usize) -> Vec<SkillPath> {
        // Depth limit
        if depth > self.max_depth {
            return vec![];
        }

        // Directory count limit (DoS protection)
        if self.dir_count >= MAX_SKILLS_DIRS_PER_ROOT {
            return vec![];
        }
        self.dir_count += 1;

        // Symlink cycle detection via canonical path
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return vec![],  // Cannot resolve, skip
        };
        if !self.visited.insert(canonical.clone()) {
            return vec![];  // Cycle detected, skip
        }

        // Skip hidden directories
        if path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.'))
            .unwrap_or(false)
        {
            return vec![];
        }

        // Continue scanning for SKILL.md files...
        let mut results = vec![];

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();

                if entry_path.is_dir() {
                    // Recurse into subdirectories
                    results.extend(self.scan_directory(&entry_path, depth + 1));
                } else if entry_path.file_name() == Some(std::ffi::OsStr::new("SKILL.md")) {
                    results.push(SkillPath {
                        skill_dir: path.to_path_buf(),
                        skill_file: entry_path,
                    });
                }
            }
        }

        results
    }
}

#[derive(Debug, Clone)]
pub struct SkillPath {
    pub skill_dir: PathBuf,
    pub skill_file: PathBuf,
}
```

---

## Fail-Open Error Handling

### SkillLoadOutcome Type

Skills use fail-open semantics: one invalid skill should not prevent loading of other skills.

```rust
/// Skill loading result with partial success support
#[derive(Debug, Default)]
pub struct SkillLoadOutcome {
    /// Successfully loaded skills
    pub skills: Vec<SkillPromptCommand>,
    /// Non-blocking errors (logged but don't stop loading)
    pub errors: Vec<SkillError>,
    /// Paths explicitly disabled by user config
    pub disabled_paths: HashSet<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct SkillError {
    pub path: PathBuf,
    pub message: String,
    pub error_type: SkillErrorType,
}

#[derive(Debug, Clone, Copy)]
pub enum SkillErrorType {
    /// Cannot read file
    IoError,
    /// Invalid YAML frontmatter
    ParseError,
    /// Field validation failed
    ValidationError,
    /// Symlink cycle or depth exceeded
    TraversalError,
}

impl SkillLoadOutcome {
    /// Get only enabled skills (not in disabled_paths)
    pub fn enabled_skills(&self) -> impl Iterator<Item = &SkillPromptCommand> {
        self.skills.iter().filter(|s| {
            s.base_dir.as_ref()
                .map(|p| !self.disabled_paths.contains(p))
                .unwrap_or(true)
        })
    }

    /// Merge another outcome into this one
    pub fn merge(&mut self, other: SkillLoadOutcome) {
        self.skills.extend(other.skills);
        self.errors.extend(other.errors);
        self.disabled_paths.extend(other.disabled_paths);
    }

    /// Check if any errors occurred
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get error count
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }
}
```

### Fail-Open Loading Functions

```rust
/// Load skills from directory with fail-open semantics
pub async fn load_skills_from_dir(
    dir: &Path,
    source: SkillSource,
    dedup: &mut SkillDeduplicator,
) -> SkillLoadOutcome {
    let mut outcome = SkillLoadOutcome::default();
    let mut scanner = SkillScanner::new();

    // Scan for skill paths
    let skill_paths = scanner.scan_directory(dir, 0);

    for skill_path in skill_paths {
        // Check deduplication first
        if dedup.is_duplicate(&skill_path.skill_file) {
            continue;
        }

        // Try to parse skill file
        match parse_skill_file(&skill_path.skill_file, source) {
            Ok(mut skill) => {
                // Validate skill fields
                if let Err(validation_err) = skill.validate() {
                    outcome.errors.push(SkillError {
                        path: skill_path.skill_file.clone(),
                        message: format!("Validation failed: {validation_err:?}"),
                        error_type: SkillErrorType::ValidationError,
                    });
                    continue;  // Skip invalid skill, continue loading others
                }

                // Set base_dir
                skill.base_dir = Some(skill_path.skill_dir);
                outcome.skills.push(skill);
            }
            Err(err) => {
                // Log error, continue loading other skills
                outcome.errors.push(SkillError {
                    path: skill_path.skill_file.clone(),
                    message: err.to_string(),
                    error_type: SkillErrorType::ParseError,
                });
            }
        }
    }

    outcome
}

/// Load all skills with priority ordering and fail-open semantics
pub async fn load_all_skills(ctx: &CommandContext) -> SkillLoadOutcome {
    let mut outcome = SkillLoadOutcome::default();
    let mut dedup = SkillDeduplicator::new();

    // Load in priority order (highest first)
    let dirs = [
        (ctx.policy_skills_dir(), SkillSource::PolicySettings),
        (ctx.user_skills_dir(), SkillSource::UserSettings),
        (ctx.project_skills_dir(), SkillSource::ProjectSettings),
        (ctx.bundled_skills_dir(), SkillSource::Bundled),
    ];

    for (dir, source) in dirs {
        if let Some(dir) = dir {
            if dir.exists() {
                let dir_outcome = load_skills_from_dir(&dir, source, &mut dedup).await;
                outcome.merge(dir_outcome);
            }
        }
    }

    // Log errors but don't fail
    if outcome.has_errors() {
        tracing::warn!(
            "Failed to load {} skill(s): {:?}",
            outcome.error_count(),
            outcome.errors.iter().map(|e| &e.path).collect::<Vec<_>>()
        );
    }

    outcome
}
```

---

## Embedded/Bundled Skills with Fingerprinting

### Bundled Skills Installation

```rust
use include_dir::{include_dir, Dir};

/// Embedded bundled skills from build time
const BUNDLED_SKILLS_DIR: Dir =
    include_dir!("$CARGO_MANIFEST_DIR/src/skills/bundled");

/// Bundled skills error
#[derive(Debug)]
pub enum BundledSkillsError {
    IoError(std::io::Error),
    HashError(String),
}

/// Install bundled skills to user directory
pub fn install_bundled_skills(cocode_home: &Path) -> Result<(), BundledSkillsError> {
    let target_dir = cocode_home.join("skills-bundled");
    let marker_path = target_dir.join(".cocode-bundled.marker");

    // Check fingerprint - only update if version changed
    let current_fingerprint = bundled_skills_fingerprint();
    if marker_path.exists() {
        if let Ok(existing) = std::fs::read_to_string(&marker_path) {
            if existing.trim() == current_fingerprint {
                return Ok(());  // Already up to date
            }
        }
    }

    // Remove old bundled skills and write new
    if target_dir.exists() {
        std::fs::remove_dir_all(&target_dir).map_err(BundledSkillsError::IoError)?;
    }
    std::fs::create_dir_all(&target_dir).map_err(BundledSkillsError::IoError)?;

    // Extract embedded files
    extract_dir(&BUNDLED_SKILLS_DIR, &target_dir)?;

    // Write marker with fingerprint
    std::fs::write(&marker_path, &current_fingerprint).map_err(BundledSkillsError::IoError)?;

    Ok(())
}

/// Compute fingerprint of embedded skills (deterministic hash)
fn bundled_skills_fingerprint() -> String {
    use std::collections::BTreeMap;
    use sha2::{Sha256, Digest};

    let mut hasher = Sha256::new();

    // Collect all files sorted by path for determinism
    let mut files: BTreeMap<&str, &[u8]> = BTreeMap::new();
    collect_files(&BUNDLED_SKILLS_DIR, "", &mut files);

    for (path, content) in files {
        hasher.update(path.as_bytes());
        hasher.update(content);
    }

    // Use first 16 chars of hex hash
    format!("{:x}", hasher.finalize())[..16].to_string()
}

/// Recursively collect files from embedded directory
fn collect_files<'a>(dir: &'a Dir, prefix: &str, files: &mut BTreeMap<&'a str, &'a [u8]>) {
    for file in dir.files() {
        let path = if prefix.is_empty() {
            file.path().to_str().unwrap_or("")
        } else {
            // Would need to allocate for full path, simplified here
            file.path().to_str().unwrap_or("")
        };
        files.insert(path, file.contents());
    }

    for subdir in dir.dirs() {
        collect_files(subdir, prefix, files);
    }
}

/// Extract embedded directory to filesystem
fn extract_dir(embedded: &Dir, target: &Path) -> Result<(), BundledSkillsError> {
    for file in embedded.files() {
        let file_path = target.join(file.path());
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(BundledSkillsError::IoError)?;
        }
        std::fs::write(&file_path, file.contents()).map_err(BundledSkillsError::IoError)?;
    }

    for subdir in embedded.dirs() {
        let subdir_path = target.join(subdir.path());
        std::fs::create_dir_all(&subdir_path).map_err(BundledSkillsError::IoError)?;
        extract_dir(subdir, target)?;
    }

    Ok(())
}
```

---

## Progressive Disclosure Instructions

### Skill Tool Description

The Skill tool includes guidance on progressive disclosure to help the LLM use skills efficiently:

```rust
impl Tool for SkillTool {
    fn description(&self) -> String {
        let skill_list = self.format_skill_list();

        format!(r#"Execute a skill within the main conversation.

Available skills:
{skill_list}

## How to use skills effectively
1. Open SKILL.md, read enough to follow the workflow
2. Load references/ directory contents only as needed
3. Prefer running provided scripts over retyping code
4. Reuse existing assets instead of recreating them
5. Keep context small - don't deep-dive into referenced files unnecessarily

Usage: skill: "<skill-name>", args: "<arguments>""#)
    }

    fn format_skill_list(&self) -> String {
        self.available_skills
            .iter()
            .map(|s| {
                let desc = s.description.chars().take(100).collect::<String>();
                format!("- {}: {}", s.name, desc)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
```

---

## Related Documentation

- [features.md](./features.md) - Skill file format, frontmatter fields, caching, interface metadata
- [hooks.md](./hooks.md) - Skill-level hooks configuration
- [subagents.md](./subagents.md) - Fork context execution
