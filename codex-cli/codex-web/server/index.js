import {
  appendFileSync,
  readFileSync,
  writeFileSync,
  existsSync,
  mkdirSync,
} from "node:fs";
import { createServer } from "node:http";
import { homedir } from "node:os";
import { parse as parseUrl } from "node:url";

// Minimal re-implementation of what we need from codex-cli.

// Built-in provider list (extend as needed).
const builtinProviders = {
  openai: {
    name: "OpenAI",
    baseURL: "https://api.openai.com/v1",
    envKey: "OPENAI_API_KEY",
  },
};

// Helpers to read & persist ~/.codex/config.json (same path codex-cli uses).
function configPath() {
  return `${homedir()}/.codex/config.json`;
}

function loadConfig() {
  try {
    return JSON.parse(readFileSync(configPath(), "utf8"));
  } catch {
    return { providers: {} };
  }
}

function saveConfig(cfg) {
  const p = configPath();
  const dir = p.split("/").slice(0, -1).join("/");
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }
  writeFileSync(p, JSON.stringify(cfg, null, 2), "utf8");
}

function setApiKey(_key) {
  // noop – real key persisted via .codex.env already
}

async function getAvailableModels(pid) {
  const provider = { ...builtinProviders, ...loadConfig().providers }[pid];
  if (!provider) {
    return [];
  }

  const key = process.env[provider.envKey];
  if (!key) {
    return [];
  }

  try {
    const response = await fetch(`${provider.baseURL}/models`, {
      headers: { Authorization: `Bearer ${key}` },
    });
    if (!response.ok) {
      return [];
    }
    const json = await response.json();
    return (json.data || []).map((m) => m.id);
  } catch {
    return [];
  }
}

function maxTokensForModel(id) {
  // very rough – better logic could be added.
  if (/32k/i.test(id)) {
    return 32000;
  }
  if (/16k/i.test(id)) {
    return 16000;
  }
  if (/13k/i.test(id)) {
    return 13000;
  }
  if (/8k|gpt-4/i.test(id)) {
    return 8192;
  }
  return 4096;
}

// Helper to send a JSON response with CORS headers.
function sendJson(res, status, data) {
  res.writeHead(status, {
    "Content-Type": "application/json",
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Headers": "Content-Type",
    "Access-Control-Allow-Methods": "GET,POST,PATCH,OPTIONS",
  });
  res.end(JSON.stringify(data));
}

async function parseBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on("data", (c) => chunks.push(c));
    req.on("end", () => {
      if (!chunks.length) {
        return resolve({});
      }
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString()));
      } catch (e) {
        reject(e);
      }
    });
    req.on("error", reject);
  });
}

// Basic route table → each entry: [method, regex, handler]
const routes = [];
function add(method, pathPattern, handler) {
  routes.push([method, new RegExp(`^${pathPattern}$`), handler]);
}

// -------------------------------------------
// Routes
// -------------------------------------------

add("GET", "/providers", (req, res) => {
  const cfg = loadConfig();
  sendJson(res, 200, { providers: { ...builtinProviders, ...cfg.providers } });
});

add("POST", "/providers", async (req, res) => {
  const body = await parseBody(req);
  const { id, name, baseURL, envKey } = body;
  if (!id || !name || !baseURL || !envKey) {
    return sendJson(res, 400, { error: "id, name, baseURL, envKey required" });
  }
  const cfg = loadConfig();
  cfg.providers = { ...cfg.providers, [id]: { name, baseURL, envKey } };
  saveConfig(cfg);
  sendJson(res, 201, { ok: true });
});

add("POST", "/providers/([^/]+)/key", async (req, res, [, pid]) => {
  const body = await parseBody(req);
  const { key } = body;
  if (!key) {
    return sendJson(res, 400, { error: "key required" });
  }

  setApiKey(key);
  const envVar = `${pid.toUpperCase()}_API_KEY`;
  const envPath = `${homedir()}/.codex.env`;
  try {
    const txt = readFileSync(envPath, "utf8");
    if (!txt.includes(envVar)) {
      appendFileSync(envPath, `\n${envVar}=${key}\n`);
    }
  } catch {
    appendFileSync(envPath, `\n${envVar}=${key}\n`);
  }
  sendJson(res, 200, { ok: true });
});

add("GET", "/providers/([^/]+)/models", async (req, res, [, pid]) => {
  try {
    let fetched = [];
    try {
      fetched = await getAvailableModels(pid);
    } catch {
      // ignore network errors when fetching models – continue with manual list
    }
    const cfg = loadConfig();
    const manual = cfg.providers?.[pid]?.manualModels ?? [];
    const merged = [...new Set([...fetched, ...manual.map((m) => m.id)])].map(
      (id) => {
        const m = manual.find((x) => x.id === id);
        return { id, ctx: m?.ctx ?? maxTokensForModel(id) };
      },
    );
    sendJson(res, 200, { models: merged });
  } catch (e) {
    sendJson(res, 500, { error: String(e) });
  }
});

add("POST", "/providers/([^/]+)/models", async (req, res, [, pid]) => {
  const body = await parseBody(req);
  const { modelId, ctx } = body;
  if (!modelId) {
    return sendJson(res, 400, { error: "modelId required" });
  }

  const cfg = loadConfig();
  const prov = cfg.providers?.[pid];
  if (!prov) {
    return sendJson(res, 404, { error: "unknown provider" });
  }
  prov.manualModels = prov.manualModels ?? [];
  if (!prov.manualModels.find((m) => m.id === modelId)) {
    prov.manualModels.push({ id: modelId, ctx: Number(ctx) || 0 });
    saveConfig(cfg);
  }
  sendJson(res, 201, { ok: true });
});

add("PATCH", "/config", async (req, res) => {
  const body = await parseBody(req);
  const { provider, model } = body;
  const cfg = loadConfig();
  if (provider) {
    cfg.provider = provider;
  }
  if (model) {
    cfg.model = model;
  }
  saveConfig(cfg);
  sendJson(res, 200, { ok: true });
});

// OPTIONS (CORS pre-flight)
add("OPTIONS", ".*", (req, res) => {
  res.writeHead(204, {
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Headers": "Content-Type",
    "Access-Control-Allow-Methods": "GET,POST,PATCH,OPTIONS",
  });
  res.end();
});

// -------------------------------------------
// Server bootstrap
// -------------------------------------------

const PORT = Number(process.env.PORT) || 8787;

createServer(async (req, res) => {
  const url = parseUrl(req.url || "").pathname || "/";
  const route = routes.find(
    ([method, regex]) => method === req.method && regex.test(url),
  );

  if (!route) {
    return sendJson(res, 404, { error: "Not found" });
  }

  const match = url.match(route[1]);
  try {
    await route[2](req, res, match);
  } catch (e) {
    sendJson(res, 500, { error: String(e) });
  }
}).listen(PORT, () => {
  // eslint-disable-next-line no-console
  console.log(`Codex gateway running on http://localhost:${PORT}`);
});
