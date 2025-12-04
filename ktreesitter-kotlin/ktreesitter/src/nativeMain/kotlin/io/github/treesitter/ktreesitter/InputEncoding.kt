package io.github.treesitter.ktreesitter

import io.github.treesitter.ktreesitter.internal.TSInputEncoding
import kotlinx.cinterop.ExperimentalForeignApi

/**
 * The encoding of the input text.
 *
 * @since 0.25.0
 */
@OptIn(ExperimentalForeignApi::class)
actual enum class InputEncoding(internal val value: TSInputEncoding) {
    UTF_8(TSInputEncoding.TSInputEncodingUTF8),
    UTF_16LE(TSInputEncoding.TSInputEncodingUTF16LE),
    UTF_16BE(TSInputEncoding.TSInputEncodingUTF16BE)
}
