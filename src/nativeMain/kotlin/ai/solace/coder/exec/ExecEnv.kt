// port-lint: source codex-rs/core/src/exec_env.rs
package ai.solace.coder.exec

import ai.solace.coder.core.config.ShellEnvironmentPolicy
import ai.solace.coder.core.config.ShellEnvironmentPolicyInherit
import platform.posix.getenv
import kotlinx.cinterop.toKString

/**
 * Core environment variables that should be inherited in "core" mode.
 */
private val CORE_VARS = setOf(
    "HOME", "LOGNAME", "PATH", "SHELL", "USER", "USERNAME", "TMPDIR", "TEMP", "TMP"
)

/**
 * Default exclude patterns for environment variables containing secrets.
 */
private val DEFAULT_EXCLUDE_PATTERNS = listOf("*KEY*", "*SECRET*", "*TOKEN*")

/**
 * Construct an environment map based on the rules in the specified policy.
 * The resulting map can be passed to process builders after clearing the
 * inherited environment.
 *
 * The derivation follows the algorithm documented in ShellEnvironmentPolicy:
 * 1. Create initial map based on `inherit`
 * 2. If `ignoreDefaultExcludes` is false, filter using default patterns
 * 3. If `exclude` is not empty, filter using provided patterns
 * 4. Insert entries from `set`
 * 5. If non-empty, filter using `includeOnly` patterns
 */
fun createEnv(policy: ShellEnvironmentPolicy): Map<String, String> {
    // In Kotlin Native, we'd get current env vars from the platform
    // For now, use an empty starting map; real implementation would enumerate env vars
    val currentEnvVars = getCurrentEnvironmentVars()
    return populateEnv(currentEnvVars, policy)
}

/**
 * Internal function that builds the environment map from given vars and policy.
 * Exposed for testing.
 */
internal fun populateEnv(
    vars: List<Pair<String, String>>,
    policy: ShellEnvironmentPolicy
): Map<String, String> {
    // Step 1: Determine starting set based on inherit strategy
    val envMap = when (policy.inherit) {
        ShellEnvironmentPolicyInherit.All -> vars.toMap().toMutableMap()
        ShellEnvironmentPolicyInherit.None -> mutableMapOf()
        ShellEnvironmentPolicyInherit.Core -> {
            vars.filter { (k, _) -> k in CORE_VARS }
                .toMap()
                .toMutableMap()
        }
    }

    // Step 2: Apply default excludes if not disabled
    if (!policy.ignoreDefaultExcludes) {
        envMap.keys.removeAll { name ->
            matchesAnyPattern(name, DEFAULT_EXCLUDE_PATTERNS)
        }
    }

    // Step 3: Apply custom excludes
    if (policy.exclude.isNotEmpty()) {
        envMap.keys.removeAll { name ->
            matchesAnyPattern(name, policy.exclude)
        }
    }

    // Step 4: Apply user-provided overrides
    for ((key, value) in policy.set) {
        envMap[key] = value
    }

    // Step 5: If include_only is non-empty, keep only matching vars
    if (policy.includeOnly.isNotEmpty()) {
        envMap.keys.retainAll { name ->
            matchesAnyPattern(name, policy.includeOnly)
        }
    }

    return envMap
}

/**
 * Check if a name matches any of the given wildcard patterns.
 * Supports simple glob patterns with '*' and '?' (case-insensitive).
 */
private fun matchesAnyPattern(name: String, patterns: List<String>): Boolean {
    return patterns.any { pattern -> wildcardMatch(name, pattern) }
}

/**
 * Simple case-insensitive wildcard matcher.
 * Supports '*' (matches any sequence) and '?' (matches single char).
 */
internal fun wildcardMatch(text: String, pattern: String): Boolean {
    val t = text.lowercase()
    val p = pattern.lowercase()
    return wildcardMatchImpl(t, 0, p, 0)
}

private fun wildcardMatchImpl(text: String, ti: Int, pattern: String, pi: Int): Boolean {
    if (pi == pattern.length) return ti == text.length
    if (pattern[pi] == '*') {
        // Try matching * with 0..n characters
        for (i in ti..text.length) {
            if (wildcardMatchImpl(text, i, pattern, pi + 1)) return true
        }
        return false
    }
    if (ti == text.length) return false
    if (pattern[pi] == '?' || pattern[pi] == text[ti]) {
        return wildcardMatchImpl(text, ti + 1, pattern, pi + 1)
    }
    return false
}

/**
 * Get current environment variables.
 * In Kotlin Native, there's no direct way to enumerate all env vars,
 * so we check for common ones. A real implementation would use
 * platform-specific APIs.
 */
private fun getCurrentEnvironmentVars(): List<Pair<String, String>> {
    val result = mutableListOf<Pair<String, String>>()
    val commonVars = listOf(
        "HOME", "LOGNAME", "PATH", "SHELL", "USER", "USERNAME",
        "TMPDIR", "TEMP", "TMP", "LANG", "LC_ALL", "TERM",
        "EDITOR", "VISUAL", "DISPLAY", "XDG_RUNTIME_DIR",
        "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME",
        "CODEX_HOME", "CODEX_MODEL", "CODEX_PROFILE",
        "OPENAI_API_KEY", "OPENAI_ORGANIZATION", "OPENAI_PROJECT",
        "HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY",
        "SSH_AUTH_SOCK", "GPG_AGENT_INFO"
    )
    for (name in commonVars) {
        val value = getenv(name)?.toKString()
        if (value != null) {
            result.add(name to value)
        }
    }
    return result
}
