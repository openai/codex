import { EventEmitter } from "node:events";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { PassThrough } from "node:stream";

import { parse, stringify } from "@iarna/toml";

import { McpManager, McpServerSummary } from "../src/mcp";
import { ConfigOverrideStore } from "../src/configOverrides";

jest.mock("node:child_process", () => {
  const actual = jest.requireActual<typeof import("node:child_process")>("node:child_process");
  return {
    ...actual,
    spawn: jest.fn(),
  };
});

const childProcessModule = jest.requireMock(
  "node:child_process",
) as typeof import("node:child_process");
const spawnMock = childProcessModule.spawn as jest.MockedFunction<
  (typeof import("node:child_process"))["spawn"]
>;

type SpawnOutcome = {
  stdout?: string;
  stderr?: string;
  exitCode?: number;
};

function mockSpawn({ stdout = "", stderr = "", exitCode = 0 }: SpawnOutcome): void {
  spawnMock.mockImplementation(() => {
    const proc = new EventEmitter() as unknown as import("node:child_process").ChildProcess;
    const stdoutStream = new PassThrough();
    const stderrStream = new PassThrough();
    stdoutStream.end(stdout);
    stderrStream.end(stderr);
    (proc as any).stdout = stdoutStream;
    (proc as any).stderr = stderrStream;
    setImmediate(() => {
      proc.emit("close", exitCode);
    });
    return proc;
  });
}

describe("McpManager", () => {
  beforeEach(() => {
    spawnMock.mockReset();
  });

  it("lists MCP servers via CLI", async () => {
    const servers: McpServerSummary[] = [
      {
        name: "context7",
        enabled: true,
        auth_status: "unknown",
        transport: { type: "stdio", command: "server", args: [] },
        startup_timeout_sec: null,
        tool_timeout_sec: null,
      },
    ];
    mockSpawn({ stdout: `${JSON.stringify(servers)}\n` });

    const manager = new McpManager({ configOverrides: new ConfigOverrideStore() });
    const result = await manager.list();

    expect(spawnMock).toHaveBeenCalledTimes(1);
    const [, args] = spawnMock.mock.calls[0]!;
    expect(args).toEqual(["mcp", "list", "--json"]);
    expect(result).toEqual(servers);
  });

  it("propagates CLI errors", async () => {
    mockSpawn({ stderr: "example failure", exitCode: 1 });
    const manager = new McpManager({ configOverrides: new ConfigOverrideStore() });

    await expect(manager.list()).rejects.toThrow("example failure");
  });

  it("passes env flags before stdio command when adding", async () => {
    mockSpawn({ stdout: "" });
    const manager = new McpManager({ configOverrides: new ConfigOverrideStore() });

    await manager.add(
      "context7",
      {
        type: "stdio",
        command: "server",
        args: ["start"],
        env: {
          FIRST: "1",
          SECOND: "2",
        },
      },
      {},
    );

    expect(spawnMock).toHaveBeenCalledTimes(1);
    const [, args] = spawnMock.mock.calls[0]!;
    expect(args).toEqual([
      "mcp",
      "add",
      "context7",
      "--env",
      "FIRST=1",
      "--env",
      "SECOND=2",
      "--",
      "server",
      "start",
    ]);
  });

  it("manages temporary overrides", () => {
    const overrides = new ConfigOverrideStore();
    const manager = new McpManager({ configOverrides: overrides });

    manager.enableOnce("context7", { enabledTools: ["search"] });
    expect(overrides.toCliArgs()).toEqual([
      "--config",
      "mcp_servers.context7.enabled=true",
      "--config",
      'mcp_servers.context7.enabled_tools=["search"]',
    ]);

    manager.disableOnce("context7");
    expect(overrides.toCliArgs()).toEqual(["--config", "mcp_servers.context7.enabled=false"]);
  });

  it("enables a server persistently", async () => {
    const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "codex-mcp-test-"));
    const configPath = path.join(tempDir, "config.toml");
    const initialConfig = stringify({
      mcp_servers: {
        context7: {
          command: "server",
          args: ["start"],
          enabled: false,
        },
      },
    });
    await fs.writeFile(configPath, initialConfig, "utf8");

    const manager = new McpManager({
      configOverrides: new ConfigOverrideStore(),
      configHomeOverride: tempDir,
    });

    await manager.enable("context7", { enabledTools: ["search", "summarize"] });

    const updated = await fs.readFile(configPath, "utf8");
    const parsed = parse(updated) as Record<string, any>;
    const server = parsed.mcp_servers.context7;
    expect(server.enabled).toBe(true);
    expect(server.enabled_tools).toEqual(["search", "summarize"]);
    expect(spawnMock).not.toHaveBeenCalled();

    await fs.rm(tempDir, { recursive: true, force: true });
  });
});
