#!/usr/bin/env node
"use strict";
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
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
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

// src/index.ts
var src_exports = {};
__export(src_exports, {
  extractTextFromDocument: () => extractTextFromDocument,
  formatSheetValues: () => formatSheetValues
});
module.exports = __toCommonJS(src_exports);
var import_promises = __toESM(require("fs/promises"));
var import_node_http = __toESM(require("http"));
var import_node_os = __toESM(require("os"));
var import_node_path = __toESM(require("path"));
var import_node_child_process = require("child_process");
var import_promises2 = require("readline/promises");
var import_node_process = __toESM(require("process"));
var import_googleapis = require("googleapis");
var z = __toESM(require("zod"));
var PROFILE_SCOPES = {
  full: ["documents", "drive", "spreadsheets"],
  read: ["documents.readonly", "drive.readonly", "spreadsheets.readonly"]
};
var TOKEN_DIR = import_node_os.default.homedir() && import_node_os.default.homedir().length > 0 ? import_node_path.default.join(import_node_os.default.homedir(), ".codex") : import_node_path.default.join(import_node_process.default.cwd(), ".codex");
var DOCS_CLIENT_CONFIG_PATH = import_node_path.default.join(
  TOKEN_DIR,
  "google-workspace-mcp-oauth-client.json"
);
function expandScopes(scopes) {
  return scopes.map(
    (scope) => scope.startsWith("http") ? scope : `https://www.googleapis.com/auth/${scope}`
  );
}
function slugifyScopes(scopes) {
  return scopes.map((s) => s.replace(/[^a-zA-Z0-9.-]/g, "_")).join("+").slice(0, 120);
}
function buildAuthContext(profile, scopesOverride) {
  if (scopesOverride && scopesOverride.length > 0) {
    const expanded = expandScopes(scopesOverride);
    const slug = slugifyScopes(expanded);
    return {
      profile: "custom",
      scopes: expanded,
      tokenPath: import_node_path.default.join(
        TOKEN_DIR,
        `google-workspace-mcp-oauth-custom-${slug}.json`
      ),
      appName: "codex-google-workspace-mcp"
    };
  }
  const profileScopes = PROFILE_SCOPES[profile];
  if (!profileScopes) {
    const available = Object.keys(PROFILE_SCOPES).join(", ");
    throw new Error(
      `Unknown profile "${profile}". Choose one of: ${available}.`
    );
  }
  return {
    profile,
    scopes: expandScopes(profileScopes),
    tokenPath: import_node_path.default.join(
      TOKEN_DIR,
      `google-workspace-mcp-oauth-${profile}.json`
    ),
    appName: "codex-google-workspace-mcp"
  };
}
var authContext = buildAuthContext("full");
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
  await ensureOAuthTokenFile(scopes, tokenPath, clientConfigPath, appName);
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
      `[${appName}] No OAuth tokens found at ${tokenPath} after attempting setup. Run with --setup-auth in an interactive shell to retry.`
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
      `[${appName}] GOOGLE_OAUTH_CLIENT_ID and GOOGLE_OAUTH_CLIENT_SECRET must be set, or run --setup-auth to store them before running this command.`
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
    `[${appName}] OAuth tokens saved to ${tokenPath}. You can now start the MCP server normally.`
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
  const envClientId = import_node_process.default.env.GOOGLE_OAUTH_CLIENT_ID?.trim();
  const envClientSecret = import_node_process.default.env.GOOGLE_OAUTH_CLIENT_SECRET?.trim();
  if (envClientId && envClientSecret) {
    await import_promises.default.mkdir(import_node_path.default.dirname(configPath), { recursive: true });
    await import_promises.default.writeFile(
      configPath,
      JSON.stringify(
        { clientId: envClientId, clientSecret: envClientSecret },
        null,
        2
      ),
      "utf8"
    );
    console.error(
      `[${appName}] Saved OAuth client credentials from environment to ${configPath}.`
    );
    return;
  }
  const rl = (0, import_promises2.createInterface)({
    input: import_node_process.default.stdin,
    output: import_node_process.default.stdout
  });
  try {
    const clientId = (await rl.question(`[${appName}] Enter Google OAuth Client ID: `)).trim();
    const clientSecret = (await rl.question(`[${appName}] Enter Google OAuth Client Secret: `)).trim();
    if (!clientId || !clientSecret) {
      throw new Error(
        `[${appName}] Both client ID and client secret are required.`
      );
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
async function ensureOAuthTokenFile(scopes, tokenPath, clientConfigPath, appName) {
  const tokens = await loadStoredTokens(tokenPath);
  if (tokens) {
    return;
  }
  const haveCreds = haveOAuthEnv() || await hasStoredClientCredentials(clientConfigPath);
  if (!haveCreds) {
    if (!import_node_process.default.stdin.isTTY) {
      throw new Error(
        `[${appName}] OAuth client credentials not found. Run with --setup-auth in an interactive shell to configure them.`
      );
    }
    await setupAuthConfig(clientConfigPath, appName);
  } else {
    console.error(
      `[${appName}] OAuth tokens not found at ${tokenPath}; starting setup flow.`
    );
  }
  await runOAuthAuthorization(scopes, tokenPath, clientConfigPath, appName);
  const refreshedTokens = await loadStoredTokens(tokenPath);
  if (!refreshedTokens) {
    throw new Error(
      `[${appName}] OAuth tokens were not saved to ${tokenPath}.`
    );
  }
}
async function getDocsClient() {
  if (haveOAuthEnv() || await hasStoredClientCredentials(DOCS_CLIENT_CONFIG_PATH)) {
    const authClient = await getOAuthAuthorizedClient(
      authContext.scopes,
      authContext.tokenPath,
      DOCS_CLIENT_CONFIG_PATH,
      authContext.appName
    );
    return import_googleapis.google.docs({ version: "v1", auth: authClient });
  }
  const auth = new import_googleapis.google.auth.GoogleAuth({
    scopes: authContext.scopes
  });
  return import_googleapis.google.docs({ version: "v1", auth });
}
async function getDriveClient() {
  if (haveOAuthEnv() || await hasStoredClientCredentials(DOCS_CLIENT_CONFIG_PATH)) {
    const authClient = await getOAuthAuthorizedClient(
      authContext.scopes,
      authContext.tokenPath,
      DOCS_CLIENT_CONFIG_PATH,
      authContext.appName
    );
    return import_googleapis.google.drive({ version: "v3", auth: authClient });
  }
  const auth = new import_googleapis.google.auth.GoogleAuth({
    scopes: authContext.scopes
  });
  return import_googleapis.google.drive({ version: "v3", auth });
}
async function getSheetsClient() {
  if (haveOAuthEnv() || await hasStoredClientCredentials(DOCS_CLIENT_CONFIG_PATH)) {
    const authClient = await getOAuthAuthorizedClient(
      authContext.scopes,
      authContext.tokenPath,
      DOCS_CLIENT_CONFIG_PATH,
      authContext.appName
    );
    return import_googleapis.google.sheets({ version: "v4", auth: authClient });
  }
  const auth = new import_googleapis.google.auth.GoogleAuth({
    scopes: authContext.scopes
  });
  return import_googleapis.google.sheets({ version: "v4", auth });
}
async function hasApplicationDefaultCredentials(scopes) {
  try {
    const auth = new import_googleapis.google.auth.GoogleAuth({ scopes });
    await auth.getClient();
    return true;
  } catch {
    return false;
  }
}
async function ensureTokens() {
  const tokens = await loadStoredTokens(authContext.tokenPath);
  if (tokens) {
    console.error(
      `[${authContext.appName}] Using stored OAuth tokens at ${authContext.tokenPath}`
    );
    return;
  }
  const haveCreds = haveOAuthEnv() || await hasStoredClientCredentials(DOCS_CLIENT_CONFIG_PATH);
  if (haveCreds) {
    await ensureOAuthTokenFile(
      authContext.scopes,
      authContext.tokenPath,
      DOCS_CLIENT_CONFIG_PATH,
      authContext.appName
    );
    return;
  }
  const hasAdc = await hasApplicationDefaultCredentials(authContext.scopes);
  if (hasAdc) {
    console.error(
      `[${authContext.appName}] Using Application Default Credentials`
    );
    return;
  }
  await ensureOAuthTokenFile(
    authContext.scopes,
    authContext.tokenPath,
    DOCS_CLIENT_CONFIG_PATH,
    authContext.appName
  );
}
function appendLinkUrl(content, linkUrl) {
  if (!linkUrl || content.includes(linkUrl)) {
    return content;
  }
  const newlineMatch = content.match(/(\n+)$/);
  if (!newlineMatch) {
    return `${content} (${linkUrl})`;
  }
  const suffix = newlineMatch[1];
  const base = content.slice(0, -suffix.length);
  if (base.includes(linkUrl)) {
    return content;
  }
  return `${base} (${linkUrl})${suffix}`;
}
function extractTextFromDocument(document) {
  const doc = document;
  const body = doc?.body;
  if (!body || !Array.isArray(body.content)) {
    return "";
  }
  let text = "";
  for (const contentElement of body.content) {
    const paragraph = contentElement.paragraph;
    if (!paragraph || !Array.isArray(paragraph.elements)) {
      continue;
    }
    for (const element of paragraph.elements) {
      const richLink = element.richLink?.richLinkProperties;
      if (richLink) {
        const title = richLink.title?.trim() ?? "";
        const uri = richLink.uri;
        const display = title.length > 0 ? title : uri ?? "";
        if (display) {
          text += appendLinkUrl(display, uri);
        }
      }
      const runContent = element.textRun?.content;
      if (typeof runContent === "string") {
        const linkUrl = element.textRun?.textStyle?.link?.url;
        text += appendLinkUrl(runContent, linkUrl);
      }
    }
  }
  return text;
}
function formatSheetValues(values) {
  if (!Array.isArray(values)) {
    return { lines: [], rows: [] };
  }
  const rows = [];
  for (const row of values) {
    if (!Array.isArray(row)) {
      continue;
    }
    const normalized = row.map(
      (cell) => cell === null || cell === void 0 ? "" : String(cell)
    );
    rows.push(normalized);
  }
  return {
    lines: rows.map((row) => row.join("	")),
    rows
  };
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
function logToolCall(name, requestId) {
  const id = requestId ?? "n/a";
  console.error(
    `[${authContext.appName}] callTool name=${name} requestId=${id}`
  );
}
function logApiActivity(toolName, requestId, message) {
  const id = requestId ?? "n/a";
  console.error(
    `[${authContext.appName}] tool=${toolName} requestId=${id} ${message}`
  );
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
      name: "codex-google-workspace-mcp",
      version: "0.0.0-dev"
    },
    {
      capabilities: {
        tools: {}
      },
      instructions: "Tools for listing and reading Google Docs, listing and reading Google Sheets, and listing Google Drive files accessible to the configured Google account. Authentication can use Google Application Default Credentials or an interactive OAuth flow (see README)."
    }
  );
  server.server.onclose = () => {
    console.error(`[${authContext.appName}] Client disconnected`);
  };
  server.server.oninitialized = () => {
    console.error(`[${authContext.appName}] Initialization complete`);
  };
  server.server.onerror = (err) => {
    const message = err instanceof Error ? err.message : String(err);
    console.error(`[${authContext.appName}] MCP error: ${message}`);
  };
  server.registerTool(
    "list_documents",
    {
      title: "List Google Docs",
      description: "List recent Google Docs files in Drive for the configured account. Results are ordered by last modified time.",
      inputSchema: {
        query: z.string().optional().describe("Optional substring to match against document titles."),
        pageSize: z.number().int().min(1).max(100).optional().describe("Maximum number of documents to return (default 25).")
      }
    },
    async ({ query, pageSize }, extra) => {
      logToolCall("list_documents", extra?.requestId);
      try {
        const drive = await getDriveClient();
        const filters = [
          "mimeType = 'application/vnd.google-apps.document'",
          "trashed = false"
        ];
        if (query && query.trim().length > 0) {
          const escaped = query.replace(/'/g, "\\'");
          filters.push(`name contains '${escaped}'`);
        }
        const queryFilter = filters.join(" and ");
        const pageSizeValue = pageSize ?? 25;
        logApiActivity(
          "list_documents",
          extra?.requestId,
          `drive.files.list q=${queryFilter} pageSize=${pageSizeValue}`
        );
        const response = await drive.files.list({
          q: queryFilter,
          pageSize: pageSizeValue,
          fields: "files(id,name,owners(displayName),modifiedTime,webViewLink)",
          orderBy: "modifiedTime desc"
        });
        const files = response.data.files ?? [];
        logApiActivity(
          "list_documents",
          extra?.requestId,
          `drive.files.list returned ${files.length} file(s)`
        );
        if (files.length === 0) {
          return {
            content: [
              {
                type: "text",
                text: "No matching Google Docs documents found."
              }
            ]
          };
        }
        const lines = files.map((file) => {
          const id = file.id ?? "(no id)";
          const name = file.name ?? "(untitled)";
          const modified = file.modifiedTime ?? "unknown time";
          const owner = file.owners?.[0]?.displayName;
          const link = file.webViewLink;
          let line = `\u2022 ${name} (${id}) \u2014 modified ${modified}`;
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
        const message = err instanceof Error ? `Failed to list Google Docs documents: ${err.message}` : "Failed to list Google Docs documents.";
        return makeErrorResult(message);
      }
    }
  );
  server.registerTool(
    "list_spreadsheets",
    {
      title: "List Google Sheets",
      description: "List recent Google Sheets spreadsheets in Drive for the configured account. Results are ordered by last modified time.",
      inputSchema: {
        query: z.string().optional().describe("Optional substring to match against spreadsheet titles."),
        pageSize: z.number().int().min(1).max(100).optional().describe("Maximum number of spreadsheets to return (default 25).")
      }
    },
    async ({ query, pageSize }, extra) => {
      logToolCall("list_spreadsheets", extra?.requestId);
      try {
        const drive = await getDriveClient();
        const filters = [
          "mimeType = 'application/vnd.google-apps.spreadsheet'",
          "trashed = false"
        ];
        if (query && query.trim().length > 0) {
          const escaped = query.replace(/'/g, "\\'");
          filters.push(`name contains '${escaped}'`);
        }
        const queryFilter = filters.join(" and ");
        const pageSizeValue = pageSize ?? 25;
        logApiActivity(
          "list_spreadsheets",
          extra?.requestId,
          `drive.files.list q=${queryFilter} pageSize=${pageSizeValue}`
        );
        const response = await drive.files.list({
          q: queryFilter,
          pageSize: pageSizeValue,
          fields: "files(id,name,owners(displayName),modifiedTime,webViewLink)",
          orderBy: "modifiedTime desc"
        });
        const files = response.data.files ?? [];
        logApiActivity(
          "list_spreadsheets",
          extra?.requestId,
          `drive.files.list returned ${files.length} file(s)`
        );
        if (files.length === 0) {
          return {
            content: [
              {
                type: "text",
                text: "No matching Google Sheets spreadsheets found."
              }
            ]
          };
        }
        const lines = files.map((file) => {
          const id = file.id ?? "(no id)";
          const name = file.name ?? "(untitled)";
          const modified = file.modifiedTime ?? "unknown time";
          const owner = file.owners?.[0]?.displayName;
          const link = file.webViewLink;
          let line = `\u2022 ${name} (${id}) \u2014 modified ${modified}`;
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
        const message = err instanceof Error ? `Failed to list Google Sheets spreadsheets: ${err.message}` : "Failed to list Google Sheets spreadsheets.";
        return makeErrorResult(message);
      }
    }
  );
  server.registerTool(
    "get_sheet_values",
    {
      title: "Get Google Sheet values",
      description: "Fetch values from a Google Sheet range using A1 notation (e.g. Sheet1!A1:C10).",
      inputSchema: {
        spreadsheetId: z.string().describe("The spreadsheet ID portion of the Google Sheets URL."),
        range: z.string().min(1).describe("A1 notation range to fetch (e.g. Sheet1!A1:C10)."),
        majorDimension: z.enum(["ROWS", "COLUMNS"]).optional().describe(
          "Whether to return rows or columns. Defaults to ROWS if omitted."
        )
      }
    },
    async ({ spreadsheetId, range, majorDimension }, extra) => {
      logToolCall("get_sheet_values", extra?.requestId);
      try {
        const sheets = await getSheetsClient();
        const resolvedMajor = majorDimension ?? "ROWS";
        logApiActivity(
          "get_sheet_values",
          extra?.requestId,
          `sheets.spreadsheets.values.get spreadsheetId=${spreadsheetId} range=${range} majorDimension=${resolvedMajor}`
        );
        const response = await sheets.spreadsheets.values.get({
          spreadsheetId,
          range,
          majorDimension: resolvedMajor
        });
        const { lines, rows } = formatSheetValues(response.data.values);
        if (rows.length === 0) {
          return {
            content: [
              {
                type: "text",
                text: `No values found for range ${range} in spreadsheet ${spreadsheetId}.`
              }
            ],
            structuredContent: {
              values: []
            }
          };
        }
        return {
          content: [
            {
              type: "text",
              text: lines.join("\n")
            }
          ],
          structuredContent: {
            values: rows
          }
        };
      } catch (err) {
        const message = err instanceof Error ? `Failed to fetch Google Sheets values: ${err.message}` : "Failed to fetch Google Sheets values.";
        return makeErrorResult(message);
      }
    }
  );
  server.registerTool(
    "get_document_text",
    {
      title: "Get Google Doc text",
      description: "Fetch the plain text content of a Google Docs document.",
      inputSchema: {
        documentId: z.string().describe("The document ID portion of the Google Docs URL.")
      }
    },
    async ({ documentId }, extra) => {
      logToolCall("get_document_text", extra?.requestId);
      try {
        const docs = await getDocsClient();
        logApiActivity(
          "get_document_text",
          extra?.requestId,
          `docs.documents.get documentId=${documentId}`
        );
        const response = await docs.documents.get({
          documentId
        });
        const title = response.data.title ?? "";
        logApiActivity(
          "get_document_text",
          extra?.requestId,
          `docs.documents.get completed documentId=${documentId} title=${title || "(untitled)"}`
        );
        const text = extractTextFromDocument(response.data);
        const combined = title.trim().length > 0 ? `Title: ${title}

${text}` : text;
        return {
          content: [
            {
              type: "text",
              text: combined
            }
          ]
        };
      } catch (err) {
        const message = err instanceof Error ? `Failed to fetch Google Docs document: ${err.message}` : "Failed to fetch Google Docs document.";
        return makeErrorResult(message);
      }
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
    async ({ query, mimeType, pageSize }, extra) => {
      logToolCall("list_files", extra?.requestId);
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
        const queryFilter = filters.join(" and ");
        const pageSizeValue = pageSize ?? 25;
        logApiActivity(
          "list_files",
          extra?.requestId,
          `drive.files.list q=${queryFilter} pageSize=${pageSizeValue}`
        );
        const response = await drive.files.list({
          q: queryFilter,
          pageSize: pageSizeValue,
          fields: "files(id,name,mimeType,owners(displayName),modifiedTime,webViewLink)",
          orderBy: "modifiedTime desc"
        });
        const files = response.data.files ?? [];
        logApiActivity(
          "list_files",
          extra?.requestId,
          `drive.files.list returned ${files.length} file(s)`
        );
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
  console.error(`[${authContext.appName}] Server connected (transport=stdio)`);
}
async function main() {
  const args = import_node_process.default.argv.slice(2);
  let profile = "full";
  let scopesOverride;
  const profileFlagIndex = args.indexOf("--profile");
  if (profileFlagIndex !== -1) {
    const value = args[profileFlagIndex + 1];
    if (!value) {
      throw new Error("--profile requires a value (e.g. full or read).");
    }
    profile = value;
  }
  const scopesFlagIndex = args.indexOf("--scopes");
  if (scopesFlagIndex !== -1) {
    const value = args[scopesFlagIndex + 1];
    if (!value) {
      throw new Error(
        "--scopes requires a comma-separated list (e.g. documents.readonly,drive.metadata.readonly)."
      );
    }
    scopesOverride = value.split(",").map((s) => s.trim()).filter(Boolean);
  }
  if (scopesOverride && profileFlagIndex !== -1) {
    throw new Error("Use either --profile or --scopes, not both.");
  }
  authContext = buildAuthContext(profile, scopesOverride);
  if (args.includes("--setup-auth")) {
    await setupAuthConfig(DOCS_CLIENT_CONFIG_PATH, authContext.appName);
    await runOAuthAuthorization(
      authContext.scopes,
      authContext.tokenPath,
      DOCS_CLIENT_CONFIG_PATH,
      authContext.appName
    );
    return;
  }
  try {
    await ensureTokens();
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    console.error(
      `[${authContext.appName}] Warning: auth precheck failed (${message}). Continuing to start; tool calls may fail until credentials are configured.`
    );
  }
  await runServer();
}
if (typeof require !== "undefined" && typeof module !== "undefined" && require.main === module) {
  void main().catch((err) => {
    const message = err instanceof Error ? err.message : "Unexpected error starting server.";
    console.error(`codex-google-workspace-mcp: ${message}`);
    import_node_process.default.exit(1);
  });
}
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  extractTextFromDocument,
  formatSheetValues
});
