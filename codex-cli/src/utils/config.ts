// NOTE: We intentionally point the TypeScript import at the source file
// (`./auto-approval-mode.ts`) instead of the emitted `.js` bundle.  This makes
// the module resolvable when the project is executed via `ts-node`, which
// resolves *source* paths rather than built artefacts.  During a production
// build the TypeScript compiler will automatically rewrite the path to
// `./auto-approval-mode.js`, so the change is completely transparent for the
// compiled `dist/` output used by the published CLI.

import type { FullAutoErrorMode } from "./auto-approval-mode.js";
import type { ReasoningEffort } from "openai/resources.mjs";

import { AutoApprovalMode } from "./auto-approval-mode.js";
import { log } from "./logger/log.js";
import { providers } from "./providers.js";
import { config as loadDotenv } from "dotenv";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { load as loadYaml, dump as dumpYaml } from "js-yaml";
import { homedir } from "os";
import { dirname, join, extname, resolve as resolvePath } from "path";

// ---------------------------------------------------------------------------
// User‑wide environment config (~/.codex.env)
// ---------------------------------------------------------------------------

// Load a user‑level dotenv file **after** process.env and any project‑local
// .env file (loaded via "dotenv/config" in cli.tsx) are in place.  We rely on
// dotenv's default behaviour of *not* overriding existing variables so that
// the precedence order becomes:
//   1. Explicit environment variables
//   2. Project‑local .env (handled in cli.tsx)
//   3. User‑wide ~/.codex.env (loaded here)
// This guarantees that users can still override the global key on a per‑project
// basis while enjoying the convenience of a persistent default.

// Skip when running inside Vitest to avoid interfering with the FS mocks used
// by tests that stub out `fs` *after* importing this module.
const USER_WIDE_CONFIG_PATH = join(homedir(), ".codex.env");

const isVitest =
  typeof (globalThis as { vitest?: unknown }).vitest !== "undefined";

if (!isVitest) {
  loadDotenv({ path: USER_WIDE_CONFIG_PATH });
}

export const DEFAULT_AGENTIC_MODEL = "o4-mini";
export const DEFAULT_FULL_CONTEXT_MODEL = "gpt-4.1";
export const DEFAULT_APPROVAL_MODE = AutoApprovalMode.SUGGEST;
export const DEFAULT_INSTRUCTIONS = "";

// Default shell output limits
export const DEFAULT_SHELL_MAX_BYTES = 1024 * 10; // 10 KB
export const DEFAULT_SHELL_MAX_LINES = 256;

export const CONFIG_DIR = join(homedir(), ".codex");
export const CONFIG_JSON_FILEPATH = join(CONFIG_DIR, "config.json");
export const CONFIG_YAML_FILEPATH = join(CONFIG_DIR, "config.yaml");
export const CONFIG_YML_FILEPATH = join(CONFIG_DIR, "config.yml");

// Keep the original constant name for backward compatibility, but point it at
// the default JSON path. Code that relies on this constant will continue to
// work unchanged.
export const CONFIG_FILEPATH = CONFIG_JSON_FILEPATH;
export const INSTRUCTIONS_FILEPATH = join(CONFIG_DIR, "instructions.md");
export const PROXY_URL =
  process.env["https_proxy"] ||
  process.env["http_proxy"] ||
  process.env["proxy"] ||
  process.env["HTTPS_PROXY"] ||
  process.env["HTTP_PROXY"] ||
  process.env["PROXY"] ||
  undefined;

export const OPENAI_TIMEOUT_MS =
  parseInt(process.env["OPENAI_TIMEOUT_MS"] || "0", 10) || undefined;
export const OPENAI_BASE_URL = process.env["OPENAI_BASE_URL"] || "";
export let OPENAI_API_KEY = process.env["OPENAI_API_KEY"] || "";

export const DEFAULT_REASONING_EFFORT = "high";
export const OPENAI_ORGANIZATION = process.env["OPENAI_ORGANIZATION"] || "";
export const OPENAI_PROJECT = process.env["OPENAI_PROJECT"] || "";

// Can be set `true` when Codex is running in an environment that is marked as already
// considered sufficiently locked-down so that we allow running wihtout an explicit sandbox.
export const CODEX_UNSAFE_ALLOW_NO_SANDBOX = Boolean(
  process.env["CODEX_UNSAFE_ALLOW_NO_SANDBOX"] || "",
);

