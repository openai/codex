package ai.solace.coder.core.features

/**
 * Centralized feature flags and metadata.
 *
 * This module defines a small set of toggles that gate experimental and
 * optional behavior across the codebase. Instead of wiring individual
 * booleans through multiple types, call sites consult a single Features
 * container attached to Config.
 *
 * Ported from Rust codex-rs/core/src/features.rs
 */

/**
 * High-level lifecycle stage for a feature.
 */
enum class Stage {
    Experimental,
    Beta,
    Stable,
    Deprecated,
    Removed
}

/**
 * Unique features toggled via configuration.
 */
enum class Feature(
    val key: String,
    val stage: Stage,
    val defaultEnabled: Boolean
) {
    /** Create a ghost commit at each turn. */
    GhostCommit("undo", Stage.Stable, true),

    /** Use the single unified PTY-backed exec tool. */
    UnifiedExec("unified_exec", Stage.Experimental, false),

    /** Enable experimental RMCP features such as OAuth login. */
    RmcpClient("rmcp_client", Stage.Experimental, false),

    /** Include the freeform apply_patch tool. */
    ApplyPatchFreeform("apply_patch_freeform", Stage.Beta, false),

    /** Include the view_image tool. */
    ViewImageTool("view_image_tool", Stage.Stable, true),

    /** Allow the model to request web searches. */
    WebSearchRequest("web_search_request", Stage.Stable, false),

    /** Gate the execpolicy enforcement for shell/unified exec. */
    ExecPolicy("exec_policy", Stage.Experimental, true),

    /** Enable the model-based risk assessments for sandboxed commands. */
    SandboxCommandAssessment("experimental_sandbox_command_assessment", Stage.Experimental, false),

    /** Enable Windows sandbox (restricted token) on Windows. */
    WindowsSandbox("enable_experimental_windows_sandbox", Stage.Experimental, false),

    /** Remote compaction enabled (only for ChatGPT auth). */
    RemoteCompaction("remote_compaction", Stage.Experimental, true),

    /** Enable the default shell tool. */
    ShellTool("shell_tool", Stage.Stable, true),

    /** Allow model to call multiple tools in parallel (only for models supporting it). */
    ParallelToolCalls("parallel", Stage.Experimental, false);

    companion object {
        private val keyMap: Map<String, Feature> = entries.associateBy { it.key }

        /**
         * Find a feature by its key.
         */
        fun forKey(key: String): Feature? = keyMap[key] ?: forLegacyKey(key)

        /**
         * Check if a key is a known feature key.
         */
        fun isKnownKey(key: String): Boolean = forKey(key) != null

        /**
         * Legacy key mappings for backwards compatibility.
         */
        private fun forLegacyKey(key: String): Feature? {
            return when (key) {
                "use_experimental_unified_exec_tool" -> UnifiedExec
                "experimental_use_unified_exec_tool" -> UnifiedExec
                "include_apply_patch_tool" -> ApplyPatchFreeform
                "experimental_use_freeform_apply_patch" -> ApplyPatchFreeform
                "experimental_use_rmcp_client" -> RmcpClient
                "tools_web_search" -> WebSearchRequest
                "tools_view_image" -> ViewImageTool
                else -> null
            }
        }
    }
}

/**
 * Record of a legacy feature key usage.
 */
data class LegacyFeatureUsage(
    val alias: String,
    val feature: Feature
)

/**
 * Holds the effective set of enabled features.
 */
class Features private constructor(
    private val enabled: MutableSet<Feature>,
    private val legacyUsages: MutableSet<LegacyFeatureUsage>
) {
    constructor() : this(mutableSetOf(), mutableSetOf())

    /**
     * Check if a feature is enabled.
     */
    fun enabled(feature: Feature): Boolean = enabled.contains(feature)

    /**
     * Enable a feature.
     */
    fun enable(feature: Feature): Features {
        enabled.add(feature)
        return this
    }

    /**
     * Disable a feature.
     */
    fun disable(feature: Feature): Features {
        enabled.remove(feature)
        return this
    }

    /**
     * Record legacy feature usage (for deprecation notices).
     */
    fun recordLegacyUsage(alias: String, feature: Feature) {
        if (alias == feature.key) {
            return
        }
        recordLegacyUsageForce(alias, feature)
    }

    /**
     * Force record legacy feature usage.
     */
    fun recordLegacyUsageForce(alias: String, feature: Feature) {
        legacyUsages.add(LegacyFeatureUsage(alias, feature))
    }

    /**
     * Get iterator of legacy feature usages.
     */
    fun legacyFeatureUsages(): Sequence<Pair<String, Feature>> {
        return legacyUsages.asSequence().map { it.alias to it.feature }
    }

    /**
     * Apply a map of key -> bool toggles.
     */
    fun applyMap(map: Map<String, Boolean>) {
        for ((key, value) in map) {
            val feature = Feature.forKey(key)
            if (feature != null) {
                if (key != feature.key) {
                    recordLegacyUsage(key, feature)
                }
                if (value) {
                    enable(feature)
                } else {
                    disable(feature)
                }
            } else {
                // Log warning for unknown feature key
                println("Warning: unknown feature key in config: $key")
            }
        }
    }

    /**
     * Get list of enabled features.
     */
    fun enabledFeatures(): List<Feature> = enabled.toList()

    /**
     * Copy this features instance.
     */
    fun copy(): Features = Features(enabled.toMutableSet(), legacyUsages.toMutableSet())

    companion object {
        /**
         * Create a Features instance with default values.
         */
        fun withDefaults(): Features {
            val features = Features()
            for (feature in Feature.entries) {
                if (feature.defaultEnabled) {
                    features.enable(feature)
                }
            }
            return features
        }
    }
}

/**
 * Feature overrides from various sources.
 */
data class FeatureOverrides(
    val includeApplyPatchTool: Boolean? = null,
    val webSearchRequest: Boolean? = null,
    val experimentalSandboxCommandAssessment: Boolean? = null
) {
    /**
     * Apply overrides to a Features instance.
     */
    fun apply(features: Features) {
        includeApplyPatchTool?.let { value ->
            if (value) {
                features.enable(Feature.ApplyPatchFreeform)
            } else {
                features.disable(Feature.ApplyPatchFreeform)
            }
        }
        webSearchRequest?.let { value ->
            if (value) {
                features.enable(Feature.WebSearchRequest)
            } else {
                features.disable(Feature.WebSearchRequest)
            }
        }
        experimentalSandboxCommandAssessment?.let { value ->
            if (value) {
                features.enable(Feature.SandboxCommandAssessment)
            } else {
                features.disable(Feature.SandboxCommandAssessment)
            }
        }
    }
}
