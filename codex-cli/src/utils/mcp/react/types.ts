import type { ReactNode } from "react";

// MCP Status types
export type MCPStatus = "idle" | "connecting" | "connected" | "error";
export type MCPStats = {
  status: MCPStatus;
  connectedServers: number;
  totalServers: number;
  erroredServers: number;
  toolsCount: number;
};

// Re-export the manager type
export type { MCPManager } from "../core/manager";

// Provider props interface
export interface MCPProviderProps {
  children: ReactNode;
}
