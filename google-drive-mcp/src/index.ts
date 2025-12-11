import fs from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline/promises";
import process from "node:process";
import { google } from "googleapis";
import type { McpServer as McpServerType } from "@modelcontextprotocol/sdk/server/mcp";
import type { StdioServerTransport as StdioServerTransportType } from "@modelcontextprotocol/sdk/server/stdio";
import type { CallToolResult } from "@modelcontextprotocol/sdk/types";
import * as z from "zod";
// Reflect and CommonJS globals are available at runtime in modern Node, but
// may not be present in the default TypeScript lib types for this tsconfig.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
declare const Reflect: any;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
declare const require: any;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
declare const module: any;

const DRIVE_SCOPES = ["https://www.googleapis.com/auth/drive.readonly"];

const TOKEN_DIR =
  os.homedir() && os.homedir().length > 0
    ? path.join(os.homedir(), ".codex")
    : path.join(process.cwd(), ".codex");
const DRIVE_TOKEN_PATH = path.join(TOKEN_DIR, "google-drive-mcp-oauth.json");
const DRIVE_CLIENT_CONFIG_PATH = path.join(
  TOKEN_DIR,
  "google-drive-mcp-oauth-client.json",
);

function haveOAuthEnv(): boolean {
  return (
    typeof process.env.GOOGLE_OAUTH_CLIENT_ID === "string" &&
    process.env.GOOGLE_OAUTH_CLIENT_ID.length > 0 &&
    typeof process.env.GOOGLE_OAUTH_CLIENT_SECRET === "string" &&
    process.env.GOOGLE_OAUTH_CLIENT_SECRET.length > 0
  );
}

async function loadStoredTokens(tokenPath: string) {
  try {
    const data = await fs.readFile(tokenPath, "utf8");
    return JSON.parse(data);
  } catch {
    return null;
  }
}

async function getOAuthAuthorizedClient(
  scopes: string[],
  tokenPath: string,
  clientConfigPath: string,
  appName: string,
) {
  let clientId = process.env.GOOGLE_OAUTH_CLIENT_ID;
  let clientSecret = process.env.GOOGLE_OAUTH_CLIENT_SECRET;
  if (!clientId || !clientSecret) {
    try {
      const raw = await fs.readFile(clientConfigPath, "utf8");
      const parsed = JSON.parse(raw) as {
        clientId?: string;
        clientSecret?: string;
      };
      if (parsed.clientId && parsed.clientSecret) {
        clientId = parsed.clientId;
        clientSecret = parsed.clientSecret;
      }
    } catch {
      // ignore; handled below
    }
  }

  if (!clientId || !clientSecret) {
    throw new Error(
      `[${appName}] GOOGLE_OAUTH_CLIENT_ID and GOOGLE_OAUTH_CLIENT_SECRET must be set, or run --setup-auth to store them for reuse.`,
    );
  }

  const tokens = await loadStoredTokens(tokenPath);
  if (!tokens) {
    throw new Error(
      `[${appName}] No OAuth tokens found. Run \`npx @openai/codex-google-drive-mcp --authorize\` to complete the login flow.`,
    );
  }

  const oAuth2Client = new google.auth.OAuth2(clientId, clientSecret);
  oAuth2Client.setCredentials(tokens);
  if (!oAuth2Client.credentials.scope) {
    oAuth2Client.credentials.scope = scopes.join(" ");
  }

  return oAuth2Client;
}

function tryOpenInBrowser(url: string): void {
  const platform = process.platform;
  let command: string;
  let args: string[];

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
    const child = spawn(command, args, { stdio: "ignore", detached: true });
    child.unref();
  } catch {
    // Best-effort only; ignore failures and rely on the printed URL.
  }
}

