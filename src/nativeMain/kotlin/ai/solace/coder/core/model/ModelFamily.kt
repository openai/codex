// port-lint: source codex-rs/core/src/model_family.rs
package ai.solace.coder.core.model

import ai.solace.coder.core.context.TruncationPolicy
import ai.solace.coder.protocol.ReasoningEffort
import ai.solace.coder.protocol.Verbosity

/**
 * Format for reasoning summaries.
 */
enum class ReasoningSummaryFormat {
    None,
    Experimental
}

/**
 * Type of apply_patch tool to use.
 */
enum class ApplyPatchToolType {
    Function,
    Freeform
}

/**
 * Shell tool type configuration.
 */
enum class ConfigShellToolType {
    Default,
    Local,
    ShellCommand,
    UnifiedExec
}

/**
 * A model family is a group of models that share certain characteristics.
 *
 * Ported from Rust codex-rs/core/src/model_family.rs
 */
data class ModelFamily(
    /** The full model slug used to derive this model family, e.g. "gpt-4.1-2025-04-14". */
    val slug: String,

    /** The model family name, e.g. "gpt-4.1". */
    val family: String,

    /** True if the model needs additional instructions on how to use the "virtual" `apply_patch` CLI. */
    val needsSpecialApplyPatchInstructions: Boolean = false,

    /** Whether the `reasoning` field can be set when making a request to this model family. */
    val supportsReasoningSummaries: Boolean = false,

    /** The reasoning effort to use for this model family when none is explicitly chosen. */
    val defaultReasoningEffort: ReasoningEffort? = null,

    /** Define if we need a special handling of reasoning summary. */
    val reasoningSummaryFormat: ReasoningSummaryFormat = ReasoningSummaryFormat.None,

    /** Whether this model supports parallel tool calls when using the Responses API. */
    val supportsParallelToolCalls: Boolean = false,

    /** Present if the model performs better when `apply_patch` is provided as a tool call instead of just a bash command. */
    val applyPatchToolType: ApplyPatchToolType? = null,

    /** Instructions to use for querying the model. */
    val baseInstructions: String = DEFAULT_BASE_INSTRUCTIONS,

    /** Names of beta tools that should be exposed to this model family. */
    val experimentalSupportedTools: List<String> = emptyList(),

    /** Percentage of the context window considered usable for inputs. */
    val effectiveContextWindowPercent: Int = 95,

    /** If the model family supports setting the verbosity level when using Responses API. */
    val supportVerbosity: Boolean = false,

    /** The default verbosity level for this model family when using Responses API. */
    val defaultVerbosity: Verbosity? = null,

    /** Preferred shell tool type for this model family when features do not override it. */
    val shellType: ConfigShellToolType = ConfigShellToolType.Default,

    /** Truncation policy for output. */
    val truncationPolicy: TruncationPolicy = TruncationPolicy.Bytes(10_000),

    // Additional fields for API compatibility
    val contextWindow: Long = 128_000,
    val autoCompactTokenLimit: Long? = null
) {
    companion object {
        const val DEFAULT_BASE_INSTRUCTIONS = """You are a helpful AI coding assistant."""
    }
}

/**
 * Returns a `ModelFamily` for the given model slug, or `null` if the slug
 * does not match any known model family.
 */
