package com.auth0.jwt.algorithms

import com.auth0.jwt.exceptions.SignatureGenerationException
import com.auth0.jwt.exceptions.SignatureVerificationException
import com.auth0.jwt.interfaces.DecodedJWT

/**
 * The Algorithm class represents an algorithm to be used in the Signing or Verification process of a Token.
 *
 *
 * This class and its subclasses are thread-safe.
 */
abstract class Algorithm(
    /**
     * Getter for the name of this Algorithm, as defined in the JWT Standard. i.e. "HS256"
     *
     * @return the algorithm name.
     */
    val name: String,
    /**
     * Getter for the description of this Algorithm,
     * required when instantiating a Mac or Signature object. i.e. "HmacSHA256"
     *
     * @return the algorithm description.
     */
    val description: String
) {
    open val signingKeyId: String?
        /**
         * Getter for the Id of the Private Key used to sign the tokens.
         * This is usually specified as the `kid` claim in the Header.
         *
         * @return the Key Id that identifies the Signing Key or null if it's not specified.
         */
        get() = null

    override fun toString(): String {
        return description
    }

    /**
     * Verify the given token using this Algorithm instance.
     *
     * @param jwt the already decoded JWT that it's going to be verified.
     * @throws SignatureVerificationException if the Token's Signature is invalid,
     * meaning that it doesn't match the signatureBytes,
     * or if the Key is invalid.
     */
    @Throws(SignatureVerificationException::class)
    abstract fun verify(jwt: DecodedJWT?)

    /**
     * Sign the given content using this Algorithm instance.
     *
     * @param headerBytes  an array of bytes representing the base64 encoded header content
     * to be verified against the signature.
     * @param payloadBytes an array of bytes representing the base64 encoded payload content
     * to be verified against the signature.
     * @return the signature in a base64 encoded array of bytes
     * @throws SignatureGenerationException if the Key is invalid.
     */
    @Throws(SignatureGenerationException::class)
    open fun sign(headerBytes: ByteArray?, payloadBytes: ByteArray?): ByteArray {
        // default implementation; keep around until sign(byte[]) method is removed
        val hBytes = headerBytes ?: ByteArray(0)
        val pBytes = payloadBytes ?: ByteArray(0)
        val contentBytes = ByteArray(hBytes.size + 1 + pBytes.size)

        hBytes.copyInto(contentBytes, 0, 0, hBytes.size)
        contentBytes[hBytes.size] = '.'.code.toByte()
        pBytes.copyInto(contentBytes, hBytes.size + 1, 0, pBytes.size)

        return sign(contentBytes)
    }

    /**
     * Sign the given content using this Algorithm instance.
     * To get the correct JWT Signature, ensure the content is in the format {HEADER}.{PAYLOAD}
     *
     * @param contentBytes an array of bytes representing the base64 encoded content
     * to be verified against the signature.
     * @return the signature in a base64 encoded array of bytes
     * @throws SignatureGenerationException if the Key is invalid.
     */
    @Throws(SignatureGenerationException::class)
    abstract fun sign(contentBytes: ByteArray?): ByteArray

    companion object {
        /**
         * Creates a new Algorithm instance using HmacSHA256. Tokens specify this as "HS256".
         *
         * @param secret the secret bytes to use in the verify or signing instance.
         * Ensure the length of the secret is at least 256 bit long
         * @return a valid HMAC256 Algorithm.
         * @throws IllegalArgumentException if the provided Secret is null.
         */
        fun hmac256(secret: String): Algorithm {
            return HMACAlgorithm("HS256", "HmacSHA256", secret)
        }

        /**
         * Creates a new Algorithm instance using HmacSHA256. Tokens specify this as "HS256".
         *
         * @param secret the secret bytes to use in the verify or signing instance.
         * Ensure the length of the secret is at least 256 bit long
         * @return a valid HMAC256 Algorithm.
         * @throws IllegalArgumentException if the provided Secret is null.
         */
        fun hmac256(secret: ByteArray): Algorithm {
            return HMACAlgorithm("HS256", "HmacSHA256", secret)
        }

        /**
         * Creates a new Algorithm instance using HmacSHA384. Tokens specify this as "HS384".
         *
         * @param secret the secret bytes to use in the verify or signing instance.
         * Ensure the length of the secret is at least 384 bit long
         * @return a valid HMAC384 Algorithm.
         * @throws IllegalArgumentException if the provided Secret is null.
         */
        fun hmac384(secret: String): Algorithm {
            return HMACAlgorithm("HS384", "HmacSHA384", secret)
        }

        /**
         * Creates a new Algorithm instance using HmacSHA384. Tokens specify this as "HS384".
         *
         * @param secret the secret bytes to use in the verify or signing instance.
         * Ensure the length of the secret is at least 384 bit long
         * @return a valid HMAC384 Algorithm.
         * @throws IllegalArgumentException if the provided Secret is null.
         */
        fun hmac384(secret: ByteArray): Algorithm {
            return HMACAlgorithm("HS384", "HmacSHA384", secret)
        }

        /**
         * Creates a new Algorithm instance using HmacSHA512. Tokens specify this as "HS512".
         *
         * @param secret the secret bytes to use in the verify or signing instance.
         * Ensure the length of the secret is at least 512 bit long
         * @return a valid HMAC512 Algorithm.
         * @throws IllegalArgumentException if the provided Secret is null.
         */
        fun hmac512(secret: String): Algorithm {
            return HMACAlgorithm("HS512", "HmacSHA512", secret)
        }

        /**
         * Creates a new Algorithm instance using HmacSHA512. Tokens specify this as "HS512".
         *
         * @param secret the secret bytes to use in the verify or signing instance.
         * Ensure the length of the secret is at least 512 bit long
         * @return a valid HMAC512 Algorithm.
         * @throws IllegalArgumentException if the provided Secret is null.
         */
        fun hmac512(secret: ByteArray): Algorithm {
            return HMACAlgorithm("HS512", "HmacSHA512", secret)
        }

        fun none(): Algorithm {
            return NoneAlgorithm()
        }

        // RSA and ECDSA are temporarily removed/stubbed until platform-specific implementations are added.
        // If you need these, please implement them using platform-specific crypto APIs.
    }
}