async function runOAuthAuthorization(
  scopes: string[],
  tokenPath: string,
  clientConfigPath: string,
  appName: string,
): Promise<void> {
  let clientId = process.env.GOOGLE_OAUTH_CLIENT_ID;
  let clientSecret = process.env.GOOGLE_OAUTH_CLIENT_SECRET;
  if (!clientId || !clientSecret) {
    try {
      const raw = await fs.readFile(clientConfigPath, "utf8");
      const parsed = JSON.parse(raw) as {
        clientId?: string;
        clientSecret?: string;
      };
      if (parsed.clientId && parsed.clientSecret) {
        clientId = parsed.clientId;
        clientSecret = parsed.clientSecret;
      }
    } catch {
      // ignore; handled below
    }
  }

  if (!clientId || !clientSecret) {
    throw new Error(
      `[${appName}] GOOGLE_OAUTH_CLIENT_ID and GOOGLE_OAUTH_CLIENT_SECRET must be set, or run --setup-auth to store them before running --authorize.`,
    );
  }

  const server = http.createServer();
  const port = await new Promise<number>((resolve, reject) => {
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
  const oAuth2Client = new google.auth.OAuth2(
    clientId,
    clientSecret,
    redirectUri,
  );

  const authUrl = oAuth2Client.generateAuthUrl({
    access_type: "offline",
    scope: scopes,
    prompt: "consent",
  });

  // eslint-disable-next-line no-console
  console.error(
    `[${appName}] Open this URL in a browser to authorize access:\n\n${authUrl}\n`,
  );
  tryOpenInBrowser(authUrl);

  const code = await new Promise<string>((resolve, reject) => {
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

  await fs.mkdir(path.dirname(tokenPath), { recursive: true });
  await fs.writeFile(tokenPath, JSON.stringify(tokens, null, 2), "utf8");

  // eslint-disable-next-line no-console
  console.error(
    `[${appName}] OAuth tokens saved to ${tokenPath}. You can now start the MCP server without --authorize.`,
  );
}

async function hasStoredClientCredentials(configPath: string) {
  try {
    const raw = await fs.readFile(configPath, "utf8");
    const parsed = JSON.parse(raw) as {
      clientId?: string;
      clientSecret?: string;
    };
    return Boolean(parsed.clientId && parsed.clientSecret);
  } catch {
    return false;
  }
}

async function setupAuthConfig(configPath: string, appName: string) {
  const rl = createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  try {
    const clientId = (await rl.question(
      `[${appName}] Enter Google OAuth Client ID: `,
    )).trim();
    const clientSecret = (await rl.question(
      `[${appName}] Enter Google OAuth Client Secret: `,
    )).trim();

    if (!clientId || !clientSecret) {
      throw new Error(`[${appName}] Both client ID and client secret are required.`);
    }

    await fs.mkdir(path.dirname(configPath), { recursive: true });
    await fs.writeFile(
      configPath,
      JSON.stringify({ clientId, clientSecret }, null, 2),
      "utf8",
    );

    // eslint-disable-next-line no-console
    console.error(
      `[${appName}] Saved OAuth client credentials to ${configPath}.`,
    );
  } finally {
    rl.close();
  }
}

async function getDriveClient() {
  if (haveOAuthEnv() || (await hasStoredClientCredentials(DRIVE_CLIENT_CONFIG_PATH))) {
    const authClient = await getOAuthAuthorizedClient(
      DRIVE_SCOPES,
      DRIVE_TOKEN_PATH,
      DRIVE_CLIENT_CONFIG_PATH,
      "codex-google-drive-mcp",
    );
    return google.drive({ version: "v3", auth: authClient });
  }

  const auth = new google.auth.GoogleAuth({
    scopes: DRIVE_SCOPES,
  });
  return google.drive({ version: "v3", auth });
}

function makeErrorResult(message: string): CallToolResult {
  return {
    content: [
      {
        type: "text",
        text: message,
      },
    ],
    isError: true,
  };
}

function createMcpServer(
  ...args: any[]
): InstanceType<typeof McpServerType> {
  // eslint-disable-next-line @typescript-eslint/no-var-requires, global-require
  const mod = require("@modelcontextprotocol/sdk/server/mcp.js") as {
    McpServer: typeof McpServerType;
  };
  const Ctor = mod.McpServer as typeof McpServerType;
  // eslint-disable-next-line @typescript-eslint/no-unsafe-return
  return Reflect.construct(Ctor, args) as InstanceType<typeof McpServerType>;
}

function createStdioServerTransport(): StdioServerTransportType {
  // eslint-disable-next-line @typescript-eslint/no-var-requires, global-require
  const mod = require("@modelcontextprotocol/sdk/server/stdio.js") as {
    StdioServerTransport: typeof StdioServerTransportType;
  };
  const Ctor = mod.StdioServerTransport as typeof StdioServerTransportType;
  // eslint-disable-next-line @typescript-eslint/no-unsafe-return
  return new Ctor();
}

async function runServer(): Promise<void> {
  const server = createMcpServer(
    {
      name: "codex-google-drive-mcp",
      version: "0.0.0-dev",
    },
    {
      capabilities: {
        tools: {},
      },
      instructions:
        "Tools for listing files in Google Drive accessible to the configured Google account. Authentication can use Google Application Default Credentials or an interactive OAuth flow (see README).",
    },
  );

  server.registerTool(
    "list_files",
    {
      title: "List Google Drive files",
      description:
        "List recent files in Google Drive for the configured account.",
      inputSchema: {
        query: z
          .string()
          .optional()
          .describe("Optional substring to match against file names."),
        mimeType: z
          .string()
          .optional()
          .describe(
            "Optional MIME type filter, for example 'application/pdf' or 'application/vnd.google-apps.document'.",
          ),
        pageSize: z
          .number()
          .int()
          .min(1)
          .max(100)
          .optional()
          .describe("Maximum number of files to return (default 25)."),
      },
    },
    async ({ query, mimeType, pageSize }): Promise<CallToolResult> => {
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
          fields:
            "files(id,name,mimeType,owners(displayName),modifiedTime,webViewLink)",
          orderBy: "modifiedTime desc",
        });

        const files = response.data.files ?? [];
        if (files.length === 0) {
          return {
            content: [
              {
                type: "text",
                text: "No matching Google Drive files found.",
              },
            ],
          };
        }

        const lines = files.map((file) => {
          const id = file.id ?? "(no id)";
          const name = file.name ?? "(untitled)";
          const mime = file.mimeType ?? "unknown MIME type";
          const modified = file.modifiedTime ?? "unknown time";
          const owner = file.owners?.[0]?.displayName;
          const link = file.webViewLink;
          let line = `• ${name} (${id}) — ${mime}, modified ${modified}`;
          if (owner) {
            line += ` by ${owner}`;
          }
          if (link) {
            line += ` — ${link}`;
          }
          return line;
        });

        return {
          content: [
            {
              type: "text",
              text: lines.join("\n"),
            },
          ],
          structuredContent: {
            files,
          },
        };
      } catch (err) {
        const message =
          err instanceof Error
            ? `Failed to list Google Drive files: ${err.message}`
            : "Failed to list Google Drive files.";
        return makeErrorResult(message);
      }
    },
  );

  const transport = createStdioServerTransport();
  await server.connect(transport);
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  if (args.includes("--setup-auth")) {
    await setupAuthConfig(DRIVE_CLIENT_CONFIG_PATH, "codex-google-drive-mcp");
    await runOAuthAuthorization(
      DRIVE_SCOPES,
      DRIVE_TOKEN_PATH,
      DRIVE_CLIENT_CONFIG_PATH,
      "codex-google-drive-mcp",
    );
    return;
  }

  if (args.includes("--authorize")) {
    await runOAuthAuthorization(
      DRIVE_SCOPES,
      DRIVE_TOKEN_PATH,
      DRIVE_CLIENT_CONFIG_PATH,
      "codex-google-drive-mcp",
    );
    return;
  }

  await runServer();
}

if (
  typeof require !== "undefined" &&
  typeof module !== "undefined" &&
  require.main === module
) {
  void main().catch((err) => {
    const message =
      err instanceof Error ? err.message : "Unexpected error starting server.";
    // eslint-disable-next-line no-console
    console.error(`codex-google-drive-mcp: ${message}`);
    process.exit(1);
  });
}
