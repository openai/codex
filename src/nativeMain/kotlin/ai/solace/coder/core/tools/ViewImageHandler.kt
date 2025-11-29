package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import okio.FileSystem
import okio.Path.Companion.toPath

/**
 * Handler for the view_image tool.
 * Attaches a local image file to the conversation.
 *
 * Ported from Rust codex-rs/core/src/tools/handlers/view_image.rs
 */
class ViewImageHandler : ToolHandler {

    override val kind: ToolKind = ToolKind.Function

    override suspend fun handle(invocation: ToolInvocation): CodexResult<ToolOutput> {
        val payload = invocation.payload
        if (payload !is ToolPayload.Function) {
            return CodexResult.failure(
                CodexError.Fatal("view_image handler received unsupported payload")
            )
        }

        val args = try {
            json.decodeFromString<ViewImageArgs>(payload.arguments)
        } catch (e: Exception) {
            return CodexResult.failure(
                CodexError.Fatal("failed to parse function arguments: ${e.message}")
            )
        }

        val absPath = invocation.turn.resolvePath(args.path)
        val path = absPath.toPath()

        // Check if file exists
        if (!FileSystem.SYSTEM.exists(path)) {
            return CodexResult.failure(
                CodexError.Fatal("unable to locate image at `$absPath`: path does not exist")
            )
        }

        // Check if it's a file (not a directory)
        val metadata = FileSystem.SYSTEM.metadataOrNull(path)
        if (metadata == null || !metadata.isRegularFile) {
            return CodexResult.failure(
                CodexError.Fatal("image path `$absPath` is not a file")
            )
        }

        // Verify it's a supported image format
        val extension = path.name.substringAfterLast('.', "").lowercase()
        if (!SUPPORTED_EXTENSIONS.contains(extension)) {
            return CodexResult.failure(
                CodexError.Fatal("unsupported image format: .$extension")
            )
        }

        // Return success with image attachment info
        // The actual image injection into the session happens at a higher level
        // via the ToolOutput.ImageAttachment output type
        return CodexResult.success(
            ToolOutput.ImageAttachment(
                path = absPath,
                message = "attached local image path"
            )
        )
    }

    companion object {
        private val json = Json {
            ignoreUnknownKeys = true
            isLenient = true
        }

        private val SUPPORTED_EXTENSIONS = setOf(
            "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif"
        )
    }
}

/**
 * Arguments for the view_image tool.
 */
@Serializable
private data class ViewImageArgs(
    val path: String
)
