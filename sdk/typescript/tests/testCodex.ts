import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { Codex } from "../src/codex";
import type { CodexConfigObject } from "../src/codexOptions";

const codexExecPath = path.join(process.cwd(), "..", "..", "codex-rs", "target", "debug", "codex");

type CreateTestClientOptions = {
  apiKey?: string;
  baseUrl?: string;
  config?: CodexConfigObject;
  env?: Record<string, string>;
  inheritEnv?: boolean;
};

type CreateTestEnvOptions = {
  env?: Record<string, string>;
  inheritEnv?: boolean;
};

export type TestClient = {
  cleanup: () => void;
  client: Codex;
  codexHome: string;
};

export type TestEnv = {
  cleanup: () => void;
  codexHome: string;
  env: Record<string, string>;
};

export function createMockClient(url: string): TestClient {
  return createTestClient({
    config: {
      model_provider: "mock",
      model_providers: {
        mock: {
          name: "Mock provider for test",
          base_url: url,
          env_key: "PATH",
          wire_api: "responses",
          supports_websockets: false,
        },
      },
    },
  });
}

export function createTestEnv(options: CreateTestEnvOptions = {}): TestEnv {
  const codexHome = fs.mkdtempSync(path.join(os.tmpdir(), "codex-home-"));
  const baseEnv = options.inheritEnv === false ? {} : getCurrentEnv();

  return {
    codexHome,
    env: {
      ...baseEnv,
      ...options.env,
      CODEX_HOME: codexHome,
    },
    cleanup: () => {
      fs.rmSync(codexHome, { recursive: true, force: true });
    },
  };
}

export function createTestClient(options: CreateTestClientOptions = {}): TestClient {
  const { cleanup, codexHome, env } = createTestEnv({
    env: options.env,
    inheritEnv: options.inheritEnv,
  });

  return {
    cleanup,
    codexHome,
    client: new Codex({
      codexPathOverride: codexExecPath,
      baseUrl: options.baseUrl,
      apiKey: options.apiKey,
      config: options.config,
      env,
    }),
  };
}

function getCurrentEnv(): Record<string, string> {
  const env: Record<string, string> = {};

  for (const [key, value] of Object.entries(process.env)) {
    if (key === "CODEX_INTERNAL_ORIGINATOR_OVERRIDE") {
      continue;
    }
    if (value !== undefined) {
      env[key] = value;
    }
  }

  return env;
}
