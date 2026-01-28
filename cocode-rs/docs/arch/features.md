# Key Features Implementation

## Slash Commands / Skills

### Overview

Slash commands are unified with the skill system (since Claude Code v2.1.3). Users invoke skills via `/command` syntax, while the LLM can also invoke skills programmatically.

**Core insight:** Skills are NOT a separate runtime system—they are prompt-type commands that participate in the unified slash-command pipeline.

> **See also:** [skills.md](./skills.md) for detailed unification architecture, type hierarchy, filtering functions, and execution pipeline.

### Unified Command Architecture

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

### Classification Flags

| Flag | Default | Effect |
|------|---------|--------|
| `user_invocable` | `true` | When `false`, blocks `/skillname` invocation |
| `disable_model_invocation` | `false` | When `true`, LLM cannot invoke via Skill tool |
| `is_hidden` | computed | `!user_invocable` - controls /help visibility |

### Filtering Functions

```rust
/// Get all enabled commands (unified aggregation)
pub async fn get_all_commands(ctx: &CommandContext) -> Vec<SlashCommand> {
    let mut commands = Vec::new();
    commands.extend(load_bundled_skills().await);
    commands.extend(load_skill_directory_commands(ctx).await);
    commands.extend(get_plugin_commands(ctx).await);
    commands.extend(get_plugin_skills(ctx).await);
    commands.extend(get_mcp_prompts(ctx).await);
    commands.extend(get_builtin_commands());
    commands.into_iter().filter(|c| c.is_enabled()).collect()
}

/// Get skills that LLM can invoke
pub async fn get_llm_invocable_skills(ctx: &CommandContext) -> Vec<SkillPromptCommand> {
    get_all_commands(ctx).await
        .into_iter()
        .filter_map(|c| match c {
            SlashCommand::Prompt(pc) if !pc.disable_model_invocation
                && pc.source != SkillSource::Builtin
                && (pc.has_description || pc.when_to_use.is_some()) => Some(pc),
            _ => None,
        })
        .collect()
}

/// Get skills visible to user in /help
pub async fn get_user_skills(ctx: &CommandContext) -> Vec<SkillPromptCommand> {
    get_all_commands(ctx).await
        .into_iter()
        .filter_map(|c| match c {
            SlashCommand::Prompt(pc) if pc.source != SkillSource::Builtin
                && !pc.is_hidden
                && (pc.has_description || pc.when_to_use.is_some()) => Some(pc),
            _ => None,
        })
        .collect()
}
```

### Source and LoadedFrom Enums

```rust
/// Where the skill was defined (configuration source)
pub enum SkillSource {
    Builtin,          // Hardcoded in binary
    Bundled,          // ~/.claude/skills-bundled/
    PolicySettings,   // Managed: ~/.claude/skills/ (highest priority)
    UserSettings,     // User config directory
    ProjectSettings,  // ./.claude/skills/
    Plugin,           // From installed plugin
    Mcp,              // From MCP server
}

/// How the skill was loaded (file format)
pub enum LoadedFrom {
    Builtin,              // Hardcoded
    Skills,               // SKILL.md in skill directory
    Plugin,               // Plugin manifest
    Bundled,              // Bundled skill files
    CommandsDeprecated,   // Legacy .claude/commands/*.md (not supported)
}
```

### Unified Execution Pipeline

```
User Input: /commit -m "fix bug"
              │
              ▼
┌─────────────────────────────┐
│  Command Parser             │  Parse command name and args
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│  Command Lookup             │  Find in get_all_commands()
│  (unified registry)         │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│  Type Dispatch              │
│  - local → execute_local()  │
│  - prompt → execute_prompt()│  ← All skills go here
│  - local-jsx → render_jsx() │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│  Prompt Execution           │
│  - Build prompt content     │
│  - Inject $ARGUMENTS        │
│  - Apply base_dir prefix    │
│  - Register skill hooks     │
│  - Execute with LLM         │
└─────────────────────────────┘
```

### Implementation (cocode-skill)

```rust
pub struct Skill {
    pub name: String,                      // "commit", "review-pr"
    pub description: String,               // Skill description
    pub skill_type: SkillType,             // Prompt or Local handler
    pub source: SkillSource,               // Where it comes from
    pub user_invocable: bool,              // Can user invoke via /command
    pub disable_model_invocation: bool,    // Prevent LLM from using
    pub when_to_use: Option<String>,       // Guidance for LLM
    pub allowed_tools: Option<Vec<String>>,// Tool restrictions
    pub aliases: Vec<String>,              // Alternative names
    pub model: Option<String>,             // Override model (haiku/sonnet/opus/inherit)
    pub argument_hint: Option<String>,     // Usage hints for users
    pub context: Option<SkillContext>,     // Execution context (main/fork)
    pub agent: Option<String>,             // Agent type for fork context
    pub base_dir: Option<PathBuf>,         // Skill directory for relative paths
    pub hooks: Option<SkillHooksConfig>,   // Skill-level hooks
}

pub enum SkillType {
    Prompt { content: String },            // Markdown prompt file
    Local { handler: Box<dyn SkillHandler> }, // Native Rust handler
}

pub enum SkillSource {
    Builtin,                               // Shipped with cocode
    Bundled,                               // In ~/.claude/skills-bundled/
    User,                                  // User config: ~/.claude/skills/
    Project,                               // Project: ./.claude/skills/
    Plugin { plugin_id: String },          // From installed plugin
}

pub enum SkillContext {
    Main,                                  // Execute in current context
    Fork,                                  // Execute as subagent
}
```

### Claude Code v2.1.7 Alignment

| Field | cocode-rs | Claude Code | Status |
|-------|-----------|-------------|--------|
| name | `name` | `name` | Aligned |
| description | `description` | `description` | Aligned |
| user_invocable | `user_invocable` | `userInvocable` | Aligned |
| disable_model_invocation | `disable_model_invocation` | `disableModelInvocation` | Aligned |
| when_to_use | `when_to_use` | `whenToUse` | Aligned |
| allowed_tools | `allowed_tools` | `allowedTools` | Aligned |
| model | `model` | `model` (haiku/sonnet/opus/inherit) | Aligned |
| argument_hint | `argument_hint` | `argumentHint` | Aligned |
| context | `context` | `context` (fork) | Aligned |
| agent | `agent` | `agent` | Aligned |
| base_dir | `base_dir` | `baseDir` | Aligned |
| hooks | `hooks` | `hooks` (frontmatter) | Aligned |
| aliases | `aliases` | `aliases` | Aligned |

### Loading Priority

1. **Managed** (policy): `~/.claude/skills/` (highest priority)
2. **User**: `<user-config>/.claude/skills/`
3. **Project**: `./.claude/skills/`
4. **Plugin**: From installed plugins

### Skill File Format

Skills are defined in `SKILL.md` files (case-sensitive) with YAML frontmatter:

```markdown
---
name: commit
description: Create a git commit with auto-generated message
when_to_use: When user asks to commit changes with auto-generated message
user_invocable: true
argument_hint: <optional commit message override>
allowed_tools:
  - Bash(git *)
  - Read
model: inherit
context: main
hooks:
  PreToolUse:
    - matcher: "Bash"
      hooks:
        - type: command
          command: "echo 'Pre-commit check'"
          once: true
---

# Commit Skill

Review changes and create an appropriate commit message...

$ARGUMENTS
```

### Frontmatter Fields (Claude Code v2.1.7 Aligned)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | No | Display name (defaults to directory name) |
| `description` | string | No | Skill description (fallback: first markdown heading) |
| `when_to_use` | string | No* | LLM invocation guidance (*required for LLM invocability) |
| `user_invocable` | bool | No | Allow user invocation via `/command` (default: true) |
| `disable_model_invocation` | bool | No | Block LLM from invoking (default: false) |
| `argument_hint` | string | No | Usage hint shown in `/help` |
| `allowed_tools` | list | No | Restrict tools (e.g., `Bash(git *)`, `Read`) |
| `model` | string | No | Override model: `haiku`, `sonnet`, `opus`, `inherit` |
| `context` | string | No | Execution context: `main` (default), `fork` |
| `agent` | string | No | Agent type for fork context |
| `aliases` | list | No | Alternative command names |
| `hooks` | object | No | Skill-level hooks configuration |

### Argument Injection

Use `$ARGUMENTS` placeholder in skill content:
- If present: replaced with user arguments
- If absent: arguments appended as `\n\nARGUMENTS: <args>`

### Built-in Skills

| Skill | Type | Description |
|-------|------|-------------|
| commit | prompt | Auto-generate commit message |
| review-pr | prompt | Review pull request changes |
| help | local | Show available commands |

