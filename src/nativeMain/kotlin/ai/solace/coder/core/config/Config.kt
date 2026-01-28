// port-lint: source codex-rs/core/src/config/mod.rs
package ai.solace.coder.core.config

import ai.solace.coder.core.auth.AuthCredentialsStoreMode
import ai.solace.coder.core.model.ModelFamily
import ai.solace.coder.core.model.ModelProviderInfo
import ai.solace.coder.protocol.AskForApproval
import ai.solace.coder.protocol.ForcedLoginMethod
import ai.solace.coder.protocol.ReasoningEffort
import ai.solace.coder.protocol.ReasoningSummary
import ai.solace.coder.protocol.SandboxPolicy
import ai.solace.coder.protocol.Verbosity
import kotlinx.serialization.json.JsonElement

const val OPENAI_DEFAULT_MODEL: String = "gpt-5.1-codex"
const val OPENAI_DEFAULT_REVIEW_MODEL: String = "gpt-5.1-codex"
const val GPT_5_CODEX_MEDIUM_MODEL: String = "gpt-5.1-codex"
const val PROJECT_DOC_MAX_BYTES: Int = 32 * 1024 // 32 KiB
const val CONFIG_TOML_FILE: String = "config.toml"

/**
 * Project configuration resolved by checking cwd for git repo, worktree, etc.
 */
data class ProjectConfig(
    /** The root directory of the project. */
    val projectRoot: String? = null,
    /** Project documentation content loaded from AGENTS.md or similar. */
    val projectDoc: String? = null,
    /** Trust level for the project. */
    val trusted: Boolean = false
)

/**
 * Application configuration loaded from disk and merged with overrides.
 *
 * Ported from Rust codex-rs/core/src/config/mod.rs
 */
data class Config(
    /** The model slug to use. */
    val model: String,

    /** Model used specifically for review sessions. */
    val reviewModel: String = OPENAI_DEFAULT_REVIEW_MODEL,

    /** The model family derived from the slug. */
    val modelFamily: ModelFamily,

    /** Size of the context window for the model, in tokens. */
    val modelContextWindow: Long? = null,

    /** Token usage threshold triggering auto-compaction of conversation history. */
    val modelAutoCompactTokenLimit: Long? = null,

    /** Key into the model_providers map that specifies which provider to use. */
    val modelProviderId: String = "openai",

    /** Info needed to make an API request to the model. */
    val modelProvider: ModelProviderInfo? = null,

    /** Approval policy for executing commands. */
    val approvalPolicy: AskForApproval = AskForApproval.OnFailure,

    /** Sandbox policy derived from approval policy and sandbox mode. */
    val sandboxPolicy: SandboxPolicy = SandboxPolicy.AutoApprove,

    /** True if the user explicitly set approval_policy or sandbox_mode. */
    val didUserSetCustomApprovalPolicyOrSandboxMode: Boolean = false,

    /** On Windows, indicates workspace-write was coerced to read-only. */
    val forcedAutoModeDowngradedOnWindows: Boolean = false,

    /** Shell environment policy for spawned processes. */
    val shellEnvironmentPolicy: ShellEnvironmentPolicy = ShellEnvironmentPolicy(),

    /** When true, AgentReasoning events are suppressed from output. */
    val hideAgentReasoning: Boolean = false,

    /** When true, AgentReasoningRawContentEvent events will be shown. */
    val showRawAgentReasoning: Boolean = false,

    /** User-provided instructions from AGENTS.md. */
    val userInstructions: String? = null,

    /** Base instructions override. */
    val baseInstructions: String? = null,

    /** Developer instructions override injected as a separate message. */
    val developerInstructions: String? = null,

    /** Compact prompt override. */
    val compactPrompt: String? = null,

    /** External notifier command invoked after each completed turn. */
    val notify: List<String>? = null,

    /** TUI notifications preference. */
    val tuiNotifications: Notifications = Notifications.default(),

    /** Enable ASCII animations and shimmer effects in the TUI. */
    val animations: Boolean = true,

    /** The directory treated as the current working directory for the session. */
    val cwd: String = ".",

    /** Preferred store for CLI auth credentials. */
    val cliAuthCredentialsStoreMode: AuthCredentialsStoreMode = AuthCredentialsStoreMode.File,

    /** Definition for MCP servers that Codex can reach out to for tool calls. */
    val mcpServers: Map<String, McpServerConfig> = emptyMap(),

    /** Combined provider map (defaults merged with user-defined overrides). */
    val modelProviders: Map<String, ModelProviderInfo> = emptyMap(),

    /** Maximum number of bytes to include from an AGENTS.md project doc file. */
    val projectDocMaxBytes: Int = PROJECT_DOC_MAX_BYTES,

    /** Additional filenames to try when looking for project-level docs. */
    val projectDocFallbackFilenames: List<String> = emptyList(),

    /** Token budget applied when storing tool/function outputs in context manager. */
    val toolOutputTokenLimit: Int? = null,

    /** Directory containing all Codex state (defaults to ~/.codex). */
    val codexHome: String = "",

    /** Settings for history.jsonl persistence. */
    val history: History = History(),

    /** Optional URI-based file opener for citations. */
    val fileOpener: UriBasedFileOpener = UriBasedFileOpener.None,

    /** Path to the codex-linux-sandbox executable. */
    val codexLinuxSandboxExe: String? = null,

    /** Value for reasoning.effort when using Responses API. */
    val modelReasoningEffort: ReasoningEffort? = null,

    /** Value for reasoning.summary when using Responses API. */
    val modelReasoningSummary: ReasoningSummary = ReasoningSummary.None,

    /** Verbosity control for GPT-5 models (Responses API text.verbosity). */
    val modelVerbosity: Verbosity? = null,

    /** Base URL for requests to ChatGPT. */
    val chatgptBaseUrl: String = "https://chatgpt.com",

    /** When set, restricts ChatGPT login to a specific workspace. */
    val forcedChatgptWorkspaceId: String? = null,

    /** When set, restricts the login mechanism users may use. */
    val forcedLoginMethod: ForcedLoginMethod? = null,

    /** Include the apply_patch tool for models that benefit from structured tool calls. */
    val includeApplyPatchTool: Boolean = false,

    /** Enable web search tool. */
    val toolsWebSearchRequest: Boolean = false,

    /** Run model-based assessment for commands denied by the sandbox. */
    val experimentalSandboxCommandAssessment: Boolean = false,

    /** Use experimental unified exec tool. */
    val useExperimentalUnifiedExecTool: Boolean = false,

    /** Use experimental official Rust MCP client. */
    val useExperimentalUseRmcpClient: Boolean = false,

    /** The active profile name used to derive this Config (if any). */
    val activeProfile: String? = null,

    /** The currently active project config. */
    val activeProject: ProjectConfig = ProjectConfig(),

    /** Tracks whether the Windows onboarding screen has been acknowledged. */
    val windowsWslSetupAcknowledged: Boolean = false,

    /** Collection of various notices we show the user. */
    val notices: Notice = Notice(),

    /** Check for Codex updates on startup. */
    val checkForUpdateOnStartup: Boolean = true,

    /** Disables burst-paste detection for typed input. */
    val disablePasteBurst: Boolean = false,

    /** OTEL configuration. */
    val otel: OtelConfig = OtelConfig(),

    /** Optional JSON output schema for structured output. */
    val outputSchema: JsonElement? = null,

    /** List of tools available. */
    val tools: List<Any> = emptyList()
)
