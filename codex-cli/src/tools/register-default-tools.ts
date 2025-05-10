import { shellToolDefinition } from "./shell/definition.js";
import { handleShellTool } from "./shell/handler.js";
import { registerTool } from "./tool-registry.js";

/**
 * Registers the default built-in tools like "shell".
 */
export function registerDefaultTools(): void {
  registerTool({
    definition: shellToolDefinition,
    handler: handleShellTool,
    aliases: ["container.exec", "container_exec"],
  });
}
