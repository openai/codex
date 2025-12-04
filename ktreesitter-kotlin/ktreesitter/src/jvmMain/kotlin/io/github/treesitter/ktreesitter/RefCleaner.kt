@file:JvmName("RefCleaner")

package io.github.treesitter.ktreesitter

import java.lang.ref.Cleaner

internal object RefCleaner {
    private val INSTANCE: Cleaner = Cleaner.create()

    @JvmName("register")
    operator fun invoke(obj: Any, action: Runnable) {
        INSTANCE.register(obj, action)
    }
}