export function setApiKey(apiKey: string): void {
  OPENAI_API_KEY = apiKey;
}

export function getBaseUrl(provider: string = "openai"): string | undefined {
  // Check for a PROVIDER-specific override: e.g. OPENAI_BASE_URL or OLLAMA_BASE_URL.
  const envKey = `${provider.toUpperCase()}_BASE_URL`;
  if (process.env[envKey]) {
    return process.env[envKey];
  }

  // Get providers config from config file.
  const config = loadConfig();
  const providersConfig = config.providers ?? providers;
  const providerInfo = providersConfig[provider.toLowerCase()];
  if (providerInfo) {
    return providerInfo.baseURL;
  }

  // If the provider not found in the providers list and `OPENAI_BASE_URL` is set, use it.
  if (OPENAI_BASE_URL !== "") {
    return OPENAI_BASE_URL;
  }

  // We tried.
  return undefined;
}

export function getApiKey(provider: string = "openai"): string | undefined {
  const config = loadConfig();
  const providersConfig = config.providers ?? providers;
  const providerInfo = providersConfig[provider.toLowerCase()];
  if (providerInfo) {
    if (providerInfo.name === "Ollama") {
      return process.env[providerInfo.envKey] ?? "dummy";
    }
    return process.env[providerInfo.envKey];
  }

  // Checking `PROVIDER_API_KEY feels more intuitive with a custom provider.
  const customApiKey = process.env[`${provider.toUpperCase()}_API_KEY`];
  if (customApiKey) {
    return customApiKey;
  }

  // If the provider not found in the providers list and `OPENAI_API_KEY` is set, use it
  if (OPENAI_API_KEY !== "") {
    return OPENAI_API_KEY;
  }

  // We tried.
  return undefined;
}

// Represents config as persisted in config.json.
export type StoredConfig = {
  model?: string;
  provider?: string;
  approvalMode?: AutoApprovalMode;
  fullAutoErrorMode?: FullAutoErrorMode;
  memory?: MemoryConfig;
  /** Whether to enable desktop notifications for responses */
  notify?: boolean;
  /** Disable server-side response storage (send full transcript each request) */
  disableResponseStorage?: boolean;
  providers?: Record<string, { name: string; baseURL: string; envKey: string }>;
  history?: {
    maxSize?: number;
    saveHistory?: boolean;
    sensitivePatterns?: Array<string>;
  };
  tools?: {
    shell?: {
      maxBytes?: number;
      maxLines?: number;
    };
  };
  /** User-defined safe commands */
  safeCommands?: Array<string>;
  reasoningEffort?: ReasoningEffort;
};

// Minimal config written on first run.  An *empty* model string ensures that
// we always fall back to DEFAULT_MODEL on load, so updates to the default keep
// propagating to existing users until they explicitly set a model.
export const EMPTY_STORED_CONFIG: StoredConfig = { model: "" };

// Pre‑stringified JSON variant so we don't stringify repeatedly.
const EMPTY_CONFIG_JSON = JSON.stringify(EMPTY_STORED_CONFIG, null, 2) + "\n";

export type MemoryConfig = {
  enabled: boolean;
};

// Represents full runtime config, including loaded instructions.
export type AppConfig = {
  apiKey?: string;
  model: string;
  provider?: string;
  instructions: string;
  approvalMode?: AutoApprovalMode;
  fullAutoErrorMode?: FullAutoErrorMode;
  memory?: MemoryConfig;
  reasoningEffort?: ReasoningEffort;
  /** Whether to enable desktop notifications for responses */
  notify?: boolean;

  /** Disable server-side response storage (send full transcript each request) */
  disableResponseStorage?: boolean;

  /** Enable the "flex-mode" processing mode for supported models (o3, o4-mini) */
  flexMode?: boolean;
  providers?: Record<string, { name: string; baseURL: string; envKey: string }>;
  history?: {
    maxSize: number;
    saveHistory: boolean;
    sensitivePatterns: Array<string>;
  };
  tools?: {
    shell?: {
      maxBytes: number;
      maxLines: number;
    };
  };
};

