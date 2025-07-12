import type { MCPStats } from "./types";
import type { mcpManager } from "../core/singleton";

import { MCPContext } from "./context";
import { useContext } from "react";

/**
 * Hook to access the MCP manager and status
 */
export function useMcpManager(): {
  manager: typeof mcpManager;
  stats: MCPStats;
} {
  const context = useContext(MCPContext);
  if (!context) {
    throw new Error("useMcpManager must be used within an MCPProvider");
  }
  return context;
}