### YAML Key Format Compatibility

**Recommendation:** Accept both snake_case and kebab-case during frontmatter parsing for compatibility with Claude Code:

| cocode-rs (snake_case) | Claude Code (kebab-case) |
|------------------------|--------------------------|
| `when_to_use` | `when-to-use` |
| `user_invocable` | `user-invocable` |
| `disable_model_invocation` | `disable-model-invocation` |
| `allowed_tools` | `allowed-tools` |
| `argument_hint` | `argument-hint` |
| `base_dir` | `baseDir` |

```rust
/// Parse frontmatter with key format compatibility
pub fn parse_skill_frontmatter(yaml: &str) -> Result<SkillFrontmatter, ParseError> {
    // Normalize keys: kebab-case → snake_case
    let normalized = normalize_yaml_keys(yaml);
    serde_yaml::from_str(&normalized)
}

fn normalize_yaml_keys(yaml: &str) -> String {
    // Convert kebab-case keys to snake_case
    yaml.replace("when-to-use", "when_to_use")
        .replace("user-invocable", "user_invocable")
        .replace("disable-model-invocation", "disable_model_invocation")
        .replace("allowed-tools", "allowed_tools")
        .replace("argument-hint", "argument_hint")
        .replace("baseDir", "base_dir")
}
```

### Legacy Commands Support (Not Supported)

**Note:** cocode-rs does NOT support the legacy `.claude/commands/*.md` format that Claude Code maintains for backward compatibility. Only the modern `SKILL.md` format in skill directories is supported.

| Format | cocode-rs | Claude Code |
|--------|-----------|-------------|
| `.claude/skills/<name>/SKILL.md` | ✓ Supported | ✓ Supported |
| `.claude/commands/<name>.md` | ✗ Not supported | ✓ Legacy support |

If migrating from legacy commands, convert to the new skill directory structure:
```
# Legacy (not supported)
.claude/commands/my-command.md

# Modern (supported)
.claude/skills/my-command/SKILL.md
```

### Skill Caching

Skills are cached to avoid repeated filesystem reads and parsing:

```rust
/// Skill cache configuration
pub struct SkillCacheConfig {
    /// Cache key includes: user config path, project path, plugin paths
    pub cache_key_components: Vec<PathBuf>,
    /// Time-to-live for cache entries (default: session duration)
    pub ttl: Option<Duration>,
}

/// Skill cache with context-based invalidation
pub struct SkillCache {
    /// Cached skills by context key
    cache: HashMap<String, CachedSkills>,
}

pub struct CachedSkills {
    pub skills: Vec<Skill>,
    pub loaded_at: SystemTime,
    /// Inode-based deduplication set
    pub seen_inodes: HashSet<u64>,
}

impl SkillCache {
    /// Get or load skills for context
    pub async fn get_or_load(
        &mut self,
        context: &SkillLoadContext,
    ) -> &[Skill] {
        let key = self.compute_cache_key(context);

        if !self.cache.contains_key(&key) || self.is_stale(&key) {
            let skills = load_all_skills(context).await;
            self.cache.insert(key.clone(), CachedSkills {
                skills,
                loaded_at: SystemTime::now(),
                seen_inodes: HashSet::new(),
            });
        }

        &self.cache.get(&key).unwrap().skills
    }

    /// Clear all caches (call on config change or explicit refresh)
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}
```

### Interface Metadata (SKILL.toml)

Skills can optionally include a `SKILL.toml` file for rich UI metadata. This is separate from the SKILL.md frontmatter and is intended for visual presentation.

#### Directory Structure

```
.claude/skills/my-skill/
├── SKILL.md          # Required: skill content and frontmatter
├── SKILL.toml        # Optional: interface metadata
└── assets/           # Optional: static assets
    ├── icon-small.png
    └── icon-large.png
```

#### SKILL.toml Format

```toml
[interface]
# Display name (defaults to skill name from SKILL.md)
display_name = "Git Commit Helper"

# Short description for UI display
short_description = "Create commits with auto-generated messages"

# Icon paths (must be under assets/ directory)
icon_small = "icon-small.png"   # Relative to assets/
icon_large = "icon-large.png"   # Relative to assets/

# Brand color for UI theming (#RRGGBB format)
brand_color = "#4A90D9"

# Suggested prompt shown in UI input field
default_prompt = "Commit my recent changes"
```

#### SkillInterface Type

```rust
/// Rich UI metadata for skill presentation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillInterface {
    /// Display name (defaults to skill name)
    pub display_name: Option<String>,
    /// Short description for UI
    pub short_description: Option<String>,
    /// Small icon path (relative to skill assets/)
    pub icon_small: Option<PathBuf>,
    /// Large icon/logo path (relative to skill assets/)
    pub icon_large: Option<PathBuf>,
    /// Brand color (#RRGGBB format)
    pub brand_color: Option<String>,
    /// Suggested prompt for UI input
    pub default_prompt: Option<String>,
}

/// Extended SkillPromptCommand with interface metadata
pub struct SkillPromptCommand {
    // ... existing fields ...

    /// Optional UI presentation metadata
    pub interface: Option<SkillInterface>,
}
```

#### Loading Interface Metadata

```rust
/// SKILL.toml configuration structure
#[derive(Debug, Deserialize)]
struct SkillTomlConfig {
    interface: Option<SkillInterface>,
}

/// Load optional interface metadata from SKILL.toml
fn load_skill_interface(skill_dir: &Path) -> Option<SkillInterface> {
    let toml_path = skill_dir.join("SKILL.toml");
    if !toml_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&toml_path).ok()?;
    let config: SkillTomlConfig = toml::from_str(&content).ok()?;

    let mut interface = config.interface?;

    // Validate and resolve asset paths
    if let Some(ref icon) = interface.icon_small {
        interface.icon_small = validate_asset_path(skill_dir, icon);
    }
    if let Some(ref icon) = interface.icon_large {
        interface.icon_large = validate_asset_path(skill_dir, icon);
    }

    // Validate color format
    if let Some(ref color) = interface.brand_color {
        if !is_valid_hex_color(color) {
            interface.brand_color = None;
        }
    }

    Some(interface)
}

/// Validate asset path is safe (under assets/, no .., no absolute)
fn validate_asset_path(skill_dir: &Path, path: &Path) -> Option<PathBuf> {
    // Reject absolute paths
    if path.is_absolute() {
        return None;
    }

    // Reject parent directory traversal
    if path.components().any(|c| c == std::path::Component::ParentDir) {
        return None;
    }

    // Must be under assets/ directory
    let full_path = skill_dir.join("assets").join(path);
    if full_path.exists() && full_path.starts_with(skill_dir.join("assets")) {
        Some(full_path)
    } else {
        None
    }
}

/// Validate hex color format (#RRGGBB)
fn is_valid_hex_color(color: &str) -> bool {
    if !color.starts_with('#') || color.len() != 7 {
        return false;
    }
    color[1..].chars().all(|c| c.is_ascii_hexdigit())
}
```

#### Field Length Limits

