package com.openai.codex.bridge

import java.io.File
import kotlin.io.path.createTempDirectory
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CodexHomeRetentionTest {
    @Test
    fun prunesOldHomesButKeepsExplicitAndActiveHomes() {
        val root = createTempDirectory("codex-home-retention").toFile()
        try {
            val oldHome = makeHome(root, "old", lastModified = 1_000)
            val newerHome = makeHome(root, "newer", lastModified = 2_000)
            val newestHome = makeHome(root, "newest", lastModified = 3_000)
            val explicitKeepHome = makeHome(root, "explicit", lastModified = 100)
            val activeHome = makeHome(root, "active", lastModified = 50)
            CodexHomeRetention.markActive(activeHome)

            val result = CodexHomeRetention.pruneSessionHomes(
                root = root,
                keepHomeNames = setOf("explicit"),
                retainedSessionHomes = 2,
            )

            assertEquals(
                CodexHomeRetention.PruneResult(
                    deletedHomeNames = listOf("old"),
                    failedHomeNames = emptyMap(),
                ),
                result,
            )
            assertFalse(oldHome.exists())
            assertTrue(newerHome.exists())
            assertTrue(newestHome.exists())
            assertTrue(explicitKeepHome.exists())
            assertTrue(activeHome.exists())
        } finally {
            root.deleteRecursively()
        }
    }

    @Test
    fun staleActiveMarkerDoesNotKeepHome() {
        val root = createTempDirectory("codex-home-retention").toFile()
        try {
            val staleActiveHome = makeHome(root, "stale-active", lastModified = 1_000)
            CodexHomeRetention.markActive(staleActiveHome)

            val result = CodexHomeRetention.pruneSessionHomes(
                root = root,
                keepHomeNames = emptySet(),
                retainedSessionHomes = 0,
                nowMillis = System.currentTimeMillis() + 7 * 60 * 60 * 1000L,
            )

            assertEquals(
                CodexHomeRetention.PruneResult(
                    deletedHomeNames = listOf("stale-active"),
                    failedHomeNames = emptyMap(),
                ),
                result,
            )
            assertFalse(staleActiveHome.exists())
        } finally {
            root.deleteRecursively()
        }
    }

    private fun makeHome(root: File, name: String, lastModified: Long): File {
        return File(root, name).apply {
            mkdirs()
            resolve("payload").writeText(name)
            setLastModified(lastModified)
        }
    }
}
