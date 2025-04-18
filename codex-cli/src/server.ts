#!/usr/bin/env node
import "dotenv/config";
import http from "node:http";
import { WebSocketServer } from "ws";
import { loadConfig } from "./utils/config";
import { createInputItem } from "./utils/input-utils";
import { AgentLoop } from "./utils/agent/agent-loop";
import { ReviewDecision } from "./utils/agent/review";

// Ensure API key is set
const apiKey = process.env.OPENAI_API_KEY;
if (!apiKey) {
  console.error("Missing OpenAI API key (set OPENAI_API_KEY)");
  process.exit(1);
}

(async () => {
  // Load configuration
  let config = loadConfig(undefined, undefined, { cwd: process.cwd() });
  config = { apiKey, ...config };

  // History of items
  const items: Array<any> = [];

  // WebSocket server for streaming updates
  const wss = new WebSocketServer({ noServer: true });

  // Broadcast helper
  function broadcast(message: any) {
    const data = JSON.stringify(message);
    wss.clients.forEach((ws) => {
      if (ws.readyState === ws.OPEN) {
        ws.send(data);
      }
    });
  }

  // Initialize AgentLoop with full-auto approval
  let agent = new AgentLoop({
    model: config.model,
    config,
    instructions: config.instructions,
    approvalPolicy: "full-auto",
    additionalWritableRoots: [],
    onItem: (item) => {
      items.push(item);
      broadcast({ type: "item", item });
    },
    onLoading: (loading) => broadcast({ type: "loading", loading }),
    getCommandConfirmation: async (_cmd, patch) => ({ review: ReviewDecision.YES, applyPatch: patch }),
    onLastResponseId: (id) => broadcast({ type: "lastResponseId", id }),
  });

  // HTTP server
  const server = http.createServer(async (req, res) => {
    const { method = "", url = "" } = req;
    const reqUrl = new URL(url, `http://${req.headers.host}`);
    // WebSocket upgrade path
    if (reqUrl.pathname === "/ws") {
      res.writeHead(426);
      res.end("Upgrade Required");
      return;
    }
    // POST /prompt to submit a new prompt
    if (method === "POST" && reqUrl.pathname === "/prompt") {
      let body = "";
      req.on("data", (chunk) => { body += chunk; });
      req.on("end", async () => {
        try {
          const { prompt } = JSON.parse(body);
          if (typeof prompt !== "string" || prompt.trim() === "") {
            throw new Error("Invalid prompt");
          }
          const inputItem = await createInputItem(prompt, []);
          // Run agent on new prompt
          agent.run([inputItem]);
          res.writeHead(200, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ success: true }));
        } catch (err: any) {
          res.writeHead(400, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ error: err.message }));
        }
      });
      return;
    }
    // GET /state to retrieve current history and status
    if (method === "GET" && reqUrl.pathname === "/state") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify({
          items,
          cwd: process.cwd(),
          model: config.model,
        }),
      );
      return;
    }
    // Fallback
    res.writeHead(404, { "Content-Type": "text/plain" });
    res.end("Not Found");
  });

  // Handle WebSocket upgrades
  server.on("upgrade", (req, socket, head) => {
    const { url = "" } = req;
    const reqUrl = new URL(url, `http://${req.headers.host}`);
    if (reqUrl.pathname === "/ws") {
      wss.handleUpgrade(req, socket, head, (ws) => {
        wss.emit("connection", ws, req);
      });
    } else {
      socket.destroy();
    }
  });

  // Start listening
  const port = process.env.PORT || 3000;
  server.listen(port, () => {
    console.log(`Codex server listening on http://localhost:${port}`);
    console.log(`WebSocket endpoint ws://localhost:${port}/ws`);
  });
})();