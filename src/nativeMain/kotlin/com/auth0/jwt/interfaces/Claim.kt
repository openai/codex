@file:OptIn(kotlin.time.ExperimentalTime::class)

package com.auth0.jwt.interfaces

import kotlin.time.Instant

/**
 * The Claim class holds the value in a generic way so that it can be recovered in many representations.
 */
interface Claim {
    /**
     * Get this Claim as a Boolean.
     * If the value isn't of type Boolean or can't be converted to a Boolean, null will be returned.
     *
     * @return the value as a Boolean or null.
     */
    fun asBoolean(): Boolean?

    /**
     * Get this Claim as an Integer.
     * If the value isn't of type Integer or can't be converted to an Integer, null will be returned.
     *
     * @return the value as an Integer or null.
     */
    fun asInt(): Int?

    /**
     * Get this Claim as a Long.
     * If the value isn't of type Long or can't be converted to a Long, null will be returned.
     *
     * @return the value as a Long or null.
     */
    fun asLong(): Long?

    /**
     * Get this Claim as a Double.
     * If the value isn't of type Double or can't be converted to a Double, null will be returned.
     *
     * @return the value as a Double or null.
     */
    fun asDouble(): Double?

    /**
     * Get this Claim as a String.
     * If the value isn't of type String or can't be converted to a String, null will be returned.
     *
     * @return the value as a String or null.
     */
    fun asString(): String?

    /**
     * Get this Claim as a Date.
     * If the value can't be converted to a Date, null will be returned.
     *
     * @return the value as a Date or null.
     */
    fun asDate(): Instant?

    /**
     * Get this Claim as an Array of type T.
     * If the value isn't an Array, null will be returned.
     *
     * @param clazz the class of the items in the array
     * @return the value as an Array or null.
     * @throws JWTDecodeException if the custom JSON parser cannot decode the claim
     */
    // Reified inline function to replace Class<T>
    // fun <T> asArray(clazz: KClass<T>): Array<T>? 
    // For simplicity in initial port, we might stick to specific list/array getters or use reified inline functions if possible in interface (not possible directly).
    // Let's stick to List for now as it's more idiomatic in Kotlin.
    
    /**
     * Get this Claim as a List of type T.
     * If the value isn't an Array, null will be returned.
     *
     * @return the value as a List or null.
     */
    fun <T : Any> asList(clazz: kotlin.reflect.KClass<T>): List<T>?

    /**
     * Get this Claim as a Map of keys and values.
     * If the value isn't a Map, null will be returned.
     *
     * @return the value as a Map or null.
     */
    fun asMap(): Map<String, Any>?

    /**
     * Checks if this Claim is null.
     *
     * @return true if this Claim is null, false otherwise.
     */
    fun isNull(): Boolean
}