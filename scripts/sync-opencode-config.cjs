#!/usr/bin/env node
/* eslint-disable no-console */

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const toml = require("@iarna/toml");

function expandHome(p) {
  if (!p.startsWith("~")) return p;
  return path.join(os.homedir(), p.slice(1));
}

function json(value) {
  return JSON.stringify(value);
}

function renderValueLines(value, indent) {
  if (Array.isArray(value)) {
    return [json(value)];
  }
  if (value && typeof value === "object") {
    const entries = Object.entries(value);
    if (entries.length === 0) return ["{}"];
    const lines = ["{"];
    for (const [index, [k, v]] of entries.entries()) {
      const comma = index !== entries.length - 1 ? "," : "";
      const vLines = renderValueLines(v, indent + 2);
      if (vLines.length === 1) {
        lines.push(" ".repeat(indent + 2) + `${json(k)}: ${vLines[0]}${comma}`);
        continue;
      }
      lines.push(" ".repeat(indent + 2) + `${json(k)}: ${vLines[0]}`);
      for (const l of vLines.slice(1)) lines.push(" ".repeat(indent + 2) + l);
      lines[lines.length - 1] = lines[lines.length - 1] + comma;
    }
    lines.push(" ".repeat(indent) + "}");
    return lines;
  }
  return [json(value)];
}

function loadToml(filePath) {
  const raw = fs.readFileSync(filePath, "utf8");
  const parsed = toml.parse(raw);
  if (!parsed || typeof parsed !== "object") throw new Error("unexpected TOML root");
  return parsed;
}

function codexMcpServersToOpencodeMcp(configToml) {
  const mcpServers = configToml.mcp_servers;
  if (mcpServers == null) return { mcp: {}, warnings: [] };
  if (typeof mcpServers !== "object") throw new Error("mcp_servers must be a table");

  const mcp = {};
  const warnings = [];

  for (const [name, cfg] of Object.entries(mcpServers)) {
    if (!cfg || typeof cfg !== "object") {
      throw new Error(`mcp_servers.${name} must be a table`);
    }
    const { entry, entryWarnings } = codexMcpServerToOpencode(name, cfg);
    mcp[name] = entry;
    for (const w of entryWarnings) warnings.push(`${name}: ${w}`);
  }
  return { mcp, warnings };
}

