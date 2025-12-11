#!/usr/bin/env node
"use strict";
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(
  // If the importer is in node compatibility mode or this is not an ESM
  // file that has been converted to a CommonJS file using a Babel-
  // compatible transform (i.e. "__esModule" has not been set), then set
  // "default" to the CommonJS "module.exports" for node compatibility.
  isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target,
  mod
));

// src/index.ts
var import_promises = __toESM(require("fs/promises"));
var import_node_http = __toESM(require("http"));
var import_node_os = __toESM(require("os"));
var import_node_path = __toESM(require("path"));
var import_node_child_process = require("child_process");
var import_promises2 = require("readline/promises");
var import_node_process = __toESM(require("process"));
var import_googleapis = require("googleapis");
var z = __toESM(require("zod"));
var DRIVE_SCOPES = ["https://www.googleapis.com/auth/drive.readonly"];
var TOKEN_DIR = import_node_os.default.homedir() && import_node_os.default.homedir().length > 0 ? import_node_path.default.join(import_node_os.default.homedir(), ".codex") : import_node_path.default.join(import_node_process.default.cwd(), ".codex");
var DRIVE_TOKEN_PATH = import_node_path.default.join(TOKEN_DIR, "google-drive-mcp-oauth.json");
var DRIVE_CLIENT_CONFIG_PATH = import_node_path.default.join(
  TOKEN_DIR,
  "google-drive-mcp-oauth-client.json"
);
function haveOAuthEnv() {
  return typeof import_node_process.default.env.GOOGLE_OAUTH_CLIENT_ID === "string" && import_node_process.default.env.GOOGLE_OAUTH_CLIENT_ID.length > 0 && typeof import_node_process.default.env.GOOGLE_OAUTH_CLIENT_SECRET === "string" && import_node_process.default.env.GOOGLE_OAUTH_CLIENT_SECRET.length > 0;
}
async function loadStoredTokens(tokenPath) {
  try {
    const data = await import_promises.default.readFile(tokenPath, "utf8");
    return JSON.parse(data);
  } catch {
    return null;
  }
}
async function getOAuthAuthorizedClient(scopes, tokenPath, clientConfigPath, appName) {
  let clientId = import_node_process.default.env.GOOGLE_OAUTH_CLIENT_ID;
  let clientSecret = import_node_process.default.env.GOOGLE_OAUTH_CLIENT_SECRET;
  if (!clientId || !clientSecret) {
    try {
      const raw = await import_promises.default.readFile(clientConfigPath, "utf8");
      const parsed = JSON.parse(raw);
      if (parsed.clientId && parsed.clientSecret) {
        clientId = parsed.clientId;
        clientSecret = parsed.clientSecret;
      }
    } catch {
    }
  }
  if (!clientId || !clientSecret) {
    throw new Error(
      `[${appName}] GOOGLE_OAUTH_CLIENT_ID and GOOGLE_OAUTH_CLIENT_SECRET must be set, or run --setup-auth to store them for reuse.`
    );
  }
  const tokens = await loadStoredTokens(tokenPath);
  if (!tokens) {
    throw new Error(
      `[${appName}] No OAuth tokens found. Run \`npx @openai/codex-google-drive-mcp --authorize\` to complete the login flow.`
    );
  }
  const oAuth2Client = new import_googleapis.google.auth.OAuth2(clientId, clientSecret);
  oAuth2Client.setCredentials(tokens);
  if (!oAuth2Client.credentials.scope) {
    oAuth2Client.credentials.scope = scopes.join(" ");
  }
  return oAuth2Client;
}
function tryOpenInBrowser(url) {
  const platform = import_node_process.default.platform;
  let command;
  let args;
  if (platform === "darwin") {
    command = "open";
    args = [url];
  } else if (platform === "win32") {
    command = "cmd";
    args = ["/c", "start", "", url];
  } else {
    command = "xdg-open";
    args = [url];
  }
  try {
    const child = (0, import_node_child_process.spawn)(command, args, { stdio: "ignore", detached: true });
    child.unref();
  } catch {
  }
}
async function runOAuthAuthorization(scopes, tokenPath, clientConfigPath, appName) {
  let clientId = import_node_process.default.env.GOOGLE_OAUTH_CLIENT_ID;
  let clientSecret = import_node_process.default.env.GOOGLE_OAUTH_CLIENT_SECRET;
  if (!clientId || !clientSecret) {
    try {
      const raw = await import_promises.default.readFile(clientConfigPath, "utf8");
      const parsed = JSON.parse(raw);
      if (parsed.clientId && parsed.clientSecret) {
        clientId = parsed.clientId;
        clientSecret = parsed.clientSecret;
      }
    } catch {
    }
  }
  if (!clientId || !clientSecret) {
    throw new Error(
      `[${appName}] GOOGLE_OAUTH_CLIENT_ID and GOOGLE_OAUTH_CLIENT_SECRET must be set, or run --setup-auth to store them before running --authorize.`
    );
  }
  const server = import_node_http.default.createServer();
  const port = await new Promise((resolve, reject) => {
    server.once("error", (err) => reject(err));
    server.listen(0, () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        reject(new Error("Failed to bind local HTTP server for OAuth flow."));
        return;
      }
      resolve(address.port);
    });
  });
  const redirectUri = `http://localhost:${port}/oauth2callback`;
  const oAuth2Client = new import_googleapis.google.auth.OAuth2(
    clientId,
    clientSecret,
    redirectUri
  );
  const authUrl = oAuth2Client.generateAuthUrl({
    access_type: "offline",
    scope: scopes,
    prompt: "consent"
  });
  console.error(
    `[${appName}] Open this URL in a browser to authorize access:

${authUrl}
`
  );
  tryOpenInBrowser(authUrl);
  const code = await new Promise((resolve, reject) => {
    server.on("request", (req, res) => {
      if (!req.url) {
        res.writeHead(400, { "Content-Type": "text/plain" });
        res.end("Missing request URL.");
        return;
      }
      const url = new URL(req.url, redirectUri);
      const receivedCode = url.searchParams.get("code");
      const error = url.searchParams.get("error");
      if (error) {
        res.writeHead(400, { "Content-Type": "text/plain" });
        res.end("Authorization error. You may close this window.");
        reject(new Error(`OAuth error: ${error}`));
        server.close();
        return;
      }
      if (!receivedCode) {
        res.writeHead(400, { "Content-Type": "text/plain" });
        res.end("Missing authorization code. You may close this window.");
        reject(new Error("Missing authorization code in OAuth callback."));
        server.close();
        return;
      }
      res.writeHead(200, { "Content-Type": "text/plain" });
      res.end("Authorization complete. You may close this window.");
      resolve(receivedCode);
      server.close();
    });
  });
  const { tokens } = await oAuth2Client.getToken(code);
  oAuth2Client.setCredentials(tokens);
  await import_promises.default.mkdir(import_node_path.default.dirname(tokenPath), { recursive: true });
  await import_promises.default.writeFile(tokenPath, JSON.stringify(tokens, null, 2), "utf8");
  console.error(
    `[${appName}] OAuth tokens saved to ${tokenPath}. You can now start the MCP server without --authorize.`
  );
}
async function hasStoredClientCredentials(configPath) {
  try {
    const raw = await import_promises.default.readFile(configPath, "utf8");
    const parsed = JSON.parse(raw);
    return Boolean(parsed.clientId && parsed.clientSecret);
  } catch {
    return false;
  }
}
async function setupAuthConfig(configPath, appName) {
  const rl = (0, import_promises2.createInterface)({
    input: import_node_process.default.stdin,
    output: import_node_process.default.stdout
  });
  try {
    const clientId = (await rl.question(
      `[${appName}] Enter Google OAuth Client ID: `
    )).trim();
    const clientSecret = (await rl.question(
      `[${appName}] Enter Google OAuth Client Secret: `
    )).trim();
    if (!clientId || !clientSecret) {
      throw new Error(`[${appName}] Both client ID and client secret are required.`);
    }
    await import_promises.default.mkdir(import_node_path.default.dirname(configPath), { recursive: true });
    await import_promises.default.writeFile(
      configPath,
      JSON.stringify({ clientId, clientSecret }, null, 2),
      "utf8"
    );
    console.error(
      `[${appName}] Saved OAuth client credentials to ${configPath}.`
    );
  } finally {
    rl.close();
  }
}
async function getDriveClient() {
  if (haveOAuthEnv() || await hasStoredClientCredentials(DRIVE_CLIENT_CONFIG_PATH)) {
    const authClient = await getOAuthAuthorizedClient(
      DRIVE_SCOPES,
      DRIVE_TOKEN_PATH,
      DRIVE_CLIENT_CONFIG_PATH,
      "codex-google-drive-mcp"
    );
    return import_googleapis.google.drive({ version: "v3", auth: authClient });
  }
  const auth = new import_googleapis.google.auth.GoogleAuth({
    scopes: DRIVE_SCOPES
  });
  return import_googleapis.google.drive({ version: "v3", auth });
}
function makeErrorResult(message) {
  return {
    content: [
      {
        type: "text",
        text: message
      }
    ],
    isError: true
  };
}
function createMcpServer(...args) {
  const mod = require("@modelcontextprotocol/sdk/server/mcp.js");
  const Ctor = mod.McpServer;
  return Reflect.construct(Ctor, args);
}
function createStdioServerTransport() {
  const mod = require("@modelcontextprotocol/sdk/server/stdio.js");
  const Ctor = mod.StdioServerTransport;
  return new Ctor();
}
async function runServer() {
  const server = createMcpServer(
    {
      name: "codex-google-drive-mcp",
      version: "0.0.0-dev"
    },
    {
      capabilities: {
        tools: {}
      },
      instructions: "Tools for listing files in Google Drive accessible to the configured Google account. Authentication can use Google Application Default Credentials or an interactive OAuth flow (see README)."
    }
  );
  server.registerTool(
    "list_files",
    {
      title: "List Google Drive files",
      description: "List recent files in Google Drive for the configured account.",
      inputSchema: {
        query: z.string().optional().describe("Optional substring to match against file names."),
        mimeType: z.string().optional().describe(
          "Optional MIME type filter, for example 'application/pdf' or 'application/vnd.google-apps.document'."
        ),
        pageSize: z.number().int().min(1).max(100).optional().describe("Maximum number of files to return (default 25).")
      }
    },
    async ({ query, mimeType, pageSize }) => {
      try {
        const drive = await getDriveClient();
        const filters = ["trashed = false"];
        if (mimeType && mimeType.trim().length > 0) {
          const escapedMime = mimeType.replace(/'/g, "\\'");
          filters.push(`mimeType = '${escapedMime}'`);
        }
        if (query && query.trim().length > 0) {
          const escapedQuery = query.replace(/'/g, "\\'");
          filters.push(`name contains '${escapedQuery}'`);
        }
        const response = await drive.files.list({
          q: filters.join(" and "),
          pageSize: pageSize ?? 25,
          fields: "files(id,name,mimeType,owners(displayName),modifiedTime,webViewLink)",
          orderBy: "modifiedTime desc"
        });
        const files = response.data.files ?? [];
        if (files.length === 0) {
          return {
            content: [
              {
                type: "text",
                text: "No matching Google Drive files found."
              }
            ]
          };
        }
        const lines = files.map((file) => {
          const id = file.id ?? "(no id)";
          const name = file.name ?? "(untitled)";
          const mime = file.mimeType ?? "unknown MIME type";
          const modified = file.modifiedTime ?? "unknown time";
          const owner = file.owners?.[0]?.displayName;
          const link = file.webViewLink;
          let line = `\u2022 ${name} (${id}) \u2014 ${mime}, modified ${modified}`;
          if (owner) {
            line += ` by ${owner}`;
          }
          if (link) {
            line += ` \u2014 ${link}`;
          }
          return line;
        });
        return {
          content: [
            {
              type: "text",
              text: lines.join("\n")
            }
          ],
          structuredContent: {
            files
          }
        };
      } catch (err) {
        const message = err instanceof Error ? `Failed to list Google Drive files: ${err.message}` : "Failed to list Google Drive files.";
        return makeErrorResult(message);
      }
    }
  );
  const transport = createStdioServerTransport();
  await server.connect(transport);
}
async function main() {
  const args = import_node_process.default.argv.slice(2);
  if (args.includes("--setup-auth")) {
    await setupAuthConfig(DRIVE_CLIENT_CONFIG_PATH, "codex-google-drive-mcp");
    await runOAuthAuthorization(
      DRIVE_SCOPES,
      DRIVE_TOKEN_PATH,
      DRIVE_CLIENT_CONFIG_PATH,
      "codex-google-drive-mcp"
    );
    return;
  }
  if (args.includes("--authorize")) {
    await runOAuthAuthorization(
      DRIVE_SCOPES,
      DRIVE_TOKEN_PATH,
      DRIVE_CLIENT_CONFIG_PATH,
      "codex-google-drive-mcp"
    );
    return;
  }
  await runServer();
}
if (typeof require !== "undefined" && typeof module !== "undefined" && require.main === module) {
  void main().catch((err) => {
    const message = err instanceof Error ? err.message : "Unexpected error starting server.";
    console.error(`codex-google-drive-mcp: ${message}`);
    import_node_process.default.exit(1);
  });
}
