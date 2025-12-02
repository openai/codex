// port-lint: source codex-rs/core/src/auth.rs
package ai.solace.coder.core

import ai.solace.coder.core.auth.AuthCredentialsStoreMode
import ai.solace.coder.core.auth.AuthStorageBackend
import ai.solace.coder.core.auth.FileAuthStorage
import ai.solace.coder.core.auth.createAuthStorage
import io.ktor.client.*
import io.ktor.client.request.*
import io.ktor.client.statement.*
import io.ktor.http.*
import kotlinx.cinterop.toKString
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.io.files.Path
import kotlinx.serialization.Contextual
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.*
import kotlin.time.TimeSource
import kotlin.time.Duration.Companion.days
import kotlin.time.ExperimentalTime

// ============================================================================
// Constants
// ============================================================================

@OptIn(ExperimentalTime::class)
private val systemClock = TimeSource.Monotonic

private const val TOKEN_REFRESH_INTERVAL = 8L // days

private const val REFRESH_TOKEN_EXPIRED_MESSAGE =
    "Your access token could not be refreshed because your refresh token has expired. Please log out and sign in again."
private const val REFRESH_TOKEN_REUSED_MESSAGE =
    "Your access token could not be refreshed because your refresh token was already used. Please log out and sign in again."
private const val REFRESH_TOKEN_INVALIDATED_MESSAGE =
    "Your access token could not be refreshed because your refresh token was revoked. Please log out and sign in again."
private const val REFRESH_TOKEN_UNKNOWN_MESSAGE =
    "Your access token could not be refreshed. Please log out and sign in again."

private const val REFRESH_TOKEN_URL = "https://auth.openai.com/oauth/token"
const val REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR = "CODEX_REFRESH_TOKEN_URL_OVERRIDE"
const val CLIENT_ID = "app_EMoamEEZ73f0CkXaXp7hrann"

const val OPENAI_API_KEY_ENV_VAR = "OPENAI_API_KEY"
const val CODEX_API_KEY_ENV_VAR = "CODEX_API_KEY"

// ============================================================================
// Core Types
// ============================================================================

/**
 * Authentication mode for API access.
 * Mirrors codex_app_server_protocol::AuthMode
 */
enum class AuthMode {
    ApiKey,
    ChatGPT
}

