@file:JvmName("RefCleaner")

package io.github.treesitter.ktreesitter

import android.os.Build.VERSION.SDK_INT
import android.os.Build.VERSION_CODES.TIRAMISU
import java.lang.ref.Cleaner

internal object RefCleaner {
    private val INSTANCE = if (SDK_INT < TIRAMISU) null else Cleaner.create()

    @JvmName("register")
    operator fun invoke(obj: Any, action: Runnable) {
        if (SDK_INT >= TIRAMISU) INSTANCE!!.register(obj, action)
    }
}