// Formatting (quiet mode-only).
export const PRETTY_PRINT = Boolean(process.env["PRETTY_PRINT"] || "");

// ---------------------------------------------------------------------------
// Project doc support (codex.md)
// ---------------------------------------------------------------------------

export const PROJECT_DOC_MAX_BYTES = 32 * 1024; // 32 kB

const PROJECT_DOC_FILENAMES = ["codex.md", ".codex.md", "CODEX.md"];
const PROJECT_DOC_SEPARATOR = "\n\n--- project-doc ---\n\n";

export function discoverProjectDocPath(startDir: string): string | null {
  const cwd = resolvePath(startDir);

  // 1) Look in the explicit CWD first:
  for (const name of PROJECT_DOC_FILENAMES) {
    const direct = join(cwd, name);
    if (existsSync(direct)) {
      return direct;
    }
  }

  // 2) Fallback: walk up to the Git root and look there.
  let dir = cwd;
  // eslint-disable-next-line no-constant-condition
  while (true) {
    const gitPath = join(dir, ".git");
    if (existsSync(gitPath)) {
      // Once we hit the Git root, search its top‑level for the doc
      for (const name of PROJECT_DOC_FILENAMES) {
        const candidate = join(dir, name);
        if (existsSync(candidate)) {
          return candidate;
        }
      }
      // If Git root but no doc, stop looking.
      return null;
    }

    const parent = dirname(dir);
    if (parent === dir) {
      // Reached filesystem root without finding Git.
      return null;
    }
    dir = parent;
  }
}

/**
 * Load the project documentation markdown (codex.md) if present. If the file
 * exceeds {@link PROJECT_DOC_MAX_BYTES} it will be truncated and a warning is
 * logged.
 *
 * @param cwd The current working directory of the caller
 * @param explicitPath If provided, skips discovery and loads the given path
 */
export function loadProjectDoc(cwd: string, explicitPath?: string): string {
  let filepath: string | null = null;

  if (explicitPath) {
    filepath = resolvePath(cwd, explicitPath);
    if (!existsSync(filepath)) {
      // eslint-disable-next-line no-console
      console.warn(`codex: project doc not found at ${filepath}`);
      filepath = null;
    }
  } else {
    filepath = discoverProjectDocPath(cwd);
  }

  if (!filepath) {
    return "";
  }

  try {
    const buf = readFileSync(filepath);
    if (buf.byteLength > PROJECT_DOC_MAX_BYTES) {
      // eslint-disable-next-line no-console
      console.warn(
        `codex: project doc '${filepath}' exceeds ${PROJECT_DOC_MAX_BYTES} bytes – truncating.`,
      );
    }
    return buf.slice(0, PROJECT_DOC_MAX_BYTES).toString("utf-8");
  } catch {
    return "";
  }
}

export type LoadConfigOptions = {
  /** Working directory used for project doc discovery */
  cwd?: string;
  /** Disable inclusion of the project doc */
  disableProjectDoc?: boolean;
  /** Explicit path to project doc (overrides discovery) */
  projectDocPath?: string;
  /** Whether we are in fullcontext mode. */
  isFullContext?: boolean;
};