@ConsistentCopyVisibility
data class CodexAuth internal constructor(
    val mode: AuthMode,
    internal val apiKey: String?,
    private val authDotJsonMutex: Mutex,
    private var cachedAuthDotJson: AuthDotJson?,
    private val storage: AuthStorageBackend,
    internal val client: HttpClient
) {

    /**
     * Refresh the access token using the refresh token.
     * Returns the new access token on success.
     */
    suspend fun refreshToken(): Result<String> {
        println("Refreshing token")

        val tokenData = getCurrentTokenData()
            ?: return Result.failure(RefreshTokenError.Transient("Token data is not available."))

        val refreshResponse = tryRefreshToken(tokenData.refreshToken, client)
            .getOrElse { return Result.failure(it) }

        val updated = updateTokens(
            storage,
            refreshResponse.idToken,
            refreshResponse.accessToken,
            refreshResponse.refreshToken
        ).getOrElse { return Result.failure(it) }

        authDotJsonMutex.withLock {
            cachedAuthDotJson = updated
        }

        val access = updated.tokens?.accessToken
            ?: return Result.failure(RefreshTokenError.Transient("Token data is not available after refresh."))

        return Result.success(access)
    }

    /**
     * Get token data, refreshing if necessary based on last refresh time.
     */
    @OptIn(ExperimentalTime::class)
    suspend fun getTokenData(): Result<TokenData> {
        val authJson = getCurrentAuthJson()
            ?: return Result.failure(Exception("Token data is not available."))

        var tokens = authJson.tokens
            ?: return Result.failure(Exception("Token data is not available."))

        val lastRefresh = authJson.lastRefresh

        // Check if token needs refresh (8 days old)
        if (lastRefresh != null) {
            val elapsedMillis = systemClock.markNow().elapsedNow().inWholeMilliseconds
            val refreshAgeMillis = elapsedMillis - lastRefresh

            if (refreshAgeMillis > TOKEN_REFRESH_INTERVAL.days.inWholeMilliseconds) {
                val refreshResult = tryRefreshToken(tokens.refreshToken, client)
                val refreshResponse = refreshResult.getOrElse {
                    return Result.failure(it)
                }

                val updatedAuthJson = updateTokens(
                    storage,
                    refreshResponse.idToken,
                    refreshResponse.accessToken,
                    refreshResponse.refreshToken
                ).getOrElse { return Result.failure(it) }

                tokens = updatedAuthJson.tokens
                    ?: return Result.failure(Exception("Token data is not available after refresh."))

                authDotJsonMutex.withLock {
                    cachedAuthDotJson = updatedAuthJson
                }
            }
        }

        return Result.success(tokens)
    }

    /**
     * Get the current access token (API key or ChatGPT access token).
     */
    suspend fun getToken(): Result<String> {
        return when (mode) {
            AuthMode.ApiKey -> Result.success(apiKey ?: "")
            AuthMode.ChatGPT -> {
                val tokenData = getTokenData().getOrElse {
                    return Result.failure(it)
                }
                Result.success(tokenData.accessToken)
            }
        }
    }

    /**
     * Get the ChatGPT account ID from the token data.
     */
    fun getAccountId(): String? {
        return getCurrentTokenData()?.accountId
    }

    /**
     * Get the email from the ID token.
     */
    fun getAccountEmail(): String? {
        return getCurrentTokenData()?.idToken?.email
    }

    /**
     * Account-facing plan classification derived from the current token.
     */
    fun accountPlanType(): AccountPlanType? {
        val tokenData = getCurrentTokenData() ?: return null
        return when (val planType = tokenData.idToken.chatgptPlanType) {
            is PlanType.Known -> when (planType.plan) {
                KnownPlan.Free -> AccountPlanType.Free
                KnownPlan.Plus -> AccountPlanType.Plus
                KnownPlan.Pro -> AccountPlanType.Pro
                KnownPlan.Team -> AccountPlanType.Team
                KnownPlan.Business -> AccountPlanType.Business
                KnownPlan.Enterprise -> AccountPlanType.Enterprise
                KnownPlan.Edu -> AccountPlanType.Edu
            }
            is PlanType.Unknown -> AccountPlanType.Unknown
            null -> null
        }
    }

    /**
     * Raw plan string from the ID token (including unknown/new plan types).
     */
    fun rawPlanType(): String? {
        return getPlanType()?.let { planType ->
            when (planType) {
                is PlanType.Known -> planType.plan.name
                is PlanType.Unknown -> planType.value
            }
        }
    }

    /**
     * Raw internal plan value from the ID token.
     */
    internal fun getPlanType(): PlanType? {
        return getCurrentTokenData()?.idToken?.chatgptPlanType
    }

    private suspend fun getCurrentAuthJson(): AuthDotJson? {
        return authDotJsonMutex.withLock { cachedAuthDotJson }
    }

    private fun getCurrentTokenData(): TokenData? {
        return cachedAuthDotJson?.tokens
    }

    companion object {
        /**
         * Create a dummy ChatGPT auth for testing.
         */
        @OptIn(ExperimentalTime::class)
        fun createDummyChatGptAuthForTesting(): CodexAuth {
            val authDotJson = AuthDotJson(
                openaiApiKey = null,
                tokens = TokenData(
                    idToken = IdTokenInfo(),
                    accessToken = "Access Token",
                    refreshToken = "test",
                    accountId = "account_id"
                ),
                lastRefresh = systemClock.markNow().elapsedNow().inWholeMilliseconds
            )

            return CodexAuth(
                mode = AuthMode.ChatGPT,
                apiKey = null,
                authDotJsonMutex = Mutex(),
                cachedAuthDotJson = authDotJson,
                storage = FileAuthStorage(Path("")),
                client = HttpClient()
            )
        }

        /**
         * Create an auth from an API key.
         */
        fun fromApiKey(apiKey: String, client: HttpClient = HttpClient()): CodexAuth {
            return CodexAuth(
                mode = AuthMode.ApiKey,
                apiKey = apiKey,
                authDotJsonMutex = Mutex(),
                cachedAuthDotJson = null,
                storage = FileAuthStorage(Path("")),
                client = client
            )
        }

        /**
         * Loads the available auth information from auth storage.
         */
        fun fromAuthStorage(
            codexHome: Path,
            authCredentialsStoreMode: AuthCredentialsStoreMode
        ): Result<CodexAuth?> {
            return loadAuth(codexHome, enableCodexApiKeyEnv = false, authCredentialsStoreMode)
        }
    }
}

