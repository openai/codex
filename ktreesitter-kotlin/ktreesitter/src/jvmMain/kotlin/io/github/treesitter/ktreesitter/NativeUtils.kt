package io.github.treesitter.ktreesitter

import java.io.File.createTempFile

internal object NativeUtils {
    private const val LIB_NAME = "ktreesitter"

    @JvmStatic
    @Throws(UnsupportedOperationException::class)
    private fun libPath(): String? {
        val osName = System.getProperty("os.name")!!.lowercase()
        val archName = System.getProperty("os.arch")!!.lowercase()
        val ext: String
        val os: String
        val prefix: String
        when {
            "windows" in osName -> {
                ext = "dll"
                os = "windows"
                prefix = ""
            }
            "linux" in osName -> {
                ext = "so"
                os = "linux"
                prefix = "lib"
            }
            "mac" in osName -> {
                ext = "dylib"
                os = "macos"
                prefix = "lib"
            }
            else -> {
                throw UnsupportedOperationException("Unsupported operating system: $osName")
            }
        }
        val arch = when {
            "amd64" in archName || "x86_64" in archName -> "x64"
            "aarch64" in archName || "arm64" in archName -> "aarch64"
            else -> throw UnsupportedOperationException("Unsupported architecture: $archName")
        }
        val libUrl = javaClass.getResource("/lib/$os/$arch/$prefix$LIB_NAME.$ext") ?: return null
        return createTempFile("$prefix$LIB_NAME", ".$ext").apply {
            writeBytes(libUrl.openStream().use { it.readAllBytes() })
            deleteOnExit()
        }.path
    }

    @JvmStatic
    @Throws(UnsatisfiedLinkError::class)
    internal fun loadLibrary() {
        try {
            System.loadLibrary(LIB_NAME)
        } catch (ex: UnsatisfiedLinkError) {
            @Suppress("UnsafeDynamicallyLoadedCode")
            System.load(libPath() ?: throw ex)
        }
    }
}