export const loadConfig = (
  configPath: string | undefined = CONFIG_FILEPATH,
  instructionsPath: string | undefined = INSTRUCTIONS_FILEPATH,
  options: LoadConfigOptions = {},
): AppConfig => {
  // Determine the actual path to load. If the provided path doesn't exist and
  // the caller passed the default JSON path, automatically fall back to YAML
  // variants.
  let actualConfigPath = configPath;
  if (!existsSync(actualConfigPath)) {
    if (configPath === CONFIG_FILEPATH) {
      if (existsSync(CONFIG_YAML_FILEPATH)) {
        actualConfigPath = CONFIG_YAML_FILEPATH;
      } else if (existsSync(CONFIG_YML_FILEPATH)) {
        actualConfigPath = CONFIG_YML_FILEPATH;
      }
    }
  }

  let storedConfig: StoredConfig = {};
  if (existsSync(actualConfigPath)) {
    const raw = readFileSync(actualConfigPath, "utf-8");
    const ext = extname(actualConfigPath).toLowerCase();
    try {
      if (ext === ".yaml" || ext === ".yml") {
        storedConfig = loadYaml(raw) as unknown as StoredConfig;
      } else {
        storedConfig = JSON.parse(raw);
      }
    } catch {
      // If parsing fails, fall back to empty config to avoid crashing.
      storedConfig = {};
    }
  }

  if (
    storedConfig.disableResponseStorage !== undefined &&
    typeof storedConfig.disableResponseStorage !== "boolean"
  ) {
    if (storedConfig.disableResponseStorage === "true") {
      storedConfig.disableResponseStorage = true;
    } else if (storedConfig.disableResponseStorage === "false") {
      storedConfig.disableResponseStorage = false;
    } else {
      log(
        `[codex] Warning: 'disableResponseStorage' in config is not a boolean (got '${storedConfig.disableResponseStorage}'). Ignoring this value.`,
      );
      delete storedConfig.disableResponseStorage;
    }
  }

  const instructionsFilePathResolved =
    instructionsPath ?? INSTRUCTIONS_FILEPATH;
  const userInstructions = existsSync(instructionsFilePathResolved)
    ? readFileSync(instructionsFilePathResolved, "utf-8")
    : DEFAULT_INSTRUCTIONS;

  // Project doc support.
  const shouldLoadProjectDoc =
    !options.disableProjectDoc &&
    process.env["CODEX_DISABLE_PROJECT_DOC"] !== "1";

  let projectDoc = "";
  let projectDocPath: string | null = null;
  if (shouldLoadProjectDoc) {
    const cwd = options.cwd ?? process.cwd();
    projectDoc = loadProjectDoc(cwd, options.projectDocPath);
    projectDocPath = options.projectDocPath
      ? resolvePath(cwd, options.projectDocPath)
      : discoverProjectDocPath(cwd);
    if (projectDocPath) {
      log(
        `[codex] Loaded project doc from ${projectDocPath} (${projectDoc.length} bytes)`,
      );
    } else {
      log(`[codex] No project doc found in ${cwd}`);
    }
  }

  const combinedInstructions = [userInstructions, projectDoc]
    .filter((s) => s && s.trim() !== "")
    .join(PROJECT_DOC_SEPARATOR);

  // Treat empty string ("" or whitespace) as absence so we can fall back to
  // the latest DEFAULT_MODEL.
  const storedModel =
    storedConfig.model && storedConfig.model.trim() !== ""
      ? storedConfig.model.trim()
      : undefined;

  const config: AppConfig = {
    model:
      storedModel ??
      (options.isFullContext
        ? DEFAULT_FULL_CONTEXT_MODEL
        : DEFAULT_AGENTIC_MODEL),
    provider: storedConfig.provider,
    instructions: combinedInstructions,
    notify: storedConfig.notify === true,
    approvalMode: storedConfig.approvalMode,
    tools: {
      shell: {
        maxBytes:
          storedConfig.tools?.shell?.maxBytes ?? DEFAULT_SHELL_MAX_BYTES,
        maxLines:
          storedConfig.tools?.shell?.maxLines ?? DEFAULT_SHELL_MAX_LINES,
      },
    },
    disableResponseStorage: storedConfig.disableResponseStorage === true,
    reasoningEffort: storedConfig.reasoningEffort,
  };

  // -----------------------------------------------------------------------
  // First‑run bootstrap: if the configuration file (and/or its containing
  // directory) didn't exist we create them now so that users end up with a
  // materialised ~/.codex/config.json file on first execution.  This mirrors
  // what `saveConfig()` would do but without requiring callers to remember to
  // invoke it separately.
  //
  // We intentionally perform this *after* we have computed the final
  // `config` object so that we can just persist the resolved defaults.  The
  // write operations are guarded by `existsSync` checks so that subsequent
  // runs that already have a config will remain read‑only here.
  // -----------------------------------------------------------------------

  try {
    if (!existsSync(actualConfigPath)) {
      // Ensure the directory exists first.
      const dir = dirname(actualConfigPath);
      if (!existsSync(dir)) {
        mkdirSync(dir, { recursive: true });
      }

      // Persist a minimal config – we include the `model` key but leave it as
      // an empty string so that `loadConfig()` treats it as "unset" and falls
      // back to whatever DEFAULT_MODEL is current at runtime.  This prevents
      // pinning users to an old default after upgrading Codex.
      const ext = extname(actualConfigPath).toLowerCase();
      if (ext === ".yaml" || ext === ".yml") {
        writeFileSync(actualConfigPath, dumpYaml(EMPTY_STORED_CONFIG), "utf-8");
      } else {
        writeFileSync(actualConfigPath, EMPTY_CONFIG_JSON, "utf-8");
      }
    }

    // Always ensure the instructions file exists so users can edit it.
    if (!existsSync(instructionsFilePathResolved)) {
      const instrDir = dirname(instructionsFilePathResolved);
      if (!existsSync(instrDir)) {
        mkdirSync(instrDir, { recursive: true });
      }
      writeFileSync(instructionsFilePathResolved, userInstructions, "utf-8");
    }
  } catch {
    // Silently ignore any errors – failure to persist the defaults shouldn't
    // block the CLI from starting.  A future explicit `codex config` command
    // or `saveConfig()` call can handle (re‑)writing later.
  }

  // Only include the "memory" key if it was explicitly set by the user. This
  // preserves backward‑compatibility with older config files (and our test
  // fixtures) that don't include a "memory" section.
  if (storedConfig.memory !== undefined) {
    config.memory = storedConfig.memory;
  }

  if (storedConfig.fullAutoErrorMode) {
    config.fullAutoErrorMode = storedConfig.fullAutoErrorMode;
  }
  // Notification setting: enable desktop notifications when set in config
  config.notify = storedConfig.notify === true;

  // Add default history config if not provided
  if (storedConfig.history !== undefined) {
    config.history = {
      maxSize: storedConfig.history.maxSize ?? 1000,
      saveHistory: storedConfig.history.saveHistory ?? true,
      sensitivePatterns: storedConfig.history.sensitivePatterns ?? [],
    };
  } else {
    config.history = {
      maxSize: 1000,
      saveHistory: true,
      sensitivePatterns: [],
    };
  }

  // Merge default providers with user configured providers in the config.
  config.providers = { ...providers, ...storedConfig.providers };

  return config;
};