function codexMcpServerToOpencode(name, cfg) {
  const warnings = [];
  const warn = (msg) => warnings.push(`codex mcp_servers.${name}.${msg}`);

  const enabled = cfg.enabled ?? true;
  if (typeof enabled !== "boolean") {
    throw new Error(`mcp_servers.${name}.enabled must be a boolean`);
  }

  const enabledTools = cfg.enabled_tools;
  if (
    enabledTools != null &&
    !(Array.isArray(enabledTools) && enabledTools.every((x) => typeof x === "string"))
  ) {
    throw new Error(`mcp_servers.${name}.enabled_tools must be an array of strings`);
  }

  if (cfg.disabled_tools != null) {
    warn(
      "disabled_tools は opencode 側に等価な表現がないため未反映（必要なら手動で tools を調整）",
    );
  }
  if (cfg.startup_timeout_sec != null || cfg.startup_timeout_ms != null) {
    warn("startup timeout は opencode 側に等価な表現がないため未反映");
  }
  if (cfg.tool_timeout_sec != null) {
    warn("tool timeout は opencode 側に等価な表現がないため未反映");
  }

  if (cfg.command != null) {
    if (typeof cfg.command !== "string" || cfg.command.length === 0) {
      throw new Error(`mcp_servers.${name}.command must be a non-empty string`);
    }
    const args = cfg.args ?? [];
    if (!(Array.isArray(args) && args.every((x) => typeof x === "string"))) {
      throw new Error(`mcp_servers.${name}.args must be an array of strings`);
    }

    const environment = {};
    if (cfg.env != null) {
      if (
        typeof cfg.env !== "object" ||
        Object.entries(cfg.env).some(([k, v]) => typeof k !== "string" || typeof v !== "string")
      ) {
        throw new Error(`mcp_servers.${name}.env must be a table of string:string`);
      }
      Object.assign(environment, cfg.env);
    }

    const envVars = cfg.env_vars ?? [];
    if (!(Array.isArray(envVars) && envVars.every((x) => typeof x === "string"))) {
      throw new Error(`mcp_servers.${name}.env_vars must be an array of strings`);
    }
    for (const key of envVars) {
      if (!(key in environment)) environment[key] = `{env:${key}}`;
    }

    if (cfg.cwd != null) {
      warn("cwd は opencode 側に等価な表現がないため未反映");
    }

    const entry = {
      type: "local",
      enabled,
      command: [cfg.command, ...args],
    };
    if (Object.keys(environment).length > 0) entry.environment = environment;
    if (enabledTools != null) entry.tools = enabledTools;
    return { entry, entryWarnings: warnings };
  }

  const url = cfg.url;
  if (url == null) {
    throw new Error(`mcp_servers.${name}: expected either command or url`);
  }
  if (typeof url !== "string" || url.length === 0) {
    throw new Error(`mcp_servers.${name}.url must be a non-empty string`);
  }

  const headers = {};
  if (cfg.http_headers != null) {
    if (
      typeof cfg.http_headers !== "object" ||
      Object.entries(cfg.http_headers).some(([k, v]) => typeof k !== "string" || typeof v !== "string")
    ) {
      throw new Error(`mcp_servers.${name}.http_headers must be a table of string:string`);
    }
    Object.assign(headers, cfg.http_headers);
  }

  if (cfg.env_http_headers != null) {
    if (
      typeof cfg.env_http_headers !== "object" ||
      Object.entries(cfg.env_http_headers).some(([k, v]) => typeof k !== "string" || typeof v !== "string")
    ) {
      throw new Error(`mcp_servers.${name}.env_http_headers must be a table of string:string`);
    }
    for (const [headerName, envKey] of Object.entries(cfg.env_http_headers)) {
      headers[headerName] = `{env:${envKey}}`;
    }
  }

  if (cfg.bearer_token_env_var != null) {
    if (typeof cfg.bearer_token_env_var !== "string" || cfg.bearer_token_env_var.length === 0) {
      throw new Error(`mcp_servers.${name}.bearer_token_env_var must be a non-empty string`);
    }
    if (!("Authorization" in headers)) {
      headers.Authorization = `Bearer {env:${cfg.bearer_token_env_var}}`;
    }
  }

  if (cfg.bearer_token != null) {
    warn(
      "bearer_token は Codex 側でも非推奨/拒否されうるため、opencode 側にも反映しない（env var を使ってください）",
    );
  }

  const entry = {
    type: "remote",
    enabled,
    url,
  };
  if (Object.keys(headers).length > 0) entry.headers = headers;
  if (enabledTools != null) entry.tools = enabledTools;
  return { entry, entryWarnings: warnings };
}

function renderOpencodeConfig({ mcp, sourcePath, entryWarnings }) {
  const now = new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
  const topComments = [
    `Generated from ${sourcePath} at ${now}`,
    "NOTE: JSONC（コメント付き JSON）として生成します（.json にコメントが含まれます）。",
  ];

  const headerWarnings = [];
  if (Object.keys(mcp).length === 0) {
    headerWarnings.push("mcp_servers が見つからなかったため、mcp は空で生成");
  }
  for (const w of headerWarnings) topComments.push(`WARNING: ${w}`);

  const warningsByServer = new Map();
  for (const w of entryWarnings) {
    const idx = w.indexOf(": ");
    if (idx === -1) continue;
    const server = w.slice(0, idx);
    const msg = w.slice(idx + 2);
    const list = warningsByServer.get(server) ?? [];
    list.push(msg);
    warningsByServer.set(server, list);
  }

  const mcpLines = ["{"];
  const names = Object.keys(mcp).sort();
  for (const [index, name] of names.entries()) {
    const comma = index !== names.length - 1 ? "," : "";
    const comments = warningsByServer.get(name) ?? [];
    for (const c of comments) mcpLines.push(" ".repeat(4) + `// ${c}`);
    const entryLines = renderValueLines(mcp[name], 4);
    if (entryLines.length === 1) {
      mcpLines.push(" ".repeat(4) + `${json(name)}: ${entryLines[0]}${comma}`);
      continue;
    }
    mcpLines.push(" ".repeat(4) + `${json(name)}: ${entryLines[0]}`);
    for (const l of entryLines.slice(1)) mcpLines.push(" ".repeat(4) + l);
    mcpLines[mcpLines.length - 1] = mcpLines[mcpLines.length - 1] + comma;
  }
  mcpLines.push("  }");

  const lines = [];
  for (const c of topComments) lines.push(`// ${c}`);
  lines.push("{");
  lines.push(`  ${json("$schema")}: ${json("https://opencode.ai/config.json")},`);
  lines.push(`  ${json("mcp")}: ${mcpLines[0]}`);
  lines.push(...mcpLines.slice(1));
  lines.push("}");
  return lines.join("\n") + "\n";
}