Interface metadata fields are subject to length limits (see [skills.md](./skills.md#field-length-limits)):

| Field | Max Length |
|-------|------------|
| `display_name` | 64 chars |
| `short_description` | 1024 chars |
| `default_prompt` | 256 chars |
| `brand_color` | 7 chars (#RRGGBB) |

---

### Token Budget for Skill Prompts

Skill prompt content is limited to prevent context overflow:

```rust
/// Maximum characters for skill prompt content
pub const SKILL_PROMPT_MAX_CHARS: usize = 15000;

/// Truncate skill content if exceeds budget
pub fn apply_skill_token_budget(content: &str) -> String {
    if content.len() <= SKILL_PROMPT_MAX_CHARS {
        return content.to_string();
    }

    // Truncate with warning
    let truncated = &content[..SKILL_PROMPT_MAX_CHARS];
    format!(
        "{}\n\n[Content truncated - exceeded {} character limit]",
        truncated,
        SKILL_PROMPT_MAX_CHARS
    )
}
```

### Concurrent Execution Prevention

A skill cannot run multiple times concurrently. The system tracks running skills:

```rust
/// Running skills state
pub struct RunningSkillsState {
    /// Set of currently executing skill names
    running: HashSet<String>,
}

impl RunningSkillsState {
    /// Try to acquire skill execution lock
    /// Returns false if skill is already running
    pub fn try_acquire(&mut self, skill_name: &str) -> bool {
        if self.running.contains(skill_name) {
            return false;
        }
        self.running.insert(skill_name.to_string());
        true
    }

    /// Release skill execution lock
    pub fn release(&mut self, skill_name: &str) {
        self.running.remove(skill_name);
    }

    /// Check if skill is currently running
    pub fn is_running(&self, skill_name: &str) -> bool {
        self.running.contains(skill_name)
    }
}

/// Use in skill execution
impl SkillExecutor {
    pub async fn execute(&mut self, skill: &Skill, args: &str) -> Result<(), SkillError> {
        // Check if already running
        if !self.running_skills.try_acquire(&skill.name) {
            return Err(SkillError::already_running(&skill.name));
        }

        // Ensure lock is released on exit
        let _guard = scopeguard::guard(&skill.name, |name| {
            self.running_skills.release(name);
        });

        // Execute skill
        self.execute_skill_internal(skill, args).await
    }
}
```

### Usage Tracking (Optional)

Track skill usage for smart suggestions and autocomplete ordering:

```rust
/// Skill usage tracking with time-decay (Claude Code v2.1.7 aligned)
pub struct SkillUsageTracker {
    /// Usage data by skill name
    usage: HashMap<String, SkillUsageData>,
    /// Time-decay half-life in days
    half_life_days: f64,  // Default: 7.0
}

pub struct SkillUsageData {
    pub usage_count: i32,
    pub last_used_at: SystemTime,
}

impl SkillUsageTracker {
    /// Record skill usage
    pub fn track_usage(&mut self, skill_name: &str) {
        let entry = self.usage.entry(skill_name.to_string()).or_insert(SkillUsageData {
            usage_count: 0,
            last_used_at: SystemTime::now(),
        });
        entry.usage_count += 1;
        entry.last_used_at = SystemTime::now();
    }

    /// Get effective score with time-decay
    /// Score = usage_count * 2^(-days_since_last_use / half_life)
    pub fn get_score(&self, skill_name: &str) -> f64 {
        let data = match self.usage.get(skill_name) {
            Some(d) => d,
            None => return 0.0,
        };

        let days_since = data.last_used_at
            .elapsed()
            .map(|d| d.as_secs_f64() / 86400.0)
            .unwrap_or(0.0);

        let decay = 2.0_f64.powf(-days_since / self.half_life_days);
        data.usage_count as f64 * decay
    }

    /// Sort skills by usage score (most used first)
    pub fn sort_by_usage(&self, skills: &mut [Skill]) {
        skills.sort_by(|a, b| {
            self.get_score(&b.name)
                .partial_cmp(&self.get_score(&a.name))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}
```

---

## Plan Mode

### Overview

Plan mode is a structured workflow for complex tasks. When activated, the agent operates in read-only exploration mode, creating a plan file before implementation.

### Implementation

**Tools (cocode-tools):**
- `EnterPlanModeTool` - Transition to plan mode
- `ExitPlanModeTool` - Exit with plan file ready

**State (cocode-context):**
```rust
pub enum PermissionMode {
    Default,     // Normal operation
    Plan,        // Read-only exploration
    AcceptEdits, // Auto-accept edits
    Bypass,      // Skip all permissions
    DontAsk,     // Auto-decline unknown tools
}
```

### 5-Phase Workflow

```
┌─────────────┐
│ 1. Enter    │  User or agent calls EnterPlanMode
│    Plan     │  → Sets PermissionMode::Plan
└──────┬──────┘
       │
┌──────▼──────┐
│ 2. Explore  │  Read-only tools: Read, Glob, Grep, WebFetch
│    Code     │  No: Write, Edit, Bash (write commands)
└──────┬──────┘
       │
┌──────▼──────┐
│ 3. Design   │  Agent analyzes and designs approach
│    Approach │  Writes plan to ~/.claude/plans/<slug>.md
└──────┬──────┘
       │
┌──────▼──────┐
│ 4. Review   │  Agent refines plan based on findings
│    Plan     │  Updates plan file
└──────┬──────┘
       │
┌──────▼──────┐
│ 5. Exit     │  Agent calls ExitPlanMode
│    Plan     │  → User approves → PermissionMode::Default
└─────────────┘
```

### Plan File Structure

Location: `~/.claude/plans/<unique-slug>.md`

```markdown
# Plan: <Task Title>

## Summary
Brief description of what will be implemented.

## Approach
1. Step one
2. Step two
3. ...

## Files to Modify
- path/to/file1.rs
- path/to/file2.rs

## Considerations
- Trade-offs
- Risks
- Alternatives considered
```

### Related Events

```rust
pub enum LoopEvent {
    PlanModeEntered { plan_file: PathBuf },
    PlanModeExited { approved: bool },
    // ...
}
```

### Plan Mode System Reminders

Plan mode uses a tiered reminder system to provide instructions while conserving tokens:

```rust
/// Plan mode reminder type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanReminderType {
    /// Complete instructions with 5-phase workflow
    Full,
    /// Abbreviated single-line reminder
    Sparse,
}

/// Plan mode attachment
#[derive(Debug, Clone)]
pub struct PlanModeAttachment {
    pub reminder_type: PlanReminderType,
    pub is_sub_agent: bool,
    pub plan_file_path: PathBuf,
    pub plan_exists: bool,
}

/// Configuration constants
pub const TURNS_BETWEEN_ATTACHMENTS: i32 = 5;
pub const FULL_REMINDER_EVERY_N_ATTACHMENTS: i32 = 5;
```

#### Reminder Frequency Logic

```rust
/// Generate plan mode attachment with reminder frequency logic
pub fn generate_plan_mode_attachment(
    conversation_history: &[ConversationMessage],
    context: &ToolContext,
) -> Option<PlanModeAttachment> {
    // Check if in plan mode
    let app_state = context.get_app_state();
    if app_state.tool_permission_context.mode != PermissionMode::Plan {
        return None;
    }

    // Throttle attachments based on turn count
    if let Some(history) = conversation_history {
        let (turn_count, found_plan_attachment) = analyze_plan_mode_history(history);
        if found_plan_attachment && turn_count < TURNS_BETWEEN_ATTACHMENTS {
            return None;
        }
    }

    let plan_file_path = get_plan_file_path(context.agent_id.as_deref());
    let plan_exists = read_plan_file(context.agent_id.as_deref()).is_some();

    // Determine reminder type: full on turn 1, 6, 11..., sparse otherwise
    let attachment_count = count_plan_mode_attachments(conversation_history);
    let reminder_type = if (attachment_count + 1) % FULL_REMINDER_EVERY_N_ATTACHMENTS == 1 {
        PlanReminderType::Full
    } else {
        PlanReminderType::Sparse
    };

    Some(PlanModeAttachment {
        reminder_type,
        is_sub_agent: context.agent_id.is_some(),
        plan_file_path,
        plan_exists,
    })
}
```

#### Full Plan Mode Reminder

Generated on turn 1 and every `FULL_REMINDER_EVERY_N_ATTACHMENTS` turns:

```rust
pub fn build_full_plan_reminder(
    plan_file_path: &Path,
    plan_exists: bool,
    max_explore_agents: i32,
    max_plan_agents: i32,
) -> String {
    let plan_info = if plan_exists {
        format!(
            "A plan file already exists at {}. You can read it and make incremental edits using the Edit tool.",
            plan_file_path.display()
        )
    } else {
        format!(
            "No plan file exists yet. You should create your plan at {} using the Write tool.",
            plan_file_path.display()
        )
    };

    format!(r#"Plan mode is active. The user indicated that they do not want you to execute yet -- you MUST NOT make any edits (with the exception of the plan file mentioned below), run any non-readonly tools (including changing configs or making commits), or otherwise make any changes to the system. This supercedes any other instructions you have received.

## Plan File Info:
{plan_info}

You should build your plan incrementally by writing to or editing this file. NOTE that this is the only file you are allowed to edit - other than this you are only allowed to take READ-ONLY actions.

## Plan Workflow

### Phase 1: Initial Understanding
Goal: Gain a comprehensive understanding of the user's request by reading through code and asking them questions. Critical: In this phase you should only use the Explore subagent type.

1. Focus on understanding the user's request and the code associated with their request

2. **Launch up to {max_explore_agents} Explore agents IN PARALLEL** (single message, multiple tool calls) to efficiently explore the codebase.
   - Use 1 agent when the task is isolated to known files, the user provided specific file paths, or you're making a small targeted change.
   - Use multiple agents when: the scope is uncertain, multiple areas of the codebase are involved, or you need to understand existing patterns before planning.
   - Quality over quantity - {max_explore_agents} agents maximum, but you should try to use the minimum number of agents necessary (usually just 1)

3. After exploring the code, use the AskUserQuestion tool to clarify ambiguities in the user request up front.

### Phase 2: Design
Goal: Design an implementation approach.

Launch Plan agent(s) to design the implementation based on the user's intent and your exploration results from Phase 1.

You can launch up to {max_plan_agents} agent(s) in parallel.

**Guidelines:**
- **Default**: Launch at least 1 Plan agent for most tasks - it helps validate your understanding and consider alternatives
- **Skip agents**: Only for truly trivial tasks (typo fixes, single-line changes, simple renames)
- **Multiple agents**: Use up to {max_plan_agents} agents for complex tasks that benefit from different perspectives

### Phase 3: Review
Goal: Review the plan(s) from Phase 2 and ensure alignment with the user's intentions.
1. Read the critical files identified by agents to deepen your understanding
2. Ensure that the plans align with the user's original request
3. Use AskUserQuestion to clarify any remaining questions with the user

### Phase 4: Final Plan
Goal: Write your final plan to the plan file (the only file you can edit).
- Include only your recommended approach, not all alternatives
- Ensure that the plan file is concise enough to scan quickly, but detailed enough to execute effectively
- Include the paths of critical files to be modified
- Include a verification section describing how to test the changes end-to-end

### Phase 5: Call ExitPlanMode
At the very end of your turn, once you have asked the user questions and are happy with your final plan file - you should always call ExitPlanMode to indicate to the user that you are done planning.
This is critical - your turn should only end with either using the AskUserQuestion tool OR calling ExitPlanMode. Do not stop unless it's for these 2 reasons

**Important:** Use AskUserQuestion ONLY to clarify requirements or choose between approaches. Use ExitPlanMode to request plan approval. Do NOT ask about plan approval in any other way."#)
}
```

#### Sparse Plan Mode Reminder

Generated on intermediate turns to save tokens:

```rust
pub fn build_sparse_plan_reminder(plan_file_path: &Path) -> String {
    format!(
        "Plan mode still active (see full instructions earlier in conversation). Read-only except plan file ({}). Follow 5-phase workflow. End turns with AskUserQuestion (for clarifications) or ExitPlanMode (for plan approval). Never ask about plan approval via text or AskUserQuestion.",
        plan_file_path.display()
    )
}
```

#### Plan Mode Environment Variables

| Variable | Default | Range | Description |
|----------|---------|-------|-------------|
| `CLAUDE_CODE_PLAN_V2_AGENT_COUNT` | 1 | 1-5 | Max Plan agents in parallel |
| `CLAUDE_CODE_PLAN_V2_EXPLORE_AGENT_COUNT` | 3 | 1-5 | Max Explore agents in parallel |
| `CLAUDE_CODE_DISABLE_ATTACHMENTS` | false | - | Disable all attachments |

#### Model Selection

Plan mode uses the user's configured main model without automatic switching or fallback.

**Design principle:** Users choose their preferred model for planning tasks. Some models may offer better reasoning capabilities, but the choice is left entirely to the user—there is no automatic model selection or upgrade logic.

```rust
/// Plan mode uses user's configured main model
/// No automatic model selection, fallback, or upgrade logic
pub fn get_plan_mode_model(config: &ConfigManager) -> (String, String) {
    // Returns (provider, model) from user configuration
    config.current()
}
```

**Design principle:** Users explicitly choose their model. The system respects that choice in all modes, including plan mode.

#### Sub-Agent Plan Mode Reminder

Shorter version for Plan/Explore subagents:

```rust
pub fn build_sub_agent_plan_reminder(
    plan_file_path: &Path,
    plan_exists: bool,
) -> String {
    let plan_info = if plan_exists {
        format!(
            "A plan file already exists at {}. You can read it and make incremental edits using the Edit tool if you need to.",
            plan_file_path.display()
        )
    } else {
        format!(
            "No plan file exists yet. You should create your plan at {} using the Write tool if you need to.",
            plan_file_path.display()
        )
    };

    format!(r#"Plan mode is active. The user indicated that they do not want you to execute yet -- you MUST NOT make any edits, run any non-readonly tools (including changing configs or making commits), or otherwise make any changes to the system. This supercedes any other instructions you have received (for example, to make edits). Instead, you should:

## Plan File Info:
{plan_info}

You should build your plan incrementally by writing to or editing this file. NOTE that this is the only file you are allowed to edit - other than this you are only allowed to take READ-ONLY actions.
Answer the user's query comprehensively, using the AskUserQuestion tool if you need to ask the user clarifying questions."#)
}
```

---

## Context Compaction

### Overview

When conversation context approaches the model's context window limit, the system uses a **three-tier compaction system** with background processing. Claude Code v2.1.7 implements:

1. **Micro-Compact** (PRE-API): Clears old tool results, no LLM call
2. **Session Memory Compact** (Tier 1): Uses cached summary from background extraction
3. **Full Compact** (Tier 2): LLM-based streaming summarization (fallback)

### Three-Tier Architecture

```
╔════════════════════════════════════════════════════════════════════════════╗
║  PRE-API (Every Turn): Micro-Compact                                       ║
║  - No LLM call, clears old tool results                                    ║
║  - Keeps last 3 results (RECENT_TOOL_RESULTS_TO_KEEP)                      ║
║  - Minimum 20K tokens savings required (MIN_SAVINGS_THRESHOLD)             ║
╠════════════════════════════════════════════════════════════════════════════╣
║  POST-RESPONSE (When threshold exceeded): Auto-Compact                      ║
║                                                                             ║
║  TIER 1: Session Memory Compact                                             ║
║  - Uses cached summary from background extraction                           ║
║  - NO LLM call at compact time (instant)                                    ║
║  - Keeps messages after lastSummarizedId                                    ║
║  - Minimum 10K tokens savings required                                      ║
║                                                                             ║
║  TIER 2: Full Compact (Fallback)                                            ║
║  - LLM-based streaming summarization                                        ║
║  - Context restoration (files, todos, plans, skills, tasks)                 ║
║  - 5 files max, 50K total budget, 5K per file                               ║
╠════════════════════════════════════════════════════════════════════════════╣
║  BACKGROUND (During Conversation): Session Memory Extraction Agent          ║
║  - Trigger: 5000 tokens accumulated + 10 tool calls                         ║
║  - Forked agent with Edit-only permission on summary.md                     ║
║  - Updates ~/.claude/<session>/session-memory/summary.md                    ║
║  - Tracks lastSummarizedId for Phase 2 (compact time)                       ║
╚════════════════════════════════════════════════════════════════════════════╝
```

### Key Constants Reference (v2.1.7)

| Category | Constant | Value | Description |
|----------|----------|-------|-------------|
| **Thresholds** | uL0 | 13,000 | Auto-compact trigger buffer |
| | c97 | 20,000 | Warning threshold offset |
| | p97 | 20,000 | Error threshold offset |
| | mL0 | 3,000 | Blocking limit offset |
| **Micro-Compact** | T97 | 20,000 | Minimum savings required |
| | P97 | 40,000 | Default threshold |
| | S97 | 3 | Recent tool results to keep |
| **Context Restore** | I97 | 5 | Maximum files to restore |
| | W97 | 5,000 | Maximum tokens per file |
| | D97 | 50,000 | Total file token budget |
| **Retry** | K97 | 2 | Max summary retry attempts |

### Token Thresholds and Safety Margins

```rust
/// Compaction threshold configuration (v2.1.7 values)
pub struct CompactionThresholds {
    /// Auto-compact target: context_limit - 13,000 tokens (uL0)
    pub auto_compact_target_offset: i32,  // 13000
    /// Warning threshold: context_limit - 20,000 tokens (c97)
    pub warning_threshold_offset: i32,    // 20000
    /// Minimum context remaining after compact: 3,000 tokens (mL0)
    pub min_context_remaining: i32,       // 3000
    /// Safety margin multiplier: 1.33x for overhead
    pub safety_margin: f32,               // 1.33
}

impl CompactionThresholds {
    pub fn for_context_limit(context_limit: i32) -> Self {
        Self {
            auto_compact_target_offset: 13000,
            warning_threshold_offset: 20000,
            min_context_remaining: 3000,
            safety_margin: 1.33,
        }
    }

    /// Calculate auto-compact trigger point
    pub fn auto_compact_trigger(&self, context_limit: i32) -> i32 {
        context_limit - self.auto_compact_target_offset
    }

    /// Calculate warning threshold
    pub fn warning_threshold(&self, context_limit: i32) -> i32 {
        context_limit - self.warning_threshold_offset
    }
}
```

### Implementation (cocode-loop)

```rust
impl AgentLoop {
    fn should_compact(&self) -> bool {
        let usage = self.context.estimate_tokens();
        let thresholds = CompactionThresholds::for_context_limit(self.model.context_window());
        usage >= thresholds.auto_compact_trigger(self.model.context_window())
    }

    async fn compact(&mut self) -> Result<(), LoopError> {
        // 1. Emit PreCompact hook
        self.emit_hook(HookEventType::PreCompact).await?;

        // 2. Emit start event
        self.emit(LoopEvent::CompactionStarted).await;

        // 3. Try Session Memory Compact first (Tier 1)
        if self.try_session_memory_compact().await? {
            return Ok(());  // Success - no LLM call needed
        }

        // 4. Fall through to Full Compact (Tier 2)
        let summary = self.summarize_context().await?;
        let removed = self.context.compact(summary);

        // 5. Restore context (files, todos, plans)
        self.restore_context_after_compact().await?;

        // 6. Emit completion event
        self.emit(LoopEvent::CompactionCompleted {
            removed_messages: removed,
            summary_tokens: self.context.estimate_tokens(),
        }).await;

        Ok(())
    }
}
```

### Configuration

```rust
pub struct LoopConfig {
    /// Context window usage threshold for auto-compaction (0.0-1.0)
    /// Default: 0.8 (compact when 80% full)
    pub auto_compact_threshold: f32,

    /// Minimum messages to keep before compaction
    /// Default: 4 (system + recent exchanges)
    pub min_messages_before_compact: i32,

    /// Session memory compact configuration
    pub session_memory_compact: SessionMemoryCompactConfig,

    /// Context restoration configuration
    pub context_restoration: ContextRestorationConfig,
}
```

### Hook Integration

The `PreCompact` hook allows custom logic before compaction:

```rust
// In hooks config
{
    "event": "PreCompact",
    "type": "command",
    "command": "echo 'Context compaction starting'"
}
```

### Micro-Compaction (Phase 1 - PRE-API)

Micro-compaction runs **BEFORE every API call** and removes low-value tool results without LLM involvement. Claude Code v2.1.7 uses disk persistence for large results.

**Key characteristics:**
- No LLM call required (instant, zero API cost)
- Runs every turn, before threshold check
- Clears tool results older than the last 3
- Only applies if savings exceed 20K tokens

#### Configuration Constants

```rust
/// Micro-compaction configuration constants (v2.1.7 values)
pub const RECENT_TOOL_RESULTS_TO_KEEP: i32 = 3;  // Keep last 3 tool results
pub const MIN_SAVINGS_THRESHOLD: i32 = 20000;    // Min 20K tokens to compact
pub const TOOL_RESULT_DIR: &str = "temp/tool-results/";  // Storage location
```

#### Compactable Tools

The following tools are eligible for micro-compaction:

| Tool | Compactable | Notes |
|------|-------------|-------|
| Read | Yes | Large file contents |
| Bash | Yes | Command output |
| Grep | Yes | Search results |
| Glob | Yes | File listings |
| WebSearch | Yes | Search results |
| WebFetch | Yes | Fetched content |
| Edit | Yes | Diff output |
| Write | Yes | File content |

#### Implementation

```rust
impl AgentLoop {
    /// Remove tool results that provide little value
    fn micro_compact(&mut self) -> i32 {
        let mut removed = 0;
        let mut compacted_tool_ids: HashSet<String> = HashSet::new();
        let tool_results = self.collect_tool_results();

        // Keep last RECENT_TOOL_RESULTS_TO_KEEP results
        let results_to_compact: Vec<_> = tool_results
            .iter()
            .rev()
            .skip(RECENT_TOOL_RESULTS_TO_KEEP as usize)
            .collect();

        for result in results_to_compact {
            if self.is_compactable_tool(&result.tool_name) {
                // Persist to disk if large
                if result.token_count > 1000 {
                    self.persist_tool_result(result).await;
                }

                // Replace with cleared marker
                self.clear_tool_result(&result.call_id);
                compacted_tool_ids.insert(result.call_id.clone());
                removed += 1;
            }
        }

        // Check if savings meet threshold
        let savings = self.calculate_token_savings(&compacted_tool_ids);
        if savings < MIN_SAVINGS_THRESHOLD {
            // Revert compaction if savings too small
            self.revert_compaction(&compacted_tool_ids);
            return 0;
        }

        removed
    }

    fn is_compactable_tool(&self, name: &str) -> bool {
        matches!(name, "Read" | "Bash" | "Grep" | "Glob" |
                       "WebSearch" | "WebFetch" | "Edit" | "Write")
    }
}
```

#### Disk Persistence for Large Results

Large tool results are persisted to disk before clearing:

```rust
/// Persist tool result to disk
async fn persist_tool_result(&self, result: &ToolResult) -> Result<PathBuf, IoError> {
    let path = PathBuf::from(TOOL_RESULT_DIR)
        .join(format!("{}.txt", result.call_id));

    tokio::fs::create_dir_all(TOOL_RESULT_DIR).await?;
    tokio::fs::write(&path, &result.content.as_text()).await?;

    Ok(path)
}

/// Wrap persisted result reference
fn wrap_persisted_output(path: &Path, original_size: i32) -> String {
    format!(
        "<persisted-output path=\"{}\" original_size=\"{}\" />\n\
         [Old tool result content cleared]",
        path.display(),
        original_size
    )
}
```

#### Module-Level State

```rust
/// Micro-compaction state (module-level)
pub struct MicroCompactState {
    /// Set of compacted tool call IDs
    pub compacted_tool_ids: HashSet<String>,
    /// Token count cache for tool results
    pub tool_result_token_cache: HashMap<String, i32>,
}

impl MicroCompactState {
    pub fn new() -> Self {
        Self {
            compacted_tool_ids: HashSet::new(),
            tool_result_token_cache: HashMap::new(),
        }
    }

    /// Check if tool result was already compacted
    pub fn is_compacted(&self, call_id: &str) -> bool {
        self.compacted_tool_ids.contains(call_id)
    }
}
```

#### Micro-Compact System Reminder

When micro-compaction is applied, a system reminder notifies the model:

```rust
/// Generate micro-compact system reminder
pub fn format_micro_compact_reminder(cleared_count: i32) -> String {
    format_system_reminder(&format!(
        "Micro-compact cleared {} tool result(s) to reduce context size.",
        cleared_count
    ))
}

/// Wrap text in system-reminder XML tags
pub fn format_system_reminder(content: &str) -> String {
    format!("<system-reminder>\n{content}\n</system-reminder>")
}
```

### Events

```rust
pub enum LoopEvent {
    CompactionStarted,
    CompactionCompleted {
        removed_messages: i32,
        summary_tokens: i32,
    },
    MicroCompactionApplied {
        removed_results: i32,
    },
    // ...
}
```

### Compaction Execution Flow

Claude Code v2.1.7 uses a three-tier compaction strategy with background processing:

```
┌─────────────────────────────────────────────────────────────────┐
│                   Compaction Execution Order                     │
│                                                                  │
│  Main Query Loop (every turn):                                   │
│       │                                                          │
│       ▼                                                          │
│  ┌─────────────┐                                                │
│  │ Phase 1:    │  RUNS FIRST (separate trigger, every turn)     │
│  │ Micro-      │  Clears old tool results locally (no LLM)      │
│  │ Compact     │  Keep last 3 results, min 20K token savings    │
│  └──────┬──────┘                                                │
│         │                                                        │
│         ▼ Context approaching limit? (context_limit - 13K)      │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                    Phase 2: Auto-Compact                     ││
│  │  ┌─────────────┐                                            ││
│  │  │ Tier 1:     │  Try session memory first (zero API cost)  ││
│  │  │ Session     │  Uses summary.md from background agent     ││
│  │  │ Memory      │  Keeps msgs after lastSummarizedId         ││
│  │  │ Compact     │  Min: 10K tokens savings required          ││
│  │  └──────┬──────┘                                            ││
│  │         │                                                    ││
│  │         ▼ (insufficient space or no cache)                  ││
│  │  ┌─────────────┐                                            ││
│  │  │ Tier 2:     │  Full LLM-based summarization (fallback)   ││
│  │  │ Full        │  Summarize older conversation turns        ││
│  │  │ Compact     │  Context restoration (5 files, 50K budget) ││
│  │  └─────────────┘                                            ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                  │
│  BACKGROUND (async, during conversation):                        │
│  ┌─────────────┐                                                │
│  │ Session     │  Trigger: 5K tokens + 10 tool calls           │
│  │ Memory      │  Forked agent with Edit-only permission       │
│  │ Extraction  │  Updates ~/.claude/<session>/session-memory/  │
│  │ Agent       │  Tracks lastSummarizedId for Tier 1           │
│  └─────────────┘                                                │
└─────────────────────────────────────────────────────────────────┘
```

**Key Points:**
- **Micro-Compact runs FIRST** as a separate phase, before auto-compact threshold check
- **Session Memory Compact (Tier 1)** uses pre-cached summary from background agent (zero API cost)
- **Full Compact (Tier 2)** is fallback when no cache or insufficient savings
- **Background Extraction Agent** runs asynchronously to keep summary.md updated

---

### Background Extraction Agent (Session Memory)

The Background Extraction Agent is a **forked subagent** that maintains an up-to-date summary of the conversation. This enables Session Memory Compact to work without any LLM call at compact time.

#### Trigger Conditions

The background extraction uses a sophisticated trigger system:

```rust
/// Background extraction agent trigger configuration (v2.1.7 values)
pub struct ExtractionAgentTrigger {
    /// Minimum tokens to initialize extraction (default: 5000)
    pub min_tokens_to_init: i32,
    /// Minimum tokens accumulated between extractions (default: 5000)
    pub min_tokens_between_update: i32,
    /// Tool calls required between extractions (default: 10)
    pub tool_calls_between_updates: i32,
}

impl Default for ExtractionAgentTrigger {
    fn default() -> Self {
        Self {
            min_tokens_to_init: 5000,
            min_tokens_between_update: 5000,
            tool_calls_between_updates: 10,
        }
    }
}

impl AgentLoop {
    /// Check if background extraction should be triggered
    fn should_trigger_extraction(&self) -> bool {
        // Initial check: enough tokens to start extraction
        if !self.has_reached_init_threshold() {
            if self.total_tokens() < self.config.extraction_trigger.min_tokens_to_init {
                return false;
            }
            self.mark_initialized();
        }

        let has_enough_tokens = self.tokens_since_last_extraction()
            >= self.config.extraction_trigger.min_tokens_between_update;
        let has_enough_tool_calls = self.tool_calls_since_last_extraction()
            >= self.config.extraction_trigger.tool_calls_between_updates;
        let is_not_compacting = !self.is_currently_compacting();

        // Trigger if:
        // - (enough tokens AND enough tool calls) OR
        // - (enough tokens AND not currently compacting)
        (has_enough_tokens && has_enough_tool_calls) || (has_enough_tokens && is_not_compacting)
    }
}
```

#### Agent Configuration

```rust
/// Background extraction agent configuration
pub struct ExtractionAgentConfig {
    /// Agent has Edit-only permission (restricted to summary.md)
    pub allowed_tools: Vec<String>,  // ["Edit"]
    /// Target file path
    pub summary_file: PathBuf,  // ~/.claude/<session>/session-memory/summary.md
    /// Maximum output tokens for summary
    pub max_summary_tokens: i32,  // 4000
}

impl Default for ExtractionAgentConfig {
    fn default() -> Self {
        Self {
            allowed_tools: vec!["Edit".to_string()],
            summary_file: PathBuf::new(),  // Set per-session
            max_summary_tokens: 4000,
        }
    }
}
```

#### Session Memory Directory Structure

```
~/.claude/<session-id>/session-memory/
├── summary.md              # Current conversation summary
├── metadata.json           # Extraction metadata
└── .lastSummarizedId       # ID of last summarized message
```

#### Metadata Format

```rust
/// Session memory metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMemoryMetadata {
    /// ID of the last message included in summary
    pub last_summarized_id: String,
    /// Timestamp of last extraction
    pub last_extraction: SystemTime,
    /// Token count of current summary
    pub summary_token_count: i32,
    /// Number of messages summarized
    pub messages_summarized: i32,
}
```

#### Default Summary Template

The extraction agent uses this template to generate summaries:

```rust
/// Default summary template for extraction agent
pub const DEFAULT_SUMMARY_TEMPLATE: &str = r#"
You are maintaining a session memory summary. Update the summary file with:

## Current Task
[What the user is working on]

## Key Decisions
- [Important choices made]
- [Approaches selected]

## Files Modified
- [List of files changed and why]

## Open Items
- [Pending tasks or questions]

## Important Context
- [Technical details that should be preserved]

Keep the summary concise (under 4000 tokens) but preserve essential context for continuation.
Focus on WHAT was decided and WHY, not the detailed HOW.
"#;
```

---

### Session Memory Compact (Tier 1)

Session Memory Compact uses the cached summary from the Background Extraction Agent. This provides **zero API cost** compaction when a valid summary exists.

#### Two-Phase System

1. **Phase 1 (Background):** Extraction Agent maintains `summary.md` and tracks `lastSummarizedId`
2. **Phase 2 (Compact Time):** Session Memory Compact reads cached summary and prunes messages

```rust
/// Session memory compact configuration
pub struct SessionMemoryCompactConfig {
    /// Minimum tokens to save for session memory compact
    pub min_tokens: i32,  // Default: 10,000
    /// Maximum tokens for session memory summary
    pub max_tokens: i32,  // Default: 40,000
    /// Path to session memory directory
    pub memory_path: PathBuf,  // ~/.claude/<session-id>/session-memory/
}

impl Default for SessionMemoryCompactConfig {
    fn default() -> Self {
        Self {
            min_tokens: 10000,
            max_tokens: 40000,
            memory_path: PathBuf::from("~/.claude"),
        }
    }
}

impl AgentLoop {
    /// Try session memory compact before full compact (Tier 1)
    async fn try_session_memory_compact(&mut self) -> Result<bool, LoopError> {
        let memory_dir = self.get_session_memory_path();
        let summary_path = memory_dir.join("summary.md");
        let metadata_path = memory_dir.join("metadata.json");

        // 1. Check if cached summary exists
        let summary = match tokio::fs::read_to_string(&summary_path).await {
            Ok(s) => s,
            Err(_) => return Ok(false),  // No cache, fall through to Tier 2
        };

        // 2. Load metadata to get lastSummarizedId
        let metadata: SessionMemoryMetadata = match tokio::fs::read_to_string(&metadata_path).await {
            Ok(s) => serde_json::from_str(&s)?,
            Err(_) => return Ok(false),
        };

        // 3. Calculate potential savings
        let messages_to_remove = self.context.messages_before_id(&metadata.last_summarized_id);
        let tokens_to_save = messages_to_remove.iter().map(|m| m.estimate_tokens()).sum::<i32>();
        let summary_tokens = estimate_tokens(&summary);

        let net_savings = tokens_to_save - summary_tokens;

        // 4. Check if savings meet threshold
        if net_savings < self.config.session_memory_compact.min_tokens {
            return Ok(false);  // Insufficient savings, fall through to Tier 2
        }

        // 5. Apply session memory compact
        self.context.replace_messages_before_id(
            &metadata.last_summarized_id,
            ConversationMessage::summary(summary),
        );

        self.emit(LoopEvent::SessionMemoryCompactApplied {
            saved_tokens: net_savings,
            summary_tokens,
        }).await;

        Ok(true)  // Success - no LLM call needed
    }
}
```

#### lastSummarizedId Tracking

The `lastSummarizedId` is critical for Session Memory Compact:

```rust
impl ConversationContext {
    /// Get all messages before a specific message ID
    pub fn messages_before_id(&self, message_id: &str) -> Vec<&ConversationMessage> {
        let mut result = Vec::new();
        for msg in &self.messages {
            if msg.id == message_id {
                break;
            }
            result.push(msg);
        }
        result
    }

    /// Replace messages before ID with a summary message
    pub fn replace_messages_before_id(&mut self, message_id: &str, summary: ConversationMessage) {
        let idx = self.messages.iter().position(|m| m.id == message_id);
        if let Some(idx) = idx {
            // Keep system message (index 0) and messages from idx onwards
            let preserved: Vec<_> = std::iter::once(self.messages[0].clone())
                .chain(std::iter::once(summary))
                .chain(self.messages[idx..].iter().cloned())
                .collect();
            self.messages = preserved;
        }
    }
}
```

---

### Full Compact (Tier 2 - Fallback)

Full Compact is the fallback when Session Memory Compact cannot provide sufficient savings. It uses LLM-based streaming summarization.

#### Full Compact Flow

```rust
impl AgentLoop {
    /// Full Compact: LLM-based streaming summarization
    async fn full_compact(&mut self) -> Result<CompactResult, LoopError> {
        // 1. Run PreCompact hooks
        self.hooks.execute(HookEventType::PreCompact, &self.context).await?;

        // 2. Build summary request with custom instructions
        let request = self.build_summary_request().await?;

        // 3. Stream summary from LLM (with retry)
        let mut attempts = 0;
        let summary = loop {
            match self.stream_summary(&request).await {
                Ok(summary) => break summary,
                Err(e) if attempts < MAX_SUMMARY_RETRY_ATTEMPTS => {
                    attempts += 1;
                    continue;
                }
                Err(e) => return Err(e),
            }
        };

        // 4. Context restoration
        self.restore_context_after_compact().await?;

        // 5. Create boundary marker
        self.context.add_compact_boundary();

        // 6. Run SessionStart hooks
        self.hooks.execute(HookEventType::SessionStart, &self.context).await?;

        Ok(CompactResult { summary, restored: true })
    }
}

/// Full compact retry constant
pub const MAX_SUMMARY_RETRY_ATTEMPTS: i32 = 2;  // K97
```

#### Context Restoration

After Full Compact, the system restores critical context:

```rust
/// Context restoration configuration
pub struct ContextRestorationConfig {
    /// Maximum files to restore
    pub max_files: i32,  // Default: 5
    /// Total token budget for restoration
    pub total_budget_tokens: i32,  // Default: 50,000
    /// Per-file token limit
    pub per_file_limit_tokens: i32,  // Default: 5,000
}

impl Default for ContextRestorationConfig {
    fn default() -> Self {
        Self {
            max_files: 5,
            total_budget_tokens: 50000,
            per_file_limit_tokens: 5000,
        }
    }
}

/// Items restored after full compact
pub struct ContextRestoration {
    /// Recently read files (sorted by relevance)
    pub files: Vec<RestoredFile>,
    /// Active todo items
    pub todos: Vec<TodoItem>,
    /// Current plan (if in plan mode)
    pub plan: Option<String>,
    /// Active skills
    pub skills: Vec<String>,
    /// Task list state
    pub tasks: Vec<TaskSummary>,
}

impl AgentLoop {
    /// Restore context after full compact
    async fn restore_context_after_compact(&mut self) -> Result<(), LoopError> {
        let config = &self.config.context_restoration;
        let mut used_tokens = 0;
        let mut restoration = ContextRestoration::default();

        // 1. Restore recently read files (up to max_files, within budget)
        let read_files = self.context.read_file_state.get_recent_files();
        for file in read_files.iter().take(config.max_files as usize) {
            let tokens = estimate_tokens(&file.content);
            if tokens > config.per_file_limit_tokens {
                continue;  // Skip files exceeding per-file limit
            }
            if used_tokens + tokens > config.total_budget_tokens {
                break;  // Stop if total budget exceeded
            }

            restoration.files.push(RestoredFile {
                path: file.path.clone(),
                content: file.content.clone(),
                tokens,
            });
            used_tokens += tokens;
        }

        // 2. Restore todos, plan, skills, tasks (if space permits)
        if let Some(todos) = self.get_active_todos().await {
            restoration.todos = todos;
        }
        if let Some(plan) = self.get_current_plan().await {
            restoration.plan = Some(plan);
        }
        restoration.skills = self.get_active_skills();
        restoration.tasks = self.get_task_summaries();

        // 3. Add restoration as system attachment
        self.context.add_restoration_attachment(restoration);

        Ok(())
    }
}
```

---

### Timeout Constants

```rust
/// Session memory timing constants (v2.1.7 values)
pub const SESSION_MEMORY_WAIT_TIMEOUT_MS: i32 = 15000;  // Max wait for pending extraction
pub const SESSION_MEMORY_STALE_UPDATE_MS: i32 = 60000;  // Consider extraction stale after this
```

### Feature Flags

Session Memory Compact requires both flags to be enabled:

| Flag | Purpose |
|------|---------|
| `tengu_session_memory` | Enable background extraction agent |
| `tengu_sm_compact` | Enable Session Memory Compact (Tier 1) |

If either flag is disabled, auto-compact falls through directly to Full Compact (Tier 2).

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DISABLE_COMPACT` | false | Disable ALL compaction |
| `DISABLE_AUTO_COMPACT` | false | Disable only auto-compact (keep micro-compact) |
| `DISABLE_MICROCOMPACT` | false | Disable micro-compact |
| `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE` | - | Override auto-compact percentage (0-100) |
| `CLAUDE_CODE_BLOCKING_LIMIT_OVERRIDE` | - | Custom blocking limit |
| `CLAUDE_SESSION_MEMORY_MIN_TOKENS` | 10000 | Min tokens for session memory compact |
| `CLAUDE_SESSION_MEMORY_MAX_TOKENS` | 40000 | Max tokens for session memory summary |
| `CLAUDE_EXTRACTION_COOLDOWN` | 60 | Seconds between background extractions |
| `CLAUDE_CONTEXT_RESTORE_MAX_FILES` | 5 | Max files to restore after compact |
| `CLAUDE_CONTEXT_RESTORE_BUDGET` | 50000 | Total token budget for restoration |

---

---

## Extended Thinking Mode

### Overview

Extended thinking allows the model to use additional "thinking" tokens before responding, improving reasoning quality for complex tasks. The system supports **two thinking levels** that map to provider-specific mechanisms.

### Thinking Levels

| Level | Keyword | Description | Anthropic | OpenAI | Gemini |
|-------|---------|-------------|-----------|--------|--------|
| **DeepThink** | `deepthink` | Standard extended thinking | 10K-20K budget | High effort | Medium/High |
| **UltraThink** | `ultrathink` | Maximum reasoning | 32K-64K budget | XHigh effort | High |

### Core Types (in `common/thinking/`)

```rust
/// Two-level thinking abstraction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingLevel {
    #[default]
    None,
    DeepThink,    // Standard extended thinking
    UltraThink,   // Maximum reasoning capacity
}

/// Thinking configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThinkingConfig {
    pub level: ThinkingLevel,
    pub budget_tokens: Option<i32>,      // Explicit override
    pub interleaved: bool,               // Anthropic beta
}

/// Result of computing effective thinking
#[derive(Debug, Clone)]
pub struct ThinkingResult {
    pub level: ThinkingLevel,
    pub budget_tokens: Option<i32>,
    pub keyword_detected: Option<String>,
    pub source: ThinkingSource,
    pub explicit: bool,  // Should include in API request?
}

pub enum ThinkingSource {
    EnvOverride, Keyword, SessionToggle,
    PerTurnOverride, GlobalConfig, ModelDefault, Fallback,
}
```

### Priority Chain

The system uses a priority chain to determine the effective thinking level:

```
1. ENV override (COCODE_THINKING_LEVEL, COCODE_MAX_THINKING_TOKENS)  ← highest
2. Keyword detection ("ultrathink" > "deepthink")
3. Session toggle (hotkey cycling: None → DeepThink → UltraThink → None)
4. Per-turn override
5. Global config
6. Model default from ModelInfo/ModelProviderInfoExt
7. Fallback (None)                                                    ← lowest
```

### Keyword Detection

```rust
pub struct KeywordConfig {
    pub deepthink_keywords: Vec<String>,   // ["deepthink"]
    pub ultrathink_keywords: Vec<String>,  // ["ultrathink"]
}

/// Detect thinking keywords in message
pub fn detect_thinking_keyword(
    message: &str,
    config: &KeywordConfig,
) -> Option<(ThinkingLevel, String)>;

/// Extract positions for UI highlighting
pub fn extract_keyword_positions(
    message: &str,
    config: &KeywordConfig,
) -> Vec<(usize, usize, ThinkingLevel)>;
```

### Session State

```rust
/// Session-level thinking state for hotkey toggle
pub struct ThinkingState {
    current_level: Option<ThinkingLevel>,
}

impl ThinkingState {
    /// Cycle: None → DeepThink → UltraThink → None
    pub fn toggle_next(&mut self) -> Option<ThinkingLevel>;
    pub fn toggle_prev(&mut self) -> Option<ThinkingLevel>;
    pub fn set_level(&mut self, level: Option<ThinkingLevel>);
}
```

### Model Capability Override

```rust
/// Override thinking capability from ModelProviderInfoExt
pub struct ThinkingCapabilityOverride {
    pub supported_levels: Vec<ThinkingLevel>,
    pub default_level: Option<ThinkingLevel>,
    pub max_budget: Option<i32>,
    pub budget_by_level: Option<BudgetByLevel>,
}

pub struct BudgetByLevel {
    pub deep_think: i32,
    pub ultra_think: i32,
}
```

### Integration Function

```rust
/// Compute effective thinking configuration
pub fn compute_effective_thinking(
    message: &str,
    session_state: &ThinkingState,
    per_turn_config: Option<&ThinkingConfig>,
    global_config: Option<&ThinkingConfig>,
    model_info: &ModelInfo,
    provider_ext: Option<&ThinkingCapabilityOverride>,
    keyword_config: &KeywordConfig,
) -> ThinkingResult;
```

### hyper-sdk Boundary

**Important:** hyper-sdk does NOT handle thinking level mapping. The SDK simply accepts final provider-specific parameters:

```rust
// hyper-sdk options remain unchanged:
AnthropicOptions { thinking_budget_tokens: Option<i32>, .. }
OpenAIOptions { reasoning_effort: Option<ReasoningEffort>, .. }
GeminiOptions { thinking_level: Option<String>, .. }
```

**Mapping responsibility is in the application layer (caller):**

```rust
// Application layer (e.g., agent-loop) does the mapping:
let thinking_result = compute_effective_thinking(...);

// Convert to provider-specific options
let options = match provider {
    "anthropic" => AnthropicOptions {
        thinking_budget_tokens: thinking_result.budget_tokens,
        ..Default::default()
    },
    "openai" => OpenAIOptions {
        reasoning_effort: level_to_effort(thinking_result.level),
        ..Default::default()
    },
    _ => ..
};

// hyper-sdk just uses these final values
client.generate(request, options).await?;
```

### Configuration

#### Environment Variables

```bash
COCODE_THINKING_LEVEL=ultra_think        # Force level
COCODE_MAX_THINKING_TOKENS=32000         # Force budget (0 = disabled)
COCODE_INTERLEAVED_THINKING=true         # Anthropic beta
```

#### Config File

```toml
[thinking]
default_level = "none"         # none, deep_think, ultra_think
auto_enable = false            # Auto-enable for capable models
interleaved = false            # Anthropic beta

[thinking.keywords]
deepthink = ["deepthink", "think deeper"]
ultrathink = ["ultrathink", "maximum thinking"]

[thinking.budgets]
deep_think = 16000
ultra_think = 32000
```

#### Per-Model Override

```json
{
  "models": {
    "claude-sonnet-4": {
      "thinking_override": {
        "supported_levels": ["deep_think", "ultra_think"],
        "default_level": "none",
        "max_budget": 64000,
        "budget_by_level": { "deep_think": 16000, "ultra_think": 64000 }
      }
    }
  }
}
```

### Related Events

```rust
pub enum LoopEvent {
    // Thinking mode events
    ThinkingDelta { turn_id: String, delta: String },
    ThinkingComplete { turn_id: String, total_tokens: i32 },
    // ...
}
```

---

## Session Memory

### Overview

Session Memory is a **two-phase system** for preserving conversation context across compaction:

1. **Background Extraction Agent** - Maintains `summary.md` asynchronously during conversation
2. **Session Memory Compact** - Uses cached summary at compaction time (zero LLM cost)

This allows the agent to maintain awareness of the conversation even after older messages are pruned.

> **Note:** For detailed implementation, see the [Context Compaction](#context-compaction) section above, specifically:
> - [Background Extraction Agent](#background-extraction-agent-session-memory)
> - [Session Memory Compact (Tier 1)](#session-memory-compact-tier-1)

### File Cache (Separate from Summary)

In addition to the conversation summary, cocode maintains a **file cache** for recently read files. This is separate from the session memory summary and used for context restoration after Full Compact.

```rust
/// File cache for preserving file context
pub struct FileCache {
    /// Cached file contents
    pub files: HashMap<PathBuf, CachedFile>,
    /// Token budget for file cache (default: 50k)
    pub budget_tokens: i32,
}

#[derive(Debug, Clone)]
pub struct CachedFile {
    pub content: String,
    pub tokens: i32,
    pub last_read: SystemTime,
    pub access_count: i32,
}

impl FileCache {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            budget_tokens: 50000,
        }
    }

    /// Track file read
    pub fn track_read(&mut self, path: &Path, content: &str) {
        let tokens = estimate_tokens(content);

        if let Some(cached) = self.files.get_mut(path) {
            cached.access_count += 1;
            cached.last_read = SystemTime::now();
            cached.content = content.to_string();
            cached.tokens = tokens;
        } else {
            self.files.insert(path.to_path_buf(), CachedFile {
                content: content.to_string(),
                tokens,
                last_read: SystemTime::now(),
                access_count: 1,
            });
        }

        // Evict if over budget
        self.evict_if_needed();
    }

    /// Evict least recently used files if over budget
    fn evict_if_needed(&mut self) {
        let total: i32 = self.files.values().map(|f| f.tokens).sum();
        if total <= self.budget_tokens {
            return;
        }

        // Sort by last_read ascending (oldest first)
        let mut files: Vec<_> = self.files.iter().collect();
        files.sort_by(|a, b| a.1.last_read.cmp(&b.1.last_read));

        // Evict until under budget
        let mut current_total = total;
        for (path, cached) in files {
            if current_total <= self.budget_tokens {
                break;
            }
            current_total -= cached.tokens;
            self.files.remove(path);
        }
    }

    /// Get files for restoration (sorted by relevance)
    pub fn get_files_for_restoration(&self, max_files: i32, max_tokens: i32) -> Vec<RestoredFile> {
        let mut files: Vec<_> = self.files.iter().collect();
        // Sort by access_count descending (most accessed first)
        files.sort_by(|a, b| b.1.access_count.cmp(&a.1.access_count));

        let mut result = Vec::new();
        let mut used_tokens = 0;

        for (path, cached) in files.iter().take(max_files as usize) {
            if used_tokens + cached.tokens > max_tokens {
                break;
            }
            result.push(RestoredFile {
                path: (*path).clone(),
                content: cached.content.clone(),
                tokens: cached.tokens,
            });
            used_tokens += cached.tokens;
        }

        result
    }
}
```

### Integration with Compaction

The file cache is used during **Full Compact (Tier 2)** context restoration:

```rust
impl AgentLoop {
    async fn restore_context_after_compact(&mut self) -> Result<(), LoopError> {
        let config = &self.config.context_restoration;

        // Get files from cache (respecting limits)
        let files = self.file_cache.get_files_for_restoration(
            config.max_files,
            config.total_budget_tokens,
        );

        // Add as restoration attachment
        self.context.add_file_restoration(files);

        Ok(())
    }
}
```

---

## Dynamic Skill Reminders

### Overview

Skills can inject reminders at different verbosity levels based on context.

### Implementation

```rust
/// Reminder verbosity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReminderLevel {
    /// Minimal reminders (for experienced users)
    Sparse,
    /// Standard reminders
    #[default]
    Normal,
    /// Full context reminders (for complex tasks)
    Detailed,
}