// ============================================================================
// Error Types
// ============================================================================

/**
 * Error types for refresh token operations.
 * Mirrors Rust's RefreshTokenError enum.
 */
sealed class RefreshTokenError(message: String) : Exception(message) {
    class Permanent(val reason: RefreshTokenFailedReason, message: String) : RefreshTokenError(message)
    class Transient(message: String) : RefreshTokenError(message)

    fun failedReason(): RefreshTokenFailedReason? {
        return when (this) {
            is Permanent -> reason
            is Transient -> null
        }
    }
}

enum class RefreshTokenFailedReason {
    Expired,
    Exhausted,
    Revoked,
    Other
}

data class RefreshTokenFailedError(
    val reason: RefreshTokenFailedReason,
    override val message: String
) : Exception(message)

// ============================================================================
// Token Data & Storage Types
// ============================================================================

/**
 * Plan type classification from ID token.
 */
sealed class PlanType {
    data class Known(val plan: KnownPlan) : PlanType()
    data class Unknown(val value: String) : PlanType()

    companion object {
        fun fromString(value: String): PlanType {
            val knownPlan = when (value.lowercase()) {
                "free" -> KnownPlan.Free
                "plus" -> KnownPlan.Plus
                "pro" -> KnownPlan.Pro
                "team" -> KnownPlan.Team
                "business" -> KnownPlan.Business
                "enterprise" -> KnownPlan.Enterprise
                "edu" -> KnownPlan.Edu
                else -> null
            }
            return if (knownPlan != null) {
                Known(knownPlan)
            } else {
                Unknown(value)
            }
        }
    }
}

enum class KnownPlan {
    Free, Plus, Pro, Team, Business, Enterprise, Edu
}

/**
 * Account plan type for external API.
 * Maps to codex_protocol::account::PlanType
 */
enum class AccountPlanType {
    Free, Plus, Pro, Team, Business, Enterprise, Edu, Unknown
}

/**
 * ID token information parsed from JWT.
 */
@Serializable
data class IdTokenInfo(
    val email: String? = null,
    @Contextual
    val chatgptPlanType: PlanType? = null,
    val chatgptAccountId: String? = null,
    val rawJwt: String = ""
)

/**
 * Token data from auth.json.
 */
@Serializable
data class TokenData(
    var idToken: IdTokenInfo = IdTokenInfo(),
    var accessToken: String = "",
    var refreshToken: String = "",
    val accountId: String? = null
)

/**
 * Auth.json file structure.
 */
@Serializable
data class AuthDotJson(
    @SerialName("OPENAI_API_KEY")
    val openaiApiKey: String? = null,
    val tokens: TokenData? = null,
    @SerialName("last_refresh")
    val lastRefresh: Long? = null  // Store as epoch milliseconds
)

/**
 * Forced login method configuration.
 */
enum class ForcedLoginMethod {
    Api,
    Chatgpt
}

// ============================================================================
// Public API Functions
// ============================================================================
// ============================================================================

/**
 * Read OpenAI API key from environment.
 */
fun readOpenaiApiKeyFromEnv(): String? {
    return getEnvironmentVariable(OPENAI_API_KEY_ENV_VAR)
        ?.trim()
        ?.takeIf { it.isNotEmpty() }
}

/**
 * Read Codex API key from environment.
 */
fun readCodexApiKeyFromEnv(): String? {
    return getEnvironmentVariable(CODEX_API_KEY_ENV_VAR)
        ?.trim()
        ?.takeIf { it.isNotEmpty() }
}

/**
 * Delete the auth.json file. Returns true if a file was removed.
 */
fun logout(
    codexHome: Path,
    authCredentialsStoreMode: AuthCredentialsStoreMode
): Result<Boolean> {
    val storage = createAuthStorage(codexHome, authCredentialsStoreMode)
    return storage.delete()
}

/**
 * Write an auth.json that contains only the API key.
 */
