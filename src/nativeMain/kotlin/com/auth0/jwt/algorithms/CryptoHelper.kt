package com.auth0.jwt.algorithms

import okio.ByteString.Companion.toByteString
import okio.ByteString.Companion.encodeUtf8

/**
 * Class used to perform the signature hash calculations.
 *
 *
 * This class is thread-safe.
 */
internal class CryptoHelper {
    /**
     * Verify signature for JWT header and payload.
     *
     * @param algorithm      algorithm name.
     * @param secretBytes    algorithm secret.
     * @param header         JWT header.
     * @param payload        JWT payload.
     * @param signatureBytes JWT signature.
     * @return true if signature is valid.
     */
    fun verifySignatureFor(
        algorithm: String?,
        secretBytes: ByteArray,
        header: String,
        payload: String,
        signatureBytes: ByteArray
    ): Boolean {
        return verifySignatureFor(
            algorithm, secretBytes,
            header.encodeUtf8().toByteArray(), payload.encodeUtf8().toByteArray(), signatureBytes
        )
    }

    /**
     * Verify signature for JWT header and payload.
     *
     * @param algorithm      algorithm name.
     * @param secretBytes    algorithm secret.
     * @param headerBytes    JWT header.
     * @param payloadBytes   JWT payload.
     * @param signatureBytes JWT signature.
     * @return true if signature is valid.
     */
    fun verifySignatureFor(
        algorithm: String?,
        secretBytes: ByteArray,
        headerBytes: ByteArray,
        payloadBytes: ByteArray,
        signatureBytes: ByteArray
    ): Boolean {
        val calculatedSignature = createSignatureFor(algorithm, secretBytes, headerBytes, payloadBytes)
        return calculatedSignature.contentEquals(signatureBytes)
    }

    /**
     * Create signature for JWT header and payload.
     *
     * @param algorithm    algorithm name.
     * @param secretBytes  algorithm secret.
     * @param headerBytes  JWT header.
     * @param payloadBytes JWT payload.
     * @return the signature bytes.
     */
    fun createSignatureFor(
        algorithm: String?,
        secretBytes: ByteArray,
        headerBytes: ByteArray,
        payloadBytes: ByteArray
    ): ByteArray {
        val content = headerBytes + JWT_PART_SEPARATOR + payloadBytes
        return createSignatureFor(algorithm, secretBytes, content)
    }

    /**
     * Create signature.
     * To get the correct JWT Signature, ensure the content is in the format {HEADER}.{PAYLOAD}
     *
     * @param algorithm    algorithm name.
     * @param secretBytes  algorithm secret.
     * @param contentBytes the content to be signed.
     * @return the signature bytes.
     */
    fun createSignatureFor(algorithm: String?, secretBytes: ByteArray, contentBytes: ByteArray): ByteArray {
        val key = secretBytes.toByteString()
        val data = contentBytes.toByteString()
        
        return when (algorithm) {
            "HmacSHA256" -> key.hmacSha256(data).toByteArray()
            "HmacSHA384" -> throw UnsupportedOperationException("HmacSHA384 not supported by Okio directly yet (requires 3.2.0+ or manual impl)") // Okio 3.x supports sha256 and sha512. 
            "HmacSHA512" -> key.hmacSha512(data).toByteArray()
            else -> throw IllegalArgumentException("Unsupported algorithm: $algorithm")
        }
    }

    companion object {
        private const val JWT_PART_SEPARATOR = 46.toByte()
    }
}