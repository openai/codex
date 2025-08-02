// Export the provider component
export { MCPProvider } from "./react/context";

// Export the hook
export { useMcpManager } from "./react/use-manager";

// Export types
export type { MCPStats, MCPStatus } from "./react/types";

// Export core functionality if needed externally
export { MCPManager } from "./core/manager";
export { MCPClient } from "./core/client";

// Re-export tool utilities if needed
export { mcpToOpenaiTools } from "./utils/to-openai";
export { flattenTools } from "./utils/flatten";
