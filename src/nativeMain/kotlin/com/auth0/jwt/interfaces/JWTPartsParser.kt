package com.auth0.jwt.interfaces

import com.auth0.jwt.exceptions.JWTDecodeException

/**
 * The JWTPartsParser class defines which parts of the JWT should be converted
 * to its specific Object representation instance.
 */
interface JWTPartsParser {
    /**
     * Parses the given JSON into a [Payload] instance.
     *
     * @param json the content of the Payload in a JSON representation.
     * @return the Payload.
     * @throws JWTDecodeException if the json doesn't have a proper JSON format.
     */
    @Throws(JWTDecodeException::class)
    fun parsePayload(json: String?): Payload?

    /**
     * Parses the given JSON into a [Header] instance.
     *
     * @param json the content of the Header in a JSON representation.
     * @return the Header.
     * @throws JWTDecodeException if the json doesn't have a proper JSON format.
     */
    @Throws(JWTDecodeException::class)
    fun parseHeader(json: String?): Header?
}