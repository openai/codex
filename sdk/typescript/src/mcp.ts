import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { parse, stringify } from "@iarna/toml";

import { CodexCliRunner, CodexCliError } from "./cliRunner";
import { ConfigOverrideStore } from "./configOverrides";
import type {
  EnableOnceOptions,
  McpAddTransportOptions,
  McpMutableFields,
  McpServerDetails,
  McpServerSummary,
  McpTransportSummary,
} from "./mcpOptions";

export type {
  EnableOnceOptions,
  McpAddTransportOptions,
  McpMutableFields,
  McpServerDetails,
  McpServerSummary,
  McpTransportSummary,
} from "./mcpOptions";

type ConfigDocument = Record<string, unknown> & {
  mcp_servers?: Record<string, Record<string, unknown>>;
};

export class McpManager {
  private readonly cliRunner: CodexCliRunner;
  private readonly overrideStore: ConfigOverrideStore;
  private readonly configHomeOverride: string | null;

  constructor(options: {
    codexPathOverride?: string | null;
    configOverrides: ConfigOverrideStore;
    configHomeOverride?: string | null;
  }) {
    const { codexPathOverride = null, configOverrides, configHomeOverride = null } = options;
    this.cliRunner = new CodexCliRunner(codexPathOverride);
    this.overrideStore = configOverrides;
    this.configHomeOverride = configHomeOverride;
  }

  async list(): Promise<McpServerSummary[]> {
    const result = await this.runCli(["mcp", "list", "--json"]);
    const trimmed = result.stdout.trim();
    if (!trimmed) {
      return [];
    }
    return JSON.parse(trimmed) as McpServerSummary[];
  }

  async get(name: string): Promise<McpServerDetails> {
    const result = await this.runCli(["mcp", "get", name, "--json"]);
    const trimmed = result.stdout.trim();
    if (!trimmed) {
      throw new Error(`No configuration returned for MCP server '${name}'.`);
    }
    return JSON.parse(trimmed) as McpServerDetails;
  }

  async add(name: string, transport: McpAddTransportOptions, fields: McpMutableFields = {}): Promise<void> {
    const args = ["mcp", "add", name];
    if (transport.type === "stdio") {
      args.push("--");
      args.push(transport.command);
      if (transport.args && transport.args.length) {
        args.push(...transport.args);
      }
      if (transport.env) {
        for (const [key, value] of Object.entries(transport.env)) {
          args.push("--env", `${key}=${value}`);
        }
      }
    } else {
      args.push("--url", transport.url);
      if (transport.bearerTokenEnvVar) {
        args.push("--bearer-token-env-var", transport.bearerTokenEnvVar);
      }
    }

    await this.runCli(args);

    await this.updateServerFields(name, fields);
  }

  async remove(name: string): Promise<void> {
    await this.runCli(["mcp", "remove", name]);
  }

  async login(name: string, scopes: string[] = []): Promise<void> {
    const args = ["mcp", "login", name];
    if (scopes.length) {
      args.push("--scopes", scopes.join(","));
    }
    await this.runCli(args);
  }

  async logout(name: string): Promise<void> {
    await this.runCli(["mcp", "logout", name]);
  }

  enableOnce(name: string, options: EnableOnceOptions = {}): void {
    this.overrideStore.set(`mcp_servers.${name}.enabled`, true);
    if (options.enabledTools !== undefined) {
      this.overrideStore.set(`mcp_servers.${name}.enabled_tools`, options.enabledTools);
    } else {
      this.overrideStore.delete(`mcp_servers.${name}.enabled_tools`);
    }
    if (options.disabledTools !== undefined) {
      this.overrideStore.set(`mcp_servers.${name}.disabled_tools`, options.disabledTools);
    } else {
      this.overrideStore.delete(`mcp_servers.${name}.disabled_tools`);
    }
  }