export const saveConfig = (
  config: AppConfig,
  configPath = CONFIG_FILEPATH,
  instructionsPath = INSTRUCTIONS_FILEPATH,
): void => {
  // If the caller passed the default JSON path *and* a YAML config already
  // exists on disk, save back to that YAML file instead to preserve the
  // user's chosen format.
  let targetPath = configPath;
  if (
    configPath === CONFIG_FILEPATH &&
    !existsSync(configPath) &&
    (existsSync(CONFIG_YAML_FILEPATH) || existsSync(CONFIG_YML_FILEPATH))
  ) {
    targetPath = existsSync(CONFIG_YAML_FILEPATH)
      ? CONFIG_YAML_FILEPATH
      : CONFIG_YML_FILEPATH;
  }

  const dir = dirname(targetPath);
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }

  const ext = extname(targetPath).toLowerCase();
  // Create the config object to save
  const configToSave: StoredConfig = {
    model: config.model,
    provider: config.provider,
    providers: config.providers,
    approvalMode: config.approvalMode,
    disableResponseStorage: config.disableResponseStorage,
    reasoningEffort: config.reasoningEffort,
  };

  // Add history settings if they exist
  if (config.history) {
    configToSave.history = {
      maxSize: config.history.maxSize,
      saveHistory: config.history.saveHistory,
      sensitivePatterns: config.history.sensitivePatterns,
    };
  }

  // Add tools settings if they exist
  if (config.tools) {
    configToSave.tools = {
      shell: config.tools.shell
        ? {
            maxBytes: config.tools.shell.maxBytes,
            maxLines: config.tools.shell.maxLines,
          }
        : undefined,
    };
  }

  if (ext === ".yaml" || ext === ".yml") {
    writeFileSync(targetPath, dumpYaml(configToSave), "utf-8");
  } else {
    writeFileSync(targetPath, JSON.stringify(configToSave, null, 2), "utf-8");
  }

  // Take everything before the first PROJECT_DOC_SEPARATOR (or the whole string if none).
  const [userInstructions = ""] = config.instructions.split(
    PROJECT_DOC_SEPARATOR,
  );
  writeFileSync(instructionsPath, userInstructions, "utf-8");
};