@OptIn(ExperimentalTime::class)
fun loginWithApiKey(
    codexHome: Path,
    apiKey: String,
    authCredentialsStoreMode: AuthCredentialsStoreMode
): Result<Unit> {
    val authDotJson = AuthDotJson(
        openaiApiKey = apiKey,
        tokens = null,
        lastRefresh = null
    )
    return saveAuth(codexHome, authDotJson, authCredentialsStoreMode)
}

/**
 * Persist the provided auth payload using the specified backend.
 */
fun saveAuth(
    codexHome: Path,
    auth: AuthDotJson,
    authCredentialsStoreMode: AuthCredentialsStoreMode
): Result<Unit> {
    val storage = createAuthStorage(codexHome, authCredentialsStoreMode)
    return storage.save(auth)
}

/**
 * Load CLI auth data. Returns null when no credentials are stored.
 */
fun loadAuthDotJson(
    codexHome: Path,
    authCredentialsStoreMode: AuthCredentialsStoreMode
): Result<AuthDotJson?> {
    val storage = createAuthStorage(codexHome, authCredentialsStoreMode)
    return storage.load()
}

/**
 * Enforce login restrictions from config.
 */
suspend fun enforceLoginRestrictions(config: Config): Result<Unit> {
    val auth = loadAuth(
        config.codexHome,
        enableCodexApiKeyEnv = true,
        config.cliAuthCredentialsStoreMode
    ).getOrElse { return Result.failure(it) }

    if (auth == null) {
        return Result.success(Unit)
    }

    // Check forced login method
    val requiredMethod = config.forcedLoginMethod
    if (requiredMethod != null) {
        val violation = when (requiredMethod) {
            ForcedLoginMethod.Api -> when (auth.mode) {
                AuthMode.ApiKey -> null
                AuthMode.ChatGPT -> "API key login is required, but ChatGPT is currently being used. Logging out."
            }
            ForcedLoginMethod.Chatgpt -> when (auth.mode) {
                AuthMode.ChatGPT -> null
                AuthMode.ApiKey -> "ChatGPT login is required, but an API key is currently being used. Logging out."
            }
        }

        if (violation != null) {
            return logoutWithMessage(
                config.codexHome,
                violation,
                config.cliAuthCredentialsStoreMode
            )
        }
    }

    // Check forced workspace ID (ChatGPT only)
    val expectedAccountId = config.forcedChatgptWorkspaceId
    if (expectedAccountId != null && auth.mode == AuthMode.ChatGPT) {
        val tokenData = auth.getTokenData().getOrElse { err ->
            return logoutWithMessage(
                config.codexHome,
                "Failed to load ChatGPT credentials while enforcing workspace restrictions: ${err.message}. Logging out.",
                config.cliAuthCredentialsStoreMode
            )
        }

        val chatgptAccountId = tokenData.idToken.chatgptAccountId
        if (chatgptAccountId != expectedAccountId) {
            val message = if (chatgptAccountId != null) {
                "Login is restricted to workspace $expectedAccountId, but current credentials belong to $chatgptAccountId. Logging out."
            } else {
                "Login is restricted to workspace $expectedAccountId, but current credentials lack a workspace identifier. Logging out."
            }
            return logoutWithMessage(
                config.codexHome,
                message,
                config.cliAuthCredentialsStoreMode
            )
        }
    }

    return Result.success(Unit)
}

// ============================================================================
// Internal Functions
// ============================================================================

private fun logoutWithMessage(
    codexHome: Path,
    message: String,
    authCredentialsStoreMode: AuthCredentialsStoreMode
): Result<Unit> {
    return when (logout(codexHome, authCredentialsStoreMode).isSuccess) {
        true -> Result.failure(Exception(message))
        false -> Result.failure(
            Exception("$message. Failed to remove auth.json")
        )
    }
}

/**
 * Load auth from storage, with optional environment variable fallback.
 */
