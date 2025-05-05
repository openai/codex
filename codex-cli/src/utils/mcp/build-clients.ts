import { getMcpServers } from "../config";
import { MCPClient } from "./mcp-client";

export function buildClients(): Array<MCPClient> {
  const servers = getMcpServers();
  const clients = [];
  for (const server of Object.keys(servers)) {
    const client = new MCPClient(server, "1.0.0");
    clients.push(client);
  }
  return clients;
}