/// Skill reminder configuration
pub struct SkillReminderConfig {
    /// Reminder verbosity level
    pub level: ReminderLevel,
    /// Include skill name in reminder
    pub include_name: bool,
}

impl Skill {
    /// Get reminder content based on level
    pub fn get_reminder(&self, level: ReminderLevel) -> Option<String> {
        match level {
            ReminderLevel::Sparse => self.sparse_reminder.clone(),
            ReminderLevel::Normal => self.normal_reminder.clone(),
            ReminderLevel::Detailed => self.detailed_reminder.clone(),
        }
    }
}
```

---

## Plan Mode Slug Generation

### Overview

Plan mode generates unique, memorable slugs for plan files.

### Implementation

```rust
/// Generate unique plan slug
pub fn generate_plan_slug() -> String {
    let adjectives = ["sparkling", "gentle", "swift", "quiet", "bold", "calm"];
    let nouns = ["fox", "river", "mountain", "forest", "cloud", "meadow"];
    let verbs = ["baking", "dancing", "running", "flying", "swimming", "climbing"];

    let mut rng = rand::thread_rng();

    format!(
        "{}-{}-{}",
        adjectives.choose(&mut rng).unwrap(),
        nouns.choose(&mut rng).unwrap(),
        verbs.choose(&mut rng).unwrap()
    )
}

// Example: "sparkling-fox-baking", "gentle-river-dancing"
```

---

## Feature Interaction Matrix

| Feature | Hooks | Skills | Plugins | Plan Mode | Compact | Thinking | Session Memory |
|---------|-------|--------|---------|-----------|---------|----------|----------------|
| Hooks | - | ✓ (skill hooks) | ✓ (plugin hooks) | - | ✓ (PreCompact) | - | - |
| Skills | ✓ | - | ✓ (from plugins) | - | - | - | - |
| Plugins | ✓ | ✓ | - | - | - | - | - |
| Plan Mode | - | - | - | - | - | - | - |
| Compact | ✓ | - | - | - | - | - | ✓ (restore) |
| Thinking | - | - | - | - | - | - | - |
| Session Memory | - | - | - | - | ✓ | - | - |
