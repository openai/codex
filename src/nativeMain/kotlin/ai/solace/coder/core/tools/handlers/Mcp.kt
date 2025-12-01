// port-lint: source core/src/tools/handlers/mcp.rs
package ai.solace.coder.core.tools.handlers

import ai.solace.coder.core.function_tool.FunctionCallError
import ai.solace.coder.core.tools.ToolHandler
import ai.solace.coder.core.tools.ToolInvocation
import ai.solace.coder.core.tools.ToolKind
import ai.solace.coder.core.tools.ToolOutput
import ai.solace.coder.core.tools.ToolPayload
import ai.solace.coder.protocol.CallToolResult
import ai.solace.coder.protocol.ResponseInputItem

class McpHandler : ToolHandler {
    override fun kind(): ToolKind {
        return ToolKind.Mcp
    }

    override suspend fun handle(invocation: ToolInvocation): ToolOutput {
        val payload = invocation.payload as? ToolPayload.Mcp ?: return ToolOutput.Mcp(
            ai.solace.coder.protocol.Result(
                value = null,
                error = "Invalid payload for McpHandler"
            )
        )

        val result = invocation.session.callMcpTool(
            payload.server,
            payload.tool,
            payload.rawArguments
        )

        return ToolOutput.Mcp(result)
    }
}