internal fun loadAuth(
    codexHome: Path,
    enableCodexApiKeyEnv: Boolean,
    authCredentialsStoreMode: AuthCredentialsStoreMode
): Result<CodexAuth?> {
    // Check environment variable first if enabled
    if (enableCodexApiKeyEnv) {
        readCodexApiKeyFromEnv()?.let { apiKey ->
            val client = HttpClient()
            return Result.success(CodexAuth.fromApiKey(apiKey, client))
        }
    }

    val storage = createAuthStorage(codexHome, authCredentialsStoreMode)
    val client = HttpClient()

    val authDotJson = storage.load().getOrElse {
        return Result.failure(it)
    } ?: return Result.success(null)

    // Prefer API key if set in auth.json
    if (authDotJson.openaiApiKey != null) {
        return Result.success(CodexAuth.fromApiKey(authDotJson.openaiApiKey, client))
    }

    // Use ChatGPT tokens
    return Result.success(
        CodexAuth(
            mode = AuthMode.ChatGPT,
            apiKey = null,
            authDotJsonMutex = Mutex(),
            cachedAuthDotJson = authDotJson,
            storage = storage,
            client = client
        )
    )
}

/**
 * Update tokens in storage and return the updated auth.json.
 */
@OptIn(ExperimentalTime::class)
private fun updateTokens(
    storage: AuthStorageBackend,
    idToken: String?,
    accessToken: String?,
    refreshToken: String?
): Result<AuthDotJson> {
    val authDotJson = storage.load().getOrElse {
        return Result.failure(it)
    } ?: return Result.failure(Exception("Token data is not available."))

    val tokens = authDotJson.tokens?.copy() ?: TokenData()

    if (idToken != null) {
        val parsed = parseIdToken(idToken).getOrElse {
            return Result.failure(it)
        }
        tokens.idToken = parsed
    }
    if (accessToken != null) {
        tokens.accessToken = accessToken
    }
    if (refreshToken != null) {
        tokens.refreshToken = refreshToken
    }

    val updated = authDotJson.copy(
        tokens = tokens,
        lastRefresh = systemClock.markNow().elapsedNow().inWholeMilliseconds
    )

    storage.save(updated).getOrElse { return Result.failure(it) }
    return Result.success(updated)
}

/**
 * Attempt to refresh a token via the OAuth endpoint.
 */
private suspend fun tryRefreshToken(
    refreshToken: String,
    client: HttpClient
): Result<RefreshResponse> {
    val request = RefreshRequest(
        clientId = CLIENT_ID,
        grantType = "refresh_token",
        refreshToken = refreshToken,
        scope = "openid profile email"
    )

    val endpoint = getEnvironmentVariable(REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR)
        ?: REFRESH_TOKEN_URL

    return try {
        val response = client.post(endpoint) {
            contentType(ContentType.Application.Json)
            setBody(Json.encodeToString(RefreshRequest.serializer(), request))
        }

        if (response.status.isSuccess()) {
            val refreshResponse = Json.decodeFromString<RefreshResponse>(
                response.bodyAsText()
            )
            Result.success(refreshResponse)
        } else {
            val body = response.bodyAsText()
            if (response.status == HttpStatusCode.Unauthorized) {
                val failed = classifyRefreshTokenFailure(body)
                Result.failure(RefreshTokenError.Permanent(failed.reason, failed.message))
            } else {
                val message = tryParseErrorMessage(body)
                Result.failure(
                    RefreshTokenError.Transient("Failed to refresh token: ${response.status}: $message")
                )
            }
        }
    } catch (e: Exception) {
        Result.failure(RefreshTokenError.Transient(e.message ?: "Unknown error"))
    }
}

/**
 * Classify a 401 refresh token failure.
 */
private fun classifyRefreshTokenFailure(body: String): RefreshTokenFailedError {
    val code = extractRefreshTokenErrorCode(body)

    val normalizedCode = code?.lowercase()
    val reason = when (normalizedCode) {
        "refresh_token_expired" -> RefreshTokenFailedReason.Expired
        "refresh_token_reused" -> RefreshTokenFailedReason.Exhausted
        "refresh_token_invalidated" -> RefreshTokenFailedReason.Revoked
        else -> RefreshTokenFailedReason.Other
    }

    if (reason == RefreshTokenFailedReason.Other && code != null) {
        println("Warning: Encountered unknown 401 response while refreshing token: $code")
    }

    val message = when (reason) {
        RefreshTokenFailedReason.Expired -> REFRESH_TOKEN_EXPIRED_MESSAGE
        RefreshTokenFailedReason.Exhausted -> REFRESH_TOKEN_REUSED_MESSAGE
        RefreshTokenFailedReason.Revoked -> REFRESH_TOKEN_INVALIDATED_MESSAGE
        RefreshTokenFailedReason.Other -> REFRESH_TOKEN_UNKNOWN_MESSAGE
    }

    return RefreshTokenFailedError(reason, message)
}

