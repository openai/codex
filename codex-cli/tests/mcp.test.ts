import type * as fsType from "fs";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

// In‑memory FS store
let memfs: Record<string, string> = {};

// Mock the config module to directly mock getMcpServers
vi.mock("../src/utils/config", async () => {
  const actual = await vi.importActual("../src/utils/config");
  return {
    ...actual,
    getMcpServers: vi.fn(() => {
      // This will be controlled in our test
      return testMcpServers;
    }),
  };
});

// Mock out the parts of "fs" that our config module uses:
vi.mock("fs", async () => {
  // now `real` is the actual fs module
  const real = (await vi.importActual("fs")) as typeof fsType;
  return {
    ...real,
    existsSync: (path: string) => memfs[path] !== undefined,
    readFileSync: (path: string) => {
      if (memfs[path] === undefined) {
        throw new Error("ENOENT");
      }
      return memfs[path];
    },
    writeFileSync: (path: string, data: string) => {
      memfs[path] = data;
    },
    mkdirSync: () => {
      // no-op in in‑memory store
    },
    rmSync: (path: string) => {
      // recursively delete any key under this prefix
      const prefix = path.endsWith("/") ? path : path + "/";
      for (const key of Object.keys(memfs)) {
        if (key === path || key.startsWith(prefix)) {
          delete memfs[key];
        }
      }
    },
  };
});

// Import after mocks are defined
import { buildClients } from "../src/utils/mcp/build-clients";

// Test data
const testMcpServers: Record<string, any> = {
  memory: {
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-memory"],
  },
};

beforeEach(() => {
  memfs = {}; // reset in‑memory store
});

afterEach(() => {
  memfs = {};
});

test("loads MCP servers correctly from user config", () => {
  const clients = buildClients();
  expect(clients).toHaveLength(1);
  expect(clients[0]?.name).toBe("memory");
});

test(
  "loads MCP tools",
  async () => {
    const [client, _] = buildClients();
    expect(client).toBeDefined();
    await client?.connectToServer(
      testMcpServers["memory"].command,
      testMcpServers["memory"].args,
    );
    const tools = await client?.getTools();
    expect(tools).toBeDefined();
    expect(tools?.length).toBe(9);
  },
  {
    timeout: 1000000,
  },
);