function writeIfChanged(filePath, content, { force }) {
  if (fs.existsSync(filePath)) {
    const existing = fs.readFileSync(filePath, "utf8");
    if (existing === content) return false;
    if (!force) {
      throw new Error(`refusing to overwrite existing file without --force: ${filePath}`);
    }
  }
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, content, "utf8");
  return true;
}

function parseArgs(argv) {
  const args = {
    codexConfig: "~/.codex/config.toml",
    out: "~/.config/opencode/opencode.json",
    workspaceDir: "~/workspace",
    skipWorkspace: false,
    force: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === "--codex-config") args.codexConfig = argv[++i];
    else if (a === "--out") args.out = argv[++i];
    else if (a === "--workspace-dir") args.workspaceDir = argv[++i];
    else if (a === "--skip-workspace") args.skipWorkspace = true;
    else if (a === "--force") args.force = true;
    else if (a === "--help" || a === "-h") args.help = true;
    else throw new Error(`unknown arg: ${a}`);
  }
  return args;
}

function printHelp() {
  console.log(
    [
      "Usage: node scripts/sync-opencode-config.cjs [options]",
      "",
      "Options:",
      "  --codex-config <path>   入力: Codex config.toml（default: ~/.codex/config.toml）",
      "  --out <path>           出力: opencode config（default: ~/.config/opencode/opencode.json）",
      "  --workspace-dir <path> workspace ルート（default: ~/workspace）",
      "  --skip-workspace       ~/workspace/.codex/config.toml があっても project 側 opencode.json を生成しない",
      "  --force                既存の opencode 設定ファイルを上書きする",
      "  -h, --help             ヘルプ",
    ].join("\n"),
  );
}

function main(argv) {
  const args = parseArgs(argv);
  if (args.help) {
    printHelp();
    return 0;
  }

  const codexConfig = expandHome(args.codexConfig);
  if (!fs.existsSync(codexConfig)) {
    console.error(`ERROR: codex config not found: ${codexConfig}`);
    return 2;
  }

  const userToml = loadToml(codexConfig);
  const { mcp: userMcp, warnings: userWarnings } = codexMcpServersToOpencodeMcp(userToml);
  const userContent = renderOpencodeConfig({
    mcp: userMcp,
    sourcePath: codexConfig,
    entryWarnings: userWarnings,
  });

  const userOut = expandHome(args.out);
  try {
    const changed = writeIfChanged(userOut, userContent, { force: args.force });
    console.log(changed ? `ok: wrote ${userOut}` : `ok: up-to-date ${userOut}`);
  } catch (e) {
    console.error(`ERROR: ${e.message}`);
    return 3;
  }
  for (const w of userWarnings) console.error(`warning: ${w}`);

  if (args.skipWorkspace) return 0;

  const workspaceDir = expandHome(args.workspaceDir);
  const workspaceCodex = path.join(workspaceDir, ".codex", "config.toml");
  if (!fs.existsSync(workspaceCodex)) {
    console.error(`note: skip workspace (not found): ${workspaceCodex}`);
    return 0;
  }

  const workspaceToml = loadToml(workspaceCodex);
  const { mcp: workspaceMcp, warnings: workspaceWarnings } = codexMcpServersToOpencodeMcp(workspaceToml);
  const workspaceContent = renderOpencodeConfig({
    mcp: workspaceMcp,
    sourcePath: workspaceCodex,
    entryWarnings: workspaceWarnings,
  });

  const workspaceOut = path.join(workspaceDir, "opencode.json");
  try {
    const changed = writeIfChanged(workspaceOut, workspaceContent, { force: args.force });
    console.log(changed ? `ok: wrote ${workspaceOut}` : `ok: up-to-date ${workspaceOut}`);
  } catch (e) {
    console.error(`ERROR: ${e.message}`);
    return 3;
  }
  for (const w of workspaceWarnings) console.error(`warning: ${w}`);

  return 0;
}

process.exitCode = main(process.argv.slice(2));