/**
 * Extract error code from refresh token response body.
 */
private fun extractRefreshTokenErrorCode(body: String): String? {
    if (body.trim().isEmpty()) return null

    val json = try {
        Json.parseToJsonElement(body).jsonObject
    } catch (_: Exception) {
        // Return empty object on parse failure (matches Rust's unwrap_or_default)
        JsonObject(emptyMap())
    }

    // Try error.code first
    val errorCode = json["error"]?.jsonObject?.get("code")?.jsonPrimitive?.content
    if (errorCode != null) return errorCode

    // Try error as string
    val errorString = json["error"]?.jsonPrimitive?.content
    if (errorString != null) return errorString

    // Try top-level code
    return json["code"]?.jsonPrimitive?.content
}

/**
 * Try to parse an error message from JSON body.
 */
private fun tryParseErrorMessage(body: String): String {
    val json = try {
        Json.parseToJsonElement(body).jsonObject
    } catch (_: Exception) {
        // Return empty object on parse failure (matches Rust's unwrap_or_default)
        JsonObject(emptyMap())
    }

    // Try to get error message from various locations
    val errorMessage = json["error"]?.jsonPrimitive?.content
        ?: json["message"]?.jsonPrimitive?.content

    if (errorMessage != null) return errorMessage

    // If no structured error found and body is empty, return generic message
    if (body.isEmpty()) return "Unknown error"

    // Otherwise return the raw body
    return body
}

/**
 * Parse ID token JWT and extract claims.
 *
 * Uses the com.auth0.jwt library to decode the JWT without verification.
 * Extracts claims from the payload:
 * - email
 * - https://api.openai.com/auth.chatgpt_plan_type
 * - https://api.openai.com/auth.chatgpt_account_id
 *
 * Note: Signature verification is NOT performed.
 * This is only for extracting user info from trusted tokens.
 *
 * Reference: codex-rs/core/src/token_data.rs - parse_id_token()
 */
private fun parseIdToken(jwt: String): Result<IdTokenInfo> {
    return try {
        // Decode JWT without verification
        val decoded = com.auth0.jwt.JWT.decode(jwt)

        // Extract email
        val email = decoded.getClaim("email").asString()

        // Extract OpenAI auth claims
        val authClaim = decoded.getClaim("https://api.openai.com/auth")
        val authMap = authClaim.asMap()

        val planTypeStr = authMap?.get("chatgpt_plan_type") as? String
        val planType = planTypeStr?.let { PlanType.fromString(it) }

        val accountId = authMap?.get("chatgpt_account_id") as? String

        Result.success(
            IdTokenInfo(
                email = email,
                chatgptPlanType = planType,
                chatgptAccountId = accountId,
                rawJwt = jwt
            )
        )
    } catch (e: com.auth0.jwt.exceptions.JWTDecodeException) {
        Result.failure(Exception("Failed to decode JWT: ${e.message}", e))
    } catch (e: Exception) {
        Result.failure(Exception("JWT parsing failed: ${e.message}", e))
    }
}

/**
 * Create auth storage backend.
 */


@Serializable
private data class RefreshRequest(
    @SerialName("client_id")
    val clientId: String,
    @SerialName("grant_type")
    val grantType: String,
    @SerialName("refresh_token")
    val refreshToken: String,
    val scope: String
)

@Serializable
private data class RefreshResponse(
    @SerialName("id_token")
    val idToken: String? = null,
    @SerialName("access_token")
    val accessToken: String? = null,
    @SerialName("refresh_token")
    val refreshToken: String? = null
)

// ============================================================================
// AuthManager
// ============================================================================

/**
 * Central manager providing a single source of truth for auth.json derived
 * authentication data.
 *
 * It loads once (or on preference change) and then hands out cloned `CodexAuth`
 * values so the rest of the program has a consistent snapshot.
 *
 * External modifications to `auth.json` will NOT be observed until `reload()`
 * is called explicitly.
 *
 * Mirrors Rust's AuthManager from core/src/auth.rs
 */
