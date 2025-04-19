#!/usr/bin/env node
import "dotenv/config";
import http from "node:http";
import { WebSocketServer } from "ws";
import { promises as fsp } from "fs";
import fs from "fs";
import path from "path";
import chokidar from "chokidar";
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

  // Dynamically load all tool definitions from src/tools directory
  const toolsDir = path.join(__dirname, 'tools');
  const availableTools: any[] = [];
  if (fsSync.existsSync(toolsDir)) {
    for (const file of fsSync.readdirSync(toolsDir)) {
      if (file.endsWith('.js')) {
        try {
          const mod = require(path.join(toolsDir, file));
          const defs = mod.tools || mod.default;
          if (Array.isArray(defs)) availableTools.push(...defs);
        } catch (e) {
          console.error('Failed to load tool:', file, e);
        }
      }
    }
  }

  // Broadcast helper
  function broadcast(message: any) {
    const data = JSON.stringify(message);
    wss.clients.forEach((ws) => {
      if (ws.readyState === ws.OPEN) {
        ws.send(data);
      }
    });
  }

  // Watch filesystem for directory changes, emit incremental fs_event messages via chokidar
  const watchDir = process.cwd();
  try {
    const watcher = chokidar.watch(watchDir, { ignoreInitial: true, persistent: true });
    // File added
    watcher.on('add', (fullPath) => {
      const relPath = path.relative(watchDir, fullPath);
      broadcast({ type: 'fs_event', event: 'add', path: relPath, nodeType: 'file' });
    });
    // Directory added
    watcher.on('addDir', (fullPath) => {
      const relPath = path.relative(watchDir, fullPath);
      broadcast({ type: 'fs_event', event: 'add', path: relPath, nodeType: 'folder' });
    });
    // File removed
    watcher.on('unlink', (fullPath) => {
      const relPath = path.relative(watchDir, fullPath);
      broadcast({ type: 'fs_event', event: 'unlink', path: relPath });
    });
    // Directory removed
    watcher.on('unlinkDir', (fullPath) => {
      const relPath = path.relative(watchDir, fullPath);
      broadcast({ type: 'fs_event', event: 'unlink', path: relPath });
    });
    watcher.on('error', (error) => {
      console.error('Filesystem watch error:', error);
    });
  } catch (e) {
    console.error('Failed to watch directory for changes', e);
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
    // Route to voice-management handlers
    try {
      const vm = await import('./voice-management');
      if (await vm.handleVoiceRequest(req, res)) return;
    } catch {}
    // Route to knowledge-management handlers
    try {
      const km = await import('./knowledge-management');
      if (await km.handleKnowledgeRequest(req, res)) return;
    } catch {}
    const { method = "", url = "" } = req;
    const reqUrl = new URL(url, `http://${req.headers.host}`);
    // GET /tools to list available function tools
    if (method === "GET" && reqUrl.pathname === "/tools") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ tools: availableTools }));
      return;
    }
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
      // Build directory tree of current working directory
      let tree: Array<any> = [];
      const cwd = process.cwd();
      async function buildTree(dir: string): Promise<any[]> {
        const entries = await fsp.readdir(dir, { withFileTypes: true });
        const nodes = await Promise.all(entries.map(async (entry) => {
          const fullPath = path.join(dir, entry.name);
          const relPath = path.relative(cwd, fullPath) || entry.name;
          if (entry.isDirectory()) {
            const children = await buildTree(fullPath);
            return { id: relPath, label: entry.name, type: 'folder', defaultExpanded: false, children };
          } else {
            return { id: relPath, label: entry.name, type: 'file' };
          }
        }));
        return nodes;
      }
      try {
        tree = await buildTree(cwd);
      } catch (e) {
        console.error('Failed to build directory tree', e);
      }
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify({
          items,
          cwd,
          model: config.model,
          tree,
        }),
      );
      return;
    }
    // Fallback
    res.writeHead(404, { "Content-Type": "text/plain" });
    res.end("Not Found");
  });

  // Handle WebSocket upgrades for Codex streaming (support both /ws and /codex-ws)
  server.on("upgrade", (req, socket, head) => {
    const { url = "" } = req;
    const reqUrl = new URL(url, `http://${req.headers.host}`);
    if (reqUrl.pathname === "/ws" || reqUrl.pathname === "/codex-ws") {
      // Upgrade to WebSocket
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
    console.log(`WebSocket endpoints: ws://localhost:${port}/ws and ws://localhost:${port}/codex-ws`);
  });
})();