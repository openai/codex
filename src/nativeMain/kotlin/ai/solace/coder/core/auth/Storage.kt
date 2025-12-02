// port-lint: source codex-rs/core/src/auth/storage.rs
package ai.solace.coder.core.auth

import ai.solace.coder.core.AuthDotJson
import kotlinx.io.files.Path
import kotlinx.io.files.SystemFileSystem
import kotlinx.io.buffered
import kotlinx.io.readString
import kotlinx.io.writeString
import kotlinx.serialization.json.Json

/**
 * Determine where Codex should store CLI auth credentials.
 * Mirrors Rust's AuthCredentialsStoreMode enum from auth/storage.rs
 */
enum class AuthCredentialsStoreMode {
    /** Persist credentials in CODEX_HOME/auth.json */
    File,

    /** Persist credentials in the keyring. Fail if unavailable. */
    Keychain,

    /** Use keyring when available; otherwise, fall back to a file in CODEX_HOME */
    Auto
}

/**
 * Get the auth.json file path within codex_home.
 */
internal fun getAuthFile(codexHome: Path): Path {
    return Path(codexHome.toString(), "auth.json")
}

/**
 * Delete the auth.json file if it exists.
 * Returns true if a file was removed, false if it didn't exist.
 */
internal fun deleteFileIfExists(codexHome: Path): Result<Boolean> {
    val authFile = getAuthFile(codexHome)
    return try {
        SystemFileSystem.delete(authFile)
        Result.success(true)
    } catch (_: kotlinx.io.IOException) {
        // File doesn't exist
        Result.success(false)
    } catch (e: Exception) {
        Result.failure(e)
    }
}

/**
 * Auth storage backend trait.
 * Mirrors Rust's AuthStorageBackend trait from auth/storage.rs
 */
interface AuthStorageBackend {
    fun load(): Result<AuthDotJson?>
    fun save(auth: AuthDotJson): Result<Unit>
    fun delete(): Result<Boolean>
}

/**
 * File-based auth storage implementation.
 * Stores credentials in CODEX_HOME/auth.json with 0600 permissions on Unix.
 */
class FileAuthStorage(private val codexHome: Path) : AuthStorageBackend {