  disableOnce(name: string): void {
    this.overrideStore.set(`mcp_servers.${name}.enabled`, false);
    this.overrideStore.delete(`mcp_servers.${name}.enabled_tools`);
    this.overrideStore.delete(`mcp_servers.${name}.disabled_tools`);
  }

  async enable(name: string, options: EnableOnceOptions = {}): Promise<void> {
    await this.updateServerFields(name, {
      enabled: true,
      enabledTools: options.enabledTools ?? undefined,
      disabledTools: options.disabledTools ?? undefined,
    });
  }

  async disable(name: string): Promise<void> {
    await this.updateServerFields(name, { enabled: false });
  }

  async updateServerFields(name: string, fields: McpMutableFields = {}): Promise<void> {
    const mutableKeys = Object.keys(fields) as (keyof McpMutableFields)[];
    if (!mutableKeys.length) {
      return;
    }

    await this.modifyConfigDocument((doc) => {
      const servers = doc.mcp_servers as Record<string, Record<string, unknown>> | undefined;
      if (!servers || typeof servers !== "object") {
        throw new Error("No MCP servers configured.");
      }
      const server = servers[name];
      if (!server || typeof server !== "object") {
        throw new Error(`No MCP server named '${name}' found.`);
      }
      const table = server as Record<string, unknown>;

      for (const key of mutableKeys) {
        const value = fields[key];
        switch (key) {
          case "enabled":
            if (value === undefined) {
              break;
            }
            if (value === null) {
              delete table["enabled"];
            } else {
              table["enabled"] = value as boolean;
            }
            break;
          case "enabledTools":
            if (value === undefined) {
              break;
            }
            if (value === null) {
              delete table["enabled_tools"];
            } else {
              table["enabled_tools"] = value as string[];
            }
            break;
          case "disabledTools":
            if (value === undefined) {
              break;
            }
            if (value === null) {
              delete table["disabled_tools"];
            } else {
              table["disabled_tools"] = value as string[];
            }
            break;
          case "startupTimeoutSec":
            if (value === undefined) {
              break;
            }
            if (value === null) {
              delete table["startup_timeout_sec"];
            } else {
              table["startup_timeout_sec"] = value as number;
            }
            break;
          case "toolTimeoutSec":
            if (value === undefined) {
              break;
            }
            if (value === null) {
              delete table["tool_timeout_sec"];
            } else {
              table["tool_timeout_sec"] = value as number;
            }
            break;
        }
      }
    });
  }

  private async runCli(args: string[]): Promise<{ stdout: string; stderr: string }> {
    try {
      return await this.cliRunner.run(args);
    } catch (error) {
      if (error instanceof CodexCliError) {
        throw new Error(error.stderr.trim() || error.message);
      }
      throw error;
    }
  }

  private async modifyConfigDocument(mutator: (doc: ConfigDocument) => void): Promise<void> {
    const codexHome = this.resolveCodexHome();
    const configPath = path.join(codexHome, "config.toml");
    let raw = "";
    try {
      raw = await fs.readFile(configPath, "utf8");
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
        throw error;
      }
    }

    let document: ConfigDocument = {};
    const trimmed = raw.trim();
    if (trimmed.length > 0) {
      try {
        document = (parse(trimmed) as ConfigDocument) ?? {};
      } catch (error) {
        throw new Error(`Failed to parse Codex configuration: ${(error as Error).message}`);
      }
    }

    if (!document.mcp_servers) {
      document.mcp_servers = {};
    }

    mutator(document);

    const serialized = stringify(document as any);

    await fs.mkdir(codexHome, { recursive: true });
    const tmpPath = `${configPath}.${process.pid}.${Date.now()}`;
    await fs.writeFile(tmpPath, serialized, "utf8");
    await fs.rename(tmpPath, configPath);
  }

  private resolveCodexHome(): string {
    if (this.configHomeOverride) {
      return this.configHomeOverride;
    }
    const envHome = process.env.CODEX_HOME;
    if (envHome && envHome.trim().length > 0) {
      return envHome;
    }
    return path.join(os.homedir(), ".codex");
  }
}
