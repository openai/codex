package ai.solace.coder.core.auth

class Sha256MessageDigest {
    // The K constants
    private val K = intArrayOf(
        0x428a2f98, 0x71374491, -0x4a3f0431, -0x164a245b, 0x3956c25b, 0x59f111f1, -0x6dc07d5c, -0x54e3a12b,
        -0x27f85568, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, -0x7f214e02, -0x6423f959, -0x3e640e8c,
        -0x1b64963f, -0x1041b87a, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        -0x67c1aeae, -0x57ce3993, -0x4ffcd838, -0x40a68039, -0x391ff40d, -0x2a586eb9, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, -0x7e3d36d2, -0x6d8dd37b,
        -0x5d40175f, -0x57e599b5, -0x3db47490, -0x3893ae5d, -0x2e6d17e7, -0x2966f9dc, -0xbf1ca7b, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, -0x7b3787ec, -0x7338fdf8, -0x6f410006, -0x5baf9315, -0x41065c09, -0x398e870e
    )

    // Initial Hash values
    private val H0 = intArrayOf(
        0x6a09e667,
        -0x4498517b,
        0x3c6ef372,
        -0x5ab00ac6,
        0x510e527f,
        -0x64fa9774,
        0x1f83d9ab,
        0x5be0cd19
    )

    fun digest(input: String): ByteArray {
        val sourceBytes: ByteArray = input.encodeToByteArray()
        return generate(sourceBytes)
    }

    fun digest(sourceBytes: ByteArray): ByteArray {
        return generate(sourceBytes)
    }

    private fun generate(sourceBytes: ByteArray): ByteArray {
        val messageBlock = createMessageBlock(sourceBytes)
        val chunks = breakIntoChunks(messageBlock)
        val messageSchedules = createMessageSchedules(chunks)
        val expandedMessageSchedules = computeMessageSchedules(messageSchedules)
        return calculateHash(expandedMessageSchedules)
    }

    private fun createMessageBlock(sourceBytes: ByteArray): ByteArray {
        val paddingLength = calculatePaddingLength(sourceBytes.size)
        val paddedLength = sourceBytes.size + paddingLength + (64 / 8)

        val buffer = ByteArray(paddedLength)
        // Initial raw bytes of source data
        sourceBytes.copyInto(buffer, 0, 0, sourceBytes.size)
        
        // Append the data with a single '1' (0x80 = 10000000)
        buffer[sourceBytes.size] = 128.toByte()
        
        // Padding is already 0 initialized
        
        // Append the source data length as a 64-bit Integer (Big Endian)
        val lengthBits = sourceBytes.size * 8L
        putLong(buffer, buffer.size - 8, lengthBits)

        return buffer
    }

    private fun putLong(buffer: ByteArray, offset: Int, value: Long) {
        buffer[offset] = (value ushr 56).toByte()
        buffer[offset + 1] = (value ushr 48).toByte()
        buffer[offset + 2] = (value ushr 40).toByte()
        buffer[offset + 3] = (value ushr 32).toByte()
        buffer[offset + 4] = (value ushr 24).toByte()
        buffer[offset + 5] = (value ushr 16).toByte()
        buffer[offset + 6] = (value ushr 8).toByte()
        buffer[offset + 7] = value.toByte()
    }

    private fun putInt(buffer: ByteArray, offset: Int, value: Int) {
        buffer[offset] = (value ushr 24).toByte()
        buffer[offset + 1] = (value ushr 16).toByte()
        buffer[offset + 2] = (value ushr 8).toByte()
        buffer[offset + 3] = value.toByte()
    }

    private fun calculatePaddingLength(sourceLength: Int): Int {
        return (512 - (sourceLength * 8 + 64) % 512) / 8
    }

    private fun breakIntoChunks(messageBlock: ByteArray): Array<ByteArray> {
        val chunkSize = 512 / 8
        val numChunks = messageBlock.size / chunkSize
        val chunks = Array(numChunks) { ByteArray(chunkSize) }

        for (i in 0 until numChunks) {
            messageBlock.copyInto(chunks[i], 0, i * chunkSize, (i + 1) * chunkSize)
        }
        return chunks
    }

    private fun createMessageSchedules(chunks: Array<ByteArray>): Array<IntArray> {
        val messageSchedules = Array(chunks.size) { IntArray(64) }

        for (i in chunks.indices) {
            for (j in 0..15) {
                messageSchedules[i][j] =
                    ((chunks[i][j * 4].toInt() and 0xFF) shl 24) or
                            ((chunks[i][j * 4 + 1].toInt() and 0xFF) shl 16) or
                            ((chunks[i][j * 4 + 2].toInt() and 0xFF) shl 8) or
                            (chunks[i][j * 4 + 3].toInt() and 0xFF)
            }
        }
        return messageSchedules
    }

    private fun computeMessageSchedules(messageSchedules: Array<IntArray>): Array<IntArray> {
        for (messageSchedule in messageSchedules) {
            for (j in 16..63) {
                val w0 = messageSchedule[j - 16]
                val s0 = smallSigma0(messageSchedule[j - 15])
                val w1 = messageSchedule[j - 7]
                val s1 = smallSigma1(messageSchedule[j - 2])

                messageSchedule[j] = w0 + s0 + w1 + s1
            }
        }
        return messageSchedules
    }

    private fun rotateRight(x: Int, dist: Int): Int {
        return (x ushr dist) or (x shl (32 - dist))
    }

    private fun smallSigma0(x: Int): Int {
        return rotateRight(x, 7) xor
                rotateRight(x, 18) xor
                (x ushr 3)
    }

    private fun smallSigma1(x: Int): Int {
        return rotateRight(x, 17) xor
                rotateRight(x, 19) xor
                (x ushr 10)
    }

    private fun calculateHash(expandedMessageSchedules: Array<IntArray>): ByteArray {
        val H = H0.copyOf(H0.size)

        for (expandedMessageSchedule in expandedMessageSchedules) {
            var a = H[0]
            var b = H[1]
            var c = H[2]
            var d = H[3]
            var e = H[4]
            var f = H[5]
            var g = H[6]
            var h = H[7]

            for (i in 0..63) {
                val T1 = h + bigSigma1(e) + ch(e, f, g) + K[i] + expandedMessageSchedule[i]
                val T2 = bigSigma0(a) + maj(a, b, c)
                h = g
                g = f
                f = e
                e = d + T1
                d = c
                c = b
                b = a
                a = T1 + T2
            }

            H[0] += a
            H[1] += b
            H[2] += c
            H[3] += d
            H[4] += e
            H[5] += f
            H[6] += g
            H[7] += h
        }

        val result = ByteArray(256 / 8)
        for (i in H.indices) {
            putInt(result, i * 4, H[i])
        }

        return result
    }

    private fun bigSigma0(x: Int): Int {
        return rotateRight(x, 2) xor
                rotateRight(x, 13) xor
                rotateRight(x, 22)
    }

    private fun bigSigma1(x: Int): Int {
        return rotateRight(x, 6) xor
                rotateRight(x, 11) xor
                rotateRight(x, 25)
    }

    private fun ch(e: Int, f: Int, g: Int): Int {
        return (e and f) xor ((e.inv()) and g)
    }

    private fun maj(a: Int, b: Int, c: Int): Int {
        return (a and b) xor (a and c) xor (b and c)
    }
}