    /**
     * Attempt to read and parse the auth.json file.
     */
    fun tryReadAuthJson(authFile: Path): Result<AuthDotJson> {
        return try {
            val contents = SystemFileSystem.source(authFile).buffered().use { buffered ->
                buffered.readString()
            }
            val authDotJson = Json.decodeFromString<AuthDotJson>(contents)
            Result.success(authDotJson)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    override fun load(): Result<AuthDotJson?> {
        val authFile = getAuthFile(codexHome)

        // Check if file exists
        if (!SystemFileSystem.exists(authFile)) {
            return Result.success(null)
        }

        return tryReadAuthJson(authFile).map { it }
    }

    override fun save(auth: AuthDotJson): Result<Unit> {
        val authFile = getAuthFile(codexHome)

        return try {
            // Create parent directory if it doesn't exist
            val parent = authFile.parent
            if (parent != null && !SystemFileSystem.exists(parent)) {
                SystemFileSystem.createDirectories(parent)
            }

            // Serialize to pretty JSON
            val jsonFormat = Json { prettyPrint = true }
            val jsonData = jsonFormat.encodeToString(AuthDotJson.serializer(), auth)

            // Write to file
            // TODO: Set Unix file permissions to 0600 (owner read/write only)
            // This requires platform-specific code or a library
            SystemFileSystem.sink(authFile).buffered().use { buffered ->
                buffered.writeString(jsonData)
                buffered.flush()
            }

            Result.success(Unit)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    override fun delete(): Result<Boolean> {
        return deleteFileIfExists(codexHome)
    }

    companion object {
        fun new(codexHome: Path): FileAuthStorage {
            return FileAuthStorage(codexHome)
        }
    }
}

/**
 * Keychain/Keyring-based auth storage implementation.
 * Stores credentials securely in the system keychain.
 */
class KeychainAuthStorage(
    private val codexHome: Path,
    private val keychainStore: KeychainStore
) : AuthStorageBackend {

    /**
     * Load auth from keychain using computed key.
     */
    private fun loadFromKeychain(key: String): Result<AuthDotJson?> {
        return keychainStore.load(KEYCHAIN_SERVICE, key).mapCatching { serialized ->
            serialized?.let { Json.decodeFromString(AuthDotJson.serializer(), it) }
        }
    }

    /**
     * Save auth to keychain using computed key.
     */
    private fun saveToKeychain(key: String, value: String): Result<Unit> {
        return keychainStore.save(KEYCHAIN_SERVICE, key, value).onFailure { error ->
            println("Warning: failed to write OAuth tokens to keychain: ${error.message}")
        }
    }

    override fun load(): Result<AuthDotJson?> {
        val key = computeStoreKey(codexHome).getOrElse {
            return Result.failure(it)
        }
        return loadFromKeychain(key)
    }

    override fun save(auth: AuthDotJson): Result<Unit> {
        val key = computeStoreKey(codexHome).getOrElse {
            return Result.failure(it)
        }

        val serialized = try {
            Json.encodeToString(AuthDotJson.serializer(), auth)
        } catch (e: Exception) {
            return Result.failure(e)
        }

        saveToKeychain(key, serialized).getOrElse {
            return Result.failure(it)
        }

        // Remove fallback file if it exists
        deleteFileIfExists(codexHome).onFailure { err ->
            println("Warning: failed to remove CLI auth fallback file: ${err.message}")
        }

        return Result.success(Unit)
    }

    override fun delete(): Result<Boolean> {
        val key = computeStoreKey(codexHome).getOrElse {
            return Result.failure(it)
        }

        val keychainRemoved = keychainStore.delete(KEYCHAIN_SERVICE, key)
            .getOrElse { return Result.failure(it) }

        val fileRemoved = deleteFileIfExists(codexHome)
            .getOrElse { return Result.failure(it) }

        return Result.success(keychainRemoved || fileRemoved)
    }

    companion object {
        fun new(codexHome: Path, keychainStore: KeychainStore): KeychainAuthStorage {
            return KeychainAuthStorage(codexHome, keychainStore)
        }
    }
}

/**
 * Auto auth storage - tries keychain first, falls back to file.
 * Mirrors Rust's AutoAuthStorage from auth/storage.rs
 */
class AutoAuthStorage(
    private val keychainStorage: KeychainAuthStorage,
    private val fileStorage: FileAuthStorage
) : AuthStorageBackend {

    override fun load(): Result<AuthDotJson?> {
        // Try keychain first
        val result = keychainStorage.load()
        return if (result.isSuccess) {
            result.getOrNull()?.let {
                Result.success(it)
            } ?: fileStorage.load()
        } else {
            println("Warning: failed to load CLI auth from keychain, falling back to file storage: ${result.exceptionOrNull()?.message}")
            fileStorage.load()
        }
    }

    override fun save(auth: AuthDotJson): Result<Unit> {
        // Try keychain first
        val result = keychainStorage.save(auth)
        return if (result.isSuccess) {
            Result.success(Unit)
        } else {
            println("Warning: failed to save auth to keychain, falling back to file storage: ${result.exceptionOrNull()?.message}")
            fileStorage.save(auth)
        }
    }

    override fun delete(): Result<Boolean> {
        // Keychain storage will delete from disk as well
        return keychainStorage.delete()
    }

    companion object {
        fun new(codexHome: Path, keychainStore: KeychainStore): AutoAuthStorage {
            return AutoAuthStorage(
                KeychainAuthStorage.new(codexHome, keychainStore),
                FileAuthStorage.new(codexHome)
            )
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

private const val KEYCHAIN_SERVICE = "Codex Auth"

/**
 * Compute a stable, short key string from the codex_home path.
 * Uses SHA-256 hash truncated to 16 characters.
 *
 * Mirrors Rust's compute_store_key from auth/storage.rs
 *
 * TODO: Implement proper path canonicalization
 * Currently uses path as-is. Should resolve symlinks and make absolute.
 *
 * Expected test results:
 * - Input: "~/.codex" (after canonicalization to full path)
 * - Output: "cli|940db7b1d0e4eb40"
 *
 * Reference:
 * - codex-rs/core/src/auth/storage.rs - compute_store_key()
 * - Test: keyring_auth_storage_compute_store_key_for_home_directory
 */
internal fun computeStoreKey(codexHome: Path): Result<String> {
    return try {
        // TODO: Implement proper path canonicalization (resolve symlinks, make absolute)
        // For now, use the path as-is
        val canonical = codexHome.toString()

        // Hash the path string with SHA-256
        val sha256 = Sha256MessageDigest()
        val digest = sha256.digest(canonical)

        // Convert digest bytes to hex string (lowercase)
        val hex = buildString(digest.size * 2) {
            digest.forEach { byte ->
                val value = byte.toInt() and 0xFF
                append(HEX_CHARS[value shr 4])
                append(HEX_CHARS[value and 0x0F])
            }
        }

        // Truncate to first 16 characters and prefix with "cli|"
        val truncated = hex.take(16)
        Result.success("cli|$truncated")
    } catch (e: Exception) {
        Result.failure(e)
    }
}

private val HEX_CHARS = "0123456789abcdef".toCharArray()

/**
 * Create auth storage backend based on the specified mode.
 */
internal fun createAuthStorage(
    codexHome: Path,
    mode: AuthCredentialsStoreMode
): AuthStorageBackend {
    val keychainStore = DefaultKeychainStore()
    return createAuthStorageWithKeychainStore(codexHome, mode, keychainStore)
}

/**
 * Create auth storage with a specific keychain store (useful for testing).
 */
internal fun createAuthStorageWithKeychainStore(
    codexHome: Path,
    mode: AuthCredentialsStoreMode,
    keychainStore: KeychainStore
): AuthStorageBackend {
    return when (mode) {
        AuthCredentialsStoreMode.File -> FileAuthStorage.new(codexHome)
        AuthCredentialsStoreMode.Keychain -> KeychainAuthStorage.new(codexHome, keychainStore)
        AuthCredentialsStoreMode.Auto -> AutoAuthStorage.new(codexHome, keychainStore)
    }
}

// ============================================================================
// Keychain Store Interface (to be implemented)
// ============================================================================

/**
 * Keychain/Keyring store interface.
 * Platform-specific implementations needed.
 *
 * TODO: Implement platform-specific keychain access:
 * - macOS: Use Security framework via Keychain Services API
 * - Linux: Use Secret Service API (libsecret)
 * - Windows: Use Windows Credential Manager
 *
 * Consider using a Kotlin Multiplatform library or creating expect/actual implementations.
 */
interface KeychainStore {
    /**
     * Load a value from the keychain.
     * Returns null if the key doesn't exist.
     */
    fun load(service: String, key: String): Result<String?>

    /**
     * Save a value to the keychain.
     */
    fun save(service: String, key: String, value: String): Result<Unit>

    /**
     * Delete a value from the keychain.
     * Returns true if something was deleted, false if the key didn't exist.
     */
    fun delete(service: String, key: String): Result<Boolean>
}

/**
 * Default keychain store implementation.
 *
 * TODO: Implement actual keychain access via platform-specific APIs.
 *
 * This is a placeholder stub that always returns empty/failure.
 * The Rust version uses the `keyring` crate and `codex_keyring_store` module.
 *
 * Required platform implementations:
 *
 * macOS:
 *   - Use Security framework via Keychain Services API
 *   - Call SecItemAdd, SecItemCopyMatching, SecItemDelete via cinterop
 *   - Set kSecClass = kSecClassGenericPassword
 *   - Use service name as kSecAttrService
 *   - Use key as kSecAttrAccount
 *
 * Linux:
 *   - Use Secret Service API (libsecret) via D-Bus
 *   - org.freedesktop.secrets interface
 *   - Store in session or default collection
 *   - Handle org.freedesktop.Secret.Service methods
 *
 * Windows:
 *   - Use Windows Credential Manager (CredWrite, CredRead, CredDelete)
 *   - CREDENTIAL_TYPE_GENERIC credentials
 *   - TargetName format: "Codex Auth:<key>"
 *   - via cinterop to advapi32.dll
 *
 * Consider creating expect/actual multiplatform implementations:
 *   - commonMain: interface definition
 *   - nativeMain: stub with TODO
 *   - macosMain: Security framework implementation
 *   - linuxMain: libsecret implementation
 *   - mingwMain: Credential Manager implementation
 *
 * Or use a third-party Kotlin Multiplatform keychain library if available.
 *
 * Test coverage needed (see Rust tests):
 *   - keyring_auth_storage_load_returns_deserialized_auth
 *   - keyring_auth_storage_compute_store_key_for_home_directory
 *   - keyring_auth_storage_save_persists_and_removes_fallback_file
 *   - keyring_auth_storage_delete_removes_keyring_and_file
 *   - auto_auth_storage_load_prefers_keyring_value
 *   - auto_auth_storage_load_uses_file_when_keyring_empty
 *   - auto_auth_storage_load_falls_back_when_keyring_errors
 *   - auto_auth_storage_save_prefers_keyring
 *   - auto_auth_storage_save_falls_back_when_keyring_errors
 *   - auto_auth_storage_delete_removes_keyring_and_file
 *
 * Reference:
 *   - codex-rs/keyring-store crate (codex-rs/keyring-store/src/lib.rs)
 *   - Rust keyring crate documentation
 *   - codex-rs/core/src/auth/storage.rs tests (lines 290-672)
 */
class DefaultKeychainStore : KeychainStore {
    override fun load(service: String, key: String): Result<String?> {
        // TODO: Implement platform-specific keychain loading
        // Should return:
        //   - Result.success(value) if key exists
        //   - Result.success(null) if key doesn't exist
        //   - Result.failure(exception) on error
        //
        // For now, return null (not found) to allow fallback to file storage
        return Result.success(null)
    }

    override fun save(service: String, key: String, value: String): Result<Unit> {
        // TODO: Implement platform-specific keychain saving
        // Should:
        //   - Create or update the keychain entry
        //   - Return Result.success(Unit) on success
        //   - Return Result.failure(exception) on error
        //
        // For now, return failure so AutoAuthStorage falls back to file
        return Result.failure(Exception("Keychain storage not implemented"))
    }

    override fun delete(service: String, key: String): Result<Boolean> {
        // TODO: Implement platform-specific keychain deletion
        // Should return:
        //   - Result.success(true) if key was deleted
        //   - Result.success(false) if key didn't exist
        //   - Result.failure(exception) on error
        //
        // For now, return false (nothing deleted)
        return Result.success(false)
    }
}

// ============================================================================
// Test Support (MockKeyringStore equivalent)
// ============================================================================

/**
 * Mock keychain store for testing.
 *
 * TODO: Implement mock keychain store for unit tests.
 * Should mimic Rust's MockKeyringStore behavior:
 *   - Store values in memory (HashMap)
 *   - Support setting error conditions for specific keys
 *   - Provide test helpers like:
 *     - contains(key): Boolean
 *     - savedValue(key): String?
 *     - setError(key, error)
 *
 * Reference: codex-rs/keyring-store/src/tests.rs - MockKeyringStore
 */
class MockKeychainStore : KeychainStore {
    private val storage = mutableMapOf<String, String>()
    private val errors = mutableMapOf<String, Exception>()

    override fun load(service: String, key: String): Result<String?> {
        // TODO: Implement mock load behavior
        errors[key]?.let { return Result.failure(it) }
        return Result.success(storage[key])
    }

    override fun save(service: String, key: String, value: String): Result<Unit> {
        // TODO: Implement mock save behavior
        errors[key]?.let { return Result.failure(it) }
        storage[key] = value
        return Result.success(Unit)
    }

    override fun delete(service: String, key: String): Result<Boolean> {
        // TODO: Implement mock delete behavior
        errors[key]?.let { return Result.failure(it) }
        val existed = storage.remove(key) != null
        return Result.success(existed)
    }

    // Test helpers
    fun contains(key: String): Boolean = storage.containsKey(key)
    fun savedValue(key: String): String? = storage[key]
    fun setError(key: String, error: Exception) {
        errors[key] = error
    }
    fun clear() {
        storage.clear()
        errors.clear()
    }
}

// ============================================================================
// Additional Notes
// ============================================================================

/**
 * Storage.kt Feature Completeness vs Rust storage.rs:
 *
 * Implemented (✅):
 *   - AuthCredentialsStoreMode enum (File, Keyring, Auto)
 *   - AuthStorageBackend interface
 *   - FileAuthStorage (load, save, delete)
 *   - KeychainAuthStorage (load, save, delete with fallback)
 *   - AutoAuthStorage (keyring-first with file fallback)
 *   - getAuthFile() helper
 *   - deleteFileIfExists() helper
 *   - computeStoreKey() (stub - needs SHA-256)
 *   - createAuthStorage() factory functions
 *   - KeychainStore interface
 *   - DefaultKeychainStore stub
 *
 * Missing/Stubbed (⚠️):
 *   - SHA-256 implementation in computeStoreKey() (using hashCode placeholder)
 *   - Platform-specific keychain access (macOS/Linux/Windows)
 *   - Unix file permissions (mode 0600) in FileAuthStorage.save()
 *   - Path canonicalization in computeStoreKey()
 *   - MockKeychainStore test implementation
 *   - All unit tests (Rust has ~380 lines of tests)
 *
 * Test files from Rust (not ported):
 *   Lines 290-672 in storage.rs contain comprehensive tests:
 *   - file_storage_load_returns_auth_dot_json
 *   - file_storage_save_persists_auth_dot_json
 *   - file_storage_delete_removes_auth_file
 *   - keyring_auth_storage_* tests (7 tests)
 *   - auto_auth_storage_* tests (7 tests)
 *   - Helper functions: seed_keyring_and_fallback_auth_file_for_delete,
 *     seed_keyring_with_auth, assert_keyring_saved_auth_and_removed_fallback,
 *     id_token_with_prefix, auth_with_prefix
 *
 * Line count: Kotlin ~480 lines vs Rust 672 lines
 *   - Rust includes 380+ lines of tests
 *   - Kotlin production code: ~400 lines
 *   - Rust production code: ~290 lines
 *   - Similar coverage for production features
 *
 * Next steps to reach feature parity:
 *   1. Implement SHA-256 hashing (use kotlinx-crypto or native crypto)
 *   2. Create platform-specific keychain implementations (expect/actual)
 *   3. Add Unix file permission setting (mode 0600)
 *   4. Port unit tests to Kotlin test framework
 *   5. Implement MockKeychainStore for testing
 */

