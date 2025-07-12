import type { MCPStats } from "./types";
import type { ReactNode } from "react";

import { log } from "../../logger/log";
import { mcpManager } from "../core/singleton";
import React, { createContext, useEffect, useState } from "react";

// Create context with initial empty stats
const MCPContext = createContext<{
  manager: typeof mcpManager;
  stats: MCPStats;
}>({
  manager: mcpManager,
  stats: {
    status: "idle",
    connectedServers: 0,
    totalServers: 0,
    erroredServers: 0,
    toolsCount: 0,
  },
});

interface MCPProviderProps {
  children: ReactNode;
}

/**
 * Provider component that manages MCP connections and makes them available through context
 */
export function MCPProvider({ children }: MCPProviderProps): JSX.Element {
  const [stats, setStats] = useState<MCPStats>({
    status: "idle",
    connectedServers: 0,
    totalServers: 0,
    erroredServers: 0,
    toolsCount: 0,
  });

  // Force a re-render to ensure updates are propagated
  const [_, forceUpdate] = React.useReducer((x) => x + 1, 0);

  useEffect(() => {
    // Set status to connecting
    log("[MCP] Setting initial status to connecting");
    setStats((prev) => ({ ...prev, status: "connecting" }));

    // Initialize MCP Manager
    const initializeMcp = async () => {
      try {
        log("[MCP] Starting initialization");
        await mcpManager.initialize();
        log("[MCP] Initialization complete");

        // Get stats after initialization
        const serversStatus = mcpManager.getServersStatus();
        log(`[MCP] Server status: ${JSON.stringify(serversStatus)}`);

        // Get tools
        log("[MCP] Getting flattened tools");
        const tools = await mcpManager.getFlattendTools();
        log(`[MCP] Found ${tools.length} tools`);

        const connectedServers = serversStatus.filter(
          (s) => s.connected,
        ).length;
        const totalServers = serversStatus.length;
        const erroredServers = totalServers - connectedServers;

        const newStatus =
          connectedServers > 0
            ? "connected"
            : erroredServers > 0
              ? "error"
              : "idle";
        log(
          `[MCP] Setting new status: ${newStatus}, connected servers: ${connectedServers}/${totalServers}, tools: ${tools.length}`,
        );

        // Update state with a completely new object to ensure React detects the change
        setStats({
          status: newStatus,
          connectedServers,
          totalServers,
          erroredServers,
          toolsCount: tools.length,
        });

        // Force a re-render to ensure the new stats are propagated
        forceUpdate();
        log("[MCP] State updated and re-render forced");
      } catch (error) {
        log(`[MCP] Error during initialization: ${error}`);
        setStats((prev) => ({
          ...prev,
          status: "error",
          erroredServers: prev.totalServers,
        }));
        forceUpdate();
      }
    };

    initializeMcp();

    return () => {
      // Clean up MCP connections on unmount
      log("[MCP] Cleaning up connections on unmount");
      mcpManager.disconnectAll();
    };
  }, []);

  return (
    <MCPContext.Provider value={{ manager: mcpManager, stats }}>
      {children}
    </MCPContext.Provider>
  );
}

// Export the context
export { MCPContext };
