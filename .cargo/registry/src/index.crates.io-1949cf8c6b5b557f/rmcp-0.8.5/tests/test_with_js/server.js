import { McpServer, ResourceTemplate } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

const server = new McpServer({
  name: "Demo",
  version: "1.0.0"
});

server.resource(
  "greeting",
  new ResourceTemplate("greeting://{name}", { list: undefined }),
  async (uri, { name }) => ({
    contents: [{
      uri: uri.href,
      text: `Hello, ${name}`
    }]
  })
);

server.tool(
  "add",
  { a: z.number(), b: z.number() },
  async ({ a, b }) => ({
    "content": [
      {
        "type": "text",
        "text": `${a + b}`
      }
    ]
  })
);

const transport = new StdioServerTransport();
await server.connect(transport);