class AuthManager private constructor(
    private val codexHome: Path,
    private val enableCodexApiKeyEnv: Boolean,
    private val authCredentialsStoreMode: AuthCredentialsStoreMode,
    initialAuth: CodexAuth?
) {
    private val mutex = Mutex()
    private var cachedAuth: CodexAuth? = initialAuth

    /**
     * Current cached auth (clone). May be null if not logged in or load failed.
     */
    suspend fun auth(): CodexAuth? {
        return mutex.withLock { cachedAuth }
    }

    /**
     * Force a reload of the auth information from auth.json.
     * Returns whether the auth value changed.
     */
    suspend fun reload(): Boolean {
        val newAuth = loadAuth(
            codexHome,
            enableCodexApiKeyEnv,
            authCredentialsStoreMode
        ).getOrNull()

        return mutex.withLock {
            val changed = !authsEqual(cachedAuth, newAuth)
            cachedAuth = newAuth
            changed
        }
    }

    /**
     * Attempt to refresh the current auth token (if any).
     *
     * On success, reloads the auth state from disk so other components
     * observe the refreshed token. If the token refresh fails in a permanent
     * (non-transient) way, logs out to clear invalid auth state.
     */
    suspend fun refreshToken(): Result<String?> {
        val currentAuth = auth() ?: return Result.success(null)

        return when {
            currentAuth.refreshToken().isSuccess -> {
                // Reload to pick up persisted changes
                reload()
                Result.success(currentAuth.refreshToken().getOrNull())
            }
            else -> {
                val error = currentAuth.refreshToken().exceptionOrNull()
                println("Error: Failed to refresh token: ${error?.message}")
                Result.failure(error ?: Exception("Unknown error"))
            }
        }
    }

    /**
     * Log out by deleting the on-disk auth.json (if present).
     *
     * Returns Ok(true) if a file was removed, Ok(false) if no auth file existed.
     * On success, reloads the in-memory auth cache so callers immediately
     * observe the unauthenticated state.
     */
    suspend fun logout(): Result<Boolean> {
        val result = logout(codexHome, authCredentialsStoreMode)
        // Always reload to clear any cached auth (even if file absent)
        reload()
        return result
    }

    private fun authsEqual(a: CodexAuth?, b: CodexAuth?): Boolean {
        return when {
            a == null && b == null -> true
            a != null && b != null -> a.mode == b.mode
            else -> false
        }
    }

    companion object {
        /**
         * Create a new manager loading the initial auth using the provided
         * preferred auth method. Errors loading auth are swallowed; `auth()` will
         * simply return `None` in that case so callers can treat it as an
         * unauthenticated state.
         */
        operator fun invoke(
            codexHome: Path,
            enableCodexApiKeyEnv: Boolean,
            authCredentialsStoreMode: AuthCredentialsStoreMode
        ): AuthManager {
            val auth = loadAuth(
                codexHome,
                enableCodexApiKeyEnv,
                authCredentialsStoreMode
            ).getOrNull()

            return AuthManager(codexHome, enableCodexApiKeyEnv, authCredentialsStoreMode, auth)
        }

        /**
         * Create an AuthManager with a specific CodexAuth, for testing only.
         */
        fun fromAuthForTesting(auth: CodexAuth): AuthManager {
            return AuthManager(
                codexHome = Path(""),
                enableCodexApiKeyEnv = false,
                authCredentialsStoreMode = AuthCredentialsStoreMode.File,
                initialAuth = auth
            )
        }
    }
}

// ============================================================================
// Platform-specific stubs
// ============================================================================

/**
 * Get environment variable value.
 *
 * Uses platform.posix.getenv for Native platforms (macOS/Linux/Windows).
 * For more complex multiplatform scenarios, consider creating expect/actual.
 */
@OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)
private fun getEnvironmentVariable(name: String): String? {
    return platform.posix.getenv(name)?.toKString()
}

// Placeholder for Config type
// TODO: Port from core/src/config.rs
data class Config(
    val codexHome: Path,
    val cliAuthCredentialsStoreMode: AuthCredentialsStoreMode,
    val forcedLoginMethod: ForcedLoginMethod?,
    val forcedChatgptWorkspaceId: String?
)

