// port-lint: source codex-rs/core/src/token_data.rs
package ai.solace.coder.core.auth

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

/**
 * Parse a JWT ID token and extract claims.
 *
 * JWT format: header.payload.signature
 * We only parse the payload (middle section) to extract claims.
 * Signature verification is NOT performed - this is only for extracting user info.
 *
 * Mirrors Rust's parse_id_token from core/src/token_data.rs
 */
@OptIn(ExperimentalEncodingApi::class)
fun parseIdToken(idToken: String): Result<IdTokenInfo> {
    return try {
        // Split JWT into three parts: header.payload.signature
        val parts = idToken.split('.')
        if (parts.size != 3) {
            return Result.failure(Exception("Invalid JWT format: expected 3 parts, got ${parts.size}"))
        }

        val (header, payload, signature) = parts
        if (header.isEmpty() || payload.isEmpty() || signature.isEmpty()) {
            return Result.failure(Exception("Invalid JWT format: empty parts"))
        }

        // Decode the payload (Base64 URL-safe without padding)
        val payloadBytes = try {
            Base64.UrlSafe.decode(payload)
        } catch (e: Exception) {
            return Result.failure(Exception("Failed to decode JWT payload: ${e.message}", e))
        }

        // Parse JSON payload
        val payloadJson = payloadBytes.decodeToString()
        val claims = try {
            Json.decodeFromString<JwtClaims>(payloadJson)
        } catch (e: Exception) {
            return Result.failure(Exception("Failed to parse JWT claims: ${e.message}", e))
        }

        // Extract auth-specific claims
        val auth = claims.auth
        val idTokenInfo = IdTokenInfo(
            email = claims.email,
            chatgptPlanType = auth?.chatgptPlanType,
            chatgptAccountId = auth?.chatgptAccountId,
            rawJwt = idToken
        )

        Result.success(idTokenInfo)
    } catch (e: Exception) {
        Result.failure(Exception("JWT parsing failed: ${e.message}", e))
    }
}

/**
 * Top-level JWT claims structure.
 * Maps the standard JWT payload fields we care about.
 */
@Serializable
private data class JwtClaims(
    @SerialName("email")
    val email: String? = null,

    @SerialName("email_verified")
    val emailVerified: Boolean? = null,

    // OpenAI-specific auth claim namespace
    @SerialName("https://api.openai.com/auth")
    val auth: AuthClaims? = null
)

/**
 * OpenAI-specific authentication claims.
 * Nested under "https://api.openai.com/auth" in the JWT payload.
 */
@Serializable
private data class AuthClaims(
    @SerialName("chatgpt_plan_type")
    val chatgptPlanType: PlanType? = null,

    @SerialName("chatgpt_account_id")
    val chatgptAccountId: String? = null,

    @SerialName("chatgpt_user_id")
    val chatgptUserId: String? = null,

    @SerialName("user_id")
    val userId: String? = null
)

/**
 * Parse a plan type string into a structured PlanType.
 * Known plan types are mapped to KnownPlan enum values.
 * Unknown plan types are preserved as strings for forward compatibility.
 */
@Serializable(with = PlanTypeSerializer::class)
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

/**
 * Known ChatGPT plan types.
 */
enum class KnownPlan {
    Free,
    Plus,
    Pro,
    Team,
    Business,
    Enterprise,
    Edu
}

/**
 * Custom serializer for PlanType to handle string -> sealed class conversion.
 */
private object PlanTypeSerializer : kotlinx.serialization.KSerializer<PlanType> {
    override val descriptor = kotlinx.serialization.descriptors.PrimitiveSerialDescriptor(
        "PlanType",
        kotlinx.serialization.descriptors.PrimitiveKind.STRING
    )

    override fun serialize(encoder: kotlinx.serialization.encoding.Encoder, value: PlanType) {
        val stringValue = when (value) {
            is PlanType.Known -> value.plan.name.lowercase()
            is PlanType.Unknown -> value.value
        }
        encoder.encodeString(stringValue)
    }

    override fun deserialize(decoder: kotlinx.serialization.encoding.Decoder): PlanType {
        val string = decoder.decodeString()
        return PlanType.fromString(string)
    }
}

/**
 * Flat subset of useful claims from JWT ID token.
 *
 * Mirrors Rust's IdTokenInfo from core/src/token_data.rs
 */
@Serializable
data class IdTokenInfo(
    val email: String? = null,

    @Serializable(with = PlanTypeSerializer::class)
    val chatgptPlanType: PlanType? = null,

    val chatgptAccountId: String? = null,

    // Store the original JWT string for serialization
    val rawJwt: String = ""
) {
    /**
     * Get the plan type as a string.
     * Returns the lowercase plan name for known plans, or the raw value for unknown plans.
     */
    fun getChatgptPlanTypeString(): String? {
        return chatgptPlanType?.let { plan ->
            when (plan) {
                is PlanType.Known -> plan.plan.name.lowercase()
                is PlanType.Unknown -> plan.value
            }
        }
    }
}