fun findFamilyForModel(slug: String): ModelFamily? {
    return when {
        slug.startsWith("o3") -> ModelFamily(
            slug = slug,
            family = "o3",
            supportsReasoningSummaries = true,
            needsSpecialApplyPatchInstructions = true
        )

        slug.startsWith("o4-mini") -> ModelFamily(
            slug = slug,
            family = "o4-mini",
            supportsReasoningSummaries = true,
            needsSpecialApplyPatchInstructions = true
        )

        slug.startsWith("codex-mini-latest") -> ModelFamily(
            slug = slug,
            family = "codex-mini-latest",
            supportsReasoningSummaries = true,
            needsSpecialApplyPatchInstructions = true,
            shellType = ConfigShellToolType.Local
        )

        slug.startsWith("gpt-4.1") -> ModelFamily(
            slug = slug,
            family = "gpt-4.1",
            needsSpecialApplyPatchInstructions = true
        )

        slug.startsWith("gpt-oss") || slug.startsWith("openai/gpt-oss") -> ModelFamily(
            slug = slug,
            family = "gpt-oss",
            applyPatchToolType = ApplyPatchToolType.Function
        )

        slug.startsWith("gpt-4o") -> ModelFamily(
            slug = slug,
            family = "gpt-4o",
            needsSpecialApplyPatchInstructions = true
        )

        slug.startsWith("gpt-3.5") -> ModelFamily(
            slug = slug,
            family = "gpt-3.5",
            needsSpecialApplyPatchInstructions = true
        )

        slug.startsWith("gpt-5.1-codex-max") -> ModelFamily(
            slug = slug,
            family = slug,
            supportsReasoningSummaries = true,
            reasoningSummaryFormat = ReasoningSummaryFormat.Experimental,
            applyPatchToolType = ApplyPatchToolType.Freeform,
            shellType = ConfigShellToolType.ShellCommand,
            supportsParallelToolCalls = true,
            supportVerbosity = false,
            truncationPolicy = TruncationPolicy.Tokens(10_000)
        )

        slug.startsWith("gpt-5-codex") || slug.startsWith("gpt-5.1-codex") || slug.startsWith("codex-") -> ModelFamily(
            slug = slug,
            family = slug,
            supportsReasoningSummaries = true,
            reasoningSummaryFormat = ReasoningSummaryFormat.Experimental,
            applyPatchToolType = ApplyPatchToolType.Freeform,
            shellType = ConfigShellToolType.ShellCommand,
            supportsParallelToolCalls = true,
            supportVerbosity = false,
            truncationPolicy = TruncationPolicy.Tokens(10_000)
        )

        slug.startsWith("gpt-5.1") -> ModelFamily(
            slug = slug,
            family = "gpt-5.1",
            supportsReasoningSummaries = true,
            applyPatchToolType = ApplyPatchToolType.Freeform,
            supportVerbosity = true,
            defaultVerbosity = Verbosity.Low,
            defaultReasoningEffort = ReasoningEffort.Medium,
            truncationPolicy = TruncationPolicy.Bytes(10_000),
            shellType = ConfigShellToolType.ShellCommand,
            supportsParallelToolCalls = true
        )

        slug.startsWith("gpt-5") -> ModelFamily(
            slug = slug,
            family = "gpt-5",
            supportsReasoningSummaries = true,
            needsSpecialApplyPatchInstructions = true,
            shellType = ConfigShellToolType.Default,
            supportVerbosity = true,
            truncationPolicy = TruncationPolicy.Bytes(10_000)
        )

        slug.startsWith("exp-") -> ModelFamily(
            slug = slug,
            family = slug,
            supportsReasoningSummaries = true,
            applyPatchToolType = ApplyPatchToolType.Freeform,
            supportVerbosity = true,
            defaultVerbosity = Verbosity.Low,
            defaultReasoningEffort = ReasoningEffort.Medium,
            truncationPolicy = TruncationPolicy.Bytes(10_000),
            shellType = ConfigShellToolType.UnifiedExec,
            supportsParallelToolCalls = true
        )

        else -> null
    }
}

/**
 * Returns a default `ModelFamily` for models that don't match any known family.
 */
fun deriveDefaultModelFamily(model: String): ModelFamily {
    return ModelFamily(
        slug = model,
        family = model,
        needsSpecialApplyPatchInstructions = false,
        supportsReasoningSummaries = false,
        reasoningSummaryFormat = ReasoningSummaryFormat.None,
        supportsParallelToolCalls = false,
        applyPatchToolType = null,
        effectiveContextWindowPercent = 95,
        supportVerbosity = false,
        shellType = ConfigShellToolType.Default,
        defaultVerbosity = null,
        defaultReasoningEffort = null,
        truncationPolicy = TruncationPolicy.Bytes(10_000)
    )
}
