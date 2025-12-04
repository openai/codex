package io.github.treesitter.ktreesitter

import java.nio.charset.Charset

/**
 * The encoding of the input text.
 *
 * @since 0.25.0
 */
actual enum class InputEncoding(val charset: Charset) {
    UTF_8(Charsets.UTF_8),
    UTF_16LE(Charsets.UTF_16LE),
    UTF_16BE(Charsets.UTF_16BE)
}
