import io.github.kotlinmania.jwt.JWT
import io.github.kotlinmania.jwt.algorithms.Algorithm
import kotlin.time.Clock
import kotlin.time.Instant
import kotlin.time.Duration.Companion.hours
import kotlin.time.ExperimentalTime

@OptIn(ExperimentalTime::class)
fun main() {
    println("Starting JWT Kotlin Native Verification")

    try {
        // 1. Create Algorithm
        val secret = "secret"
        val algorithm = Algorithm.hmac256(secret)
        println("Algorithm created: ${algorithm.name}")

        // 2. Create Token
        val now = Clock.System.now()
        val expiresAt: Instant = now.plus(1.hours)
        
        val token: String = JWT.create()
            .withIssuer("auth0")
            .withSubject("user123")
            .withClaim("admin", true)
            .withExpiresAt(expiresAt)
            .sign(algorithm)
        
        println("Token created: $token")

        // 3. Decode Token
        val decodedJWT = JWT.decode(token)
        println("Token decoded:")
        println("  Header: ${decodedJWT.header}")
        println("  Payload: ${decodedJWT.payload}")
        println("  Issuer: ${decodedJWT.issuer}")
        println("  Subject: ${decodedJWT.subject}")
        println("  Admin Claim: ${decodedJWT.getClaim("admin").asBoolean()}")

        // 4. Verify Token
        val verifier = JWT.require(algorithm)
            .withIssuer("auth0")
            .build()
        
        val verifiedJWT = verifier.verify(token)
        println("Token verified successfully!")
        
    } catch (e: Exception) {
        println("Error during verification:")
        e.printStackTrace()
    }
}
