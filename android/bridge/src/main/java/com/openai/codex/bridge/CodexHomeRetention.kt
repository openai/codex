package com.openai.codex.bridge

import java.io.File

object CodexHomeRetention {
    const val DEFAULT_RETAINED_SESSION_HOMES: Int = 10
    private const val ACTIVE_MARKER = ".codex-active-session"
    private const val STALE_ACTIVE_MARKER_MS = 6 * 60 * 60 * 1000L

    data class PruneResult(
        val deletedHomeNames: List<String>,
        val failedHomeNames: Map<String, String>,
    )

    fun markActive(codexHome: File) {
        codexHome.mkdirs()
        File(codexHome, ACTIVE_MARKER).writeText(System.currentTimeMillis().toString())
    }

    fun clearActive(codexHome: File) {
        File(codexHome, ACTIVE_MARKER).delete()
    }

    fun pruneSessionHomes(
        root: File,
        keepHomeNames: Set<String>,
        retainedSessionHomes: Int = DEFAULT_RETAINED_SESSION_HOMES,
        nowMillis: Long = System.currentTimeMillis(),
    ): PruneResult {
        val children = root.listFiles()
            ?.filter(File::isDirectory)
            ?.filter { it.name.isNotBlank() }
            .orEmpty()
        if (children.isEmpty()) {
            return PruneResult(deletedHomeNames = emptyList(), failedHomeNames = emptyMap())
        }

        val candidates = children.filterNot { home ->
            home.name in keepHomeNames || hasFreshActiveMarker(home, nowMillis)
        }
        val retainedCandidateNames = candidates
            .sortedWith(compareByDescending<File> { it.lastModified() }.thenBy(File::getName))
            .take(retainedSessionHomes.coerceAtLeast(0))
            .mapTo(mutableSetOf(), File::getName)

        val deletedHomeNames = mutableListOf<String>()
        val failedHomeNames = linkedMapOf<String, String>()
        candidates
            .filterNot { it.name in retainedCandidateNames }
            .forEach { home ->
                runCatching {
                    home.deleteRecursively()
                }.onSuccess { deleted ->
                    if (deleted) {
                        deletedHomeNames += home.name
                    } else {
                        failedHomeNames[home.name] = "deleteRecursively returned false"
                    }
                }.onFailure { err ->
                    failedHomeNames[home.name] = err.message ?: err::class.java.simpleName
                }
            }

        return PruneResult(
            deletedHomeNames = deletedHomeNames,
            failedHomeNames = failedHomeNames,
        )
    }

    private fun hasFreshActiveMarker(home: File, nowMillis: Long): Boolean {
        val marker = File(home, ACTIVE_MARKER)
        if (!marker.isFile) {
            return false
        }
        val markerTime = marker.readText()
            .trim()
            .toLongOrNull()
            ?: marker.lastModified()
        return nowMillis - markerTime < STALE_ACTIVE_MARKER_MS
    }
}
