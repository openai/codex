@file:kotlin.jvm.JvmName("KTreeSitter")

package io.github.treesitter.ktreesitter

// NOTE: don't forget to bump these when necessary

/**
 * The latest ABI version that is supported by the current version of the library.
 *
 * The Tree-sitter library is generally backwards-compatible with languages
 * generated using older CLI versions, but is not forwards-compatible.
 */
const val LANGUAGE_VERSION: UInt = 14U

/** The earliest ABI version that is supported by the current version of the library. */
const val MIN_COMPATIBLE_LANGUAGE_VERSION: UInt = 13U
