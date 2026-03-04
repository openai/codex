// Node-based kernel for js_repl.
// Communicates over JSON lines on stdin/stdout.
// Requires Node started with --experimental-vm-modules.

const { Buffer } = require("node:buffer");
const crypto = require("node:crypto");
const fs = require("node:fs");
const { builtinModules, createRequire } = require("node:module");
const { createInterface } = require("node:readline");
const { performance } = require("node:perf_hooks");
const path = require("node:path");
const { URL, URLSearchParams, pathToFileURL } = require("node:url");
const { inspect, TextDecoder, TextEncoder } = require("node:util");
const vm = require("node:vm");

const { SourceTextModule, SyntheticModule } = vm;
const meriyahPromise = import("./meriyah.umd.min.js").then(
  (m) => m.default ?? m,
);

// vm contexts start with very few globals. Populate common Node/web globals
// so snippets and dependencies behave like a normal modern JS runtime.
const context = vm.createContext({});
context.globalThis = context;
context.global = context;
context.Buffer = Buffer;
context.console = console;
context.URL = URL;
context.URLSearchParams = URLSearchParams;
if (typeof TextEncoder !== "undefined") {
  context.TextEncoder = TextEncoder;
}
if (typeof TextDecoder !== "undefined") {
  context.TextDecoder = TextDecoder;
}
if (typeof AbortController !== "undefined") {
  context.AbortController = AbortController;
}
if (typeof AbortSignal !== "undefined") {
  context.AbortSignal = AbortSignal;
}
if (typeof structuredClone !== "undefined") {
  context.structuredClone = structuredClone;
}
if (typeof fetch !== "undefined") {
  context.fetch = fetch;
}
if (typeof Headers !== "undefined") {
  context.Headers = Headers;
}
if (typeof Request !== "undefined") {
  context.Request = Request;
}
if (typeof Response !== "undefined") {
  context.Response = Response;
}
if (typeof performance !== "undefined") {
  context.performance = performance;
}
context.crypto = crypto.webcrypto ?? crypto;
context.setTimeout = setTimeout;
context.clearTimeout = clearTimeout;
context.setInterval = setInterval;
context.clearInterval = clearInterval;
context.queueMicrotask = queueMicrotask;
if (typeof setImmediate !== "undefined") {
  context.setImmediate = setImmediate;
  context.clearImmediate = clearImmediate;
}
context.atob = (data) => Buffer.from(data, "base64").toString("binary");
context.btoa = (data) => Buffer.from(data, "binary").toString("base64");

/**
 * @typedef {{ name: string, kind: "const"|"let"|"var"|"function"|"class" }} Binding
 */

// REPL state model:
// - Every exec is compiled as a fresh ESM "cell".
// - `previousModule` is the most recently evaluated module namespace.
// - `previousBindings` tracks which top-level names should be carried forward.
// Each new cell imports a synthetic view of the previous namespace and
// redeclares those names so user variables behave like a persistent REPL.
let previousModule = null;
/** @type {Binding[]} */
let previousBindings = [];
let cellCounter = 0;
let activeExecId = null;
let fatalExitScheduled = false;

const builtinModuleSet = new Set([
  ...builtinModules,
  ...builtinModules.map((name) => `node:${name}`),
]);
const deniedBuiltinModules = new Set([
  "process",
  "node:process",
  "child_process",
  "node:child_process",
  "worker_threads",
  "node:worker_threads",
]);

function toNodeBuiltinSpecifier(specifier) {
  return specifier.startsWith("node:") ? specifier : `node:${specifier}`;
}

function isDeniedBuiltin(specifier) {
  const normalized = specifier.startsWith("node:")
    ? specifier.slice(5)
    : specifier;
  return (
    deniedBuiltinModules.has(specifier) || deniedBuiltinModules.has(normalized)
  );
}

/** @type {Map<string, (msg: any) => void>} */
const pendingTool = new Map();
/** @type {Map<string, (msg: any) => void>} */
const pendingEmitImage = new Map();
let toolCounter = 0;
let emitImageCounter = 0;
const tmpDir = process.env.CODEX_JS_TMP_DIR || process.cwd();
const nodeModuleDirEnv = process.env.CODEX_JS_REPL_NODE_MODULE_DIRS ?? "";
const moduleSearchBases = (() => {
  const bases = [];
  const seen = new Set();
  for (const entry of nodeModuleDirEnv.split(path.delimiter)) {
    const trimmed = entry.trim();
    if (!trimmed) {
      continue;
    }
    const resolved = path.isAbsolute(trimmed)
      ? trimmed
      : path.resolve(process.cwd(), trimmed);
    const base =
      path.basename(resolved) === "node_modules"
        ? path.dirname(resolved)
        : resolved;
    if (seen.has(base)) {
      continue;
    }
    seen.add(base);
    bases.push(base);
  }
  const cwd = process.cwd();
  if (!seen.has(cwd)) {
    bases.push(cwd);
  }
  return bases;
})();

const importResolveConditions = new Set(["node", "import"]);
const requireByBase = new Map();

function canonicalizePath(value) {
  try {
    return fs.realpathSync.native(value);
  } catch {
    return value;
  }
}

function getRequireForBase(base) {
  let req = requireByBase.get(base);
  if (!req) {
    req = createRequire(path.join(base, "__codex_js_repl__.cjs"));
    requireByBase.set(base, req);
  }
  return req;
}

function isModuleNotFoundError(err) {
  return (
    err?.code === "MODULE_NOT_FOUND" || err?.code === "ERR_MODULE_NOT_FOUND"
  );
}

function isWithinBaseNodeModules(base, resolvedPath) {
  const canonicalBase = canonicalizePath(base);
  const canonicalResolved = canonicalizePath(resolvedPath);
  const nodeModulesRoot = path.resolve(canonicalBase, "node_modules");
  const relative = path.relative(nodeModulesRoot, canonicalResolved);
  return (
    relative !== "" && !relative.startsWith("..") && !path.isAbsolute(relative)
  );
}

function isBarePackageSpecifier(specifier) {
  if (
    typeof specifier !== "string" ||
    !specifier ||
    specifier.trim() !== specifier
  ) {
    return false;
  }
  if (specifier.startsWith("./") || specifier.startsWith("../")) {
    return false;
  }
  if (specifier.startsWith("/") || specifier.startsWith("\\")) {
    return false;
  }
  if (path.isAbsolute(specifier)) {
    return false;
  }
  if (/^[a-zA-Z][a-zA-Z\d+.-]*:/.test(specifier)) {
    return false;
  }
  if (specifier.includes("\\")) {
    return false;
  }
  return true;
}

function resolveBareSpecifier(specifier) {
  let firstResolutionError = null;

  for (const base of moduleSearchBases) {
    try {
      const resolved = getRequireForBase(base).resolve(specifier, {
        conditions: importResolveConditions,
      });
      if (isWithinBaseNodeModules(base, resolved)) {
        return resolved;
      }
      // Ignore resolutions that escape this base via parent node_modules lookup.
    } catch (err) {
      if (isModuleNotFoundError(err)) {
        continue;
      }
      if (!firstResolutionError) {
        firstResolutionError = err;
      }
    }
  }

  if (firstResolutionError) {
    throw firstResolutionError;
  }
  return null;
}

function resolveSpecifier(specifier) {
  if (specifier.startsWith("node:") || builtinModuleSet.has(specifier)) {
    if (isDeniedBuiltin(specifier)) {
      throw new Error(
        `Importing module "${specifier}" is not allowed in js_repl`,
      );
    }
    return { kind: "builtin", specifier: toNodeBuiltinSpecifier(specifier) };
  }

  if (!isBarePackageSpecifier(specifier)) {
    throw new Error(
      `Unsupported import specifier "${specifier}" in js_repl. Use a package name like "lodash" or "@scope/pkg".`,
    );
  }

  const resolvedBare = resolveBareSpecifier(specifier);
  if (!resolvedBare) {
    throw new Error(`Module not found: ${specifier}`);
  }

  return { kind: "path", path: resolvedBare };
}

function importResolved(resolved) {
  if (resolved.kind === "builtin") {
    return import(resolved.specifier);
  }
  if (resolved.kind === "path") {
    return import(pathToFileURL(resolved.path).href);
  }
  throw new Error(`Unsupported module resolution kind: ${resolved.kind}`);
}

function collectPatternNames(pattern, kind, map) {
  if (!pattern) return;
  switch (pattern.type) {
    case "Identifier":
      if (!map.has(pattern.name)) map.set(pattern.name, kind);
      return;
    case "ObjectPattern":
      for (const prop of pattern.properties ?? []) {
        if (prop.type === "Property") {
          collectPatternNames(prop.value, kind, map);
        } else if (prop.type === "RestElement") {
          collectPatternNames(prop.argument, kind, map);
        }
      }
      return;
    case "ArrayPattern":
      for (const elem of pattern.elements ?? []) {
        if (!elem) continue;
        if (elem.type === "RestElement") {
          collectPatternNames(elem.argument, kind, map);
        } else {
          collectPatternNames(elem, kind, map);
        }
      }
      return;
    case "AssignmentPattern":
      collectPatternNames(pattern.left, kind, map);
      return;
    case "RestElement":
      collectPatternNames(pattern.argument, kind, map);
      return;
    default:
      return;
  }
}

function collectBindings(ast) {
  const map = new Map();
  for (const stmt of ast.body ?? []) {
    if (stmt.type === "VariableDeclaration") {
      const kind = stmt.kind;
      for (const decl of stmt.declarations) {
        collectPatternNames(decl.id, kind, map);
      }
    } else if (stmt.type === "FunctionDeclaration" && stmt.id) {
      map.set(stmt.id.name, "function");
    } else if (stmt.type === "ClassDeclaration" && stmt.id) {
      map.set(stmt.id.name, "class");
    } else if (stmt.type === "ForStatement") {
      if (
        stmt.init &&
        stmt.init.type === "VariableDeclaration" &&
        stmt.init.kind === "var"
      ) {
        for (const decl of stmt.init.declarations) {
          collectPatternNames(decl.id, "var", map);
        }
      }
    } else if (
      stmt.type === "ForInStatement" ||
      stmt.type === "ForOfStatement"
    ) {
      if (
        stmt.left &&
        stmt.left.type === "VariableDeclaration" &&
        stmt.left.kind === "var"
      ) {
        for (const decl of stmt.left.declarations) {
          collectPatternNames(decl.id, "var", map);
        }
      }
    }
  }
  return Array.from(map.entries()).map(([name, kind]) => ({ name, kind }));
}

async function buildModuleSource(code) {
  const meriyah = await meriyahPromise;
  const ast = meriyah.parseModule(code, {
    next: true,
    module: true,
    ranges: false,
    loc: false,
    disableWebCompat: true,
  });
  const currentBindings = collectBindings(ast);
  const priorBindings = previousModule ? previousBindings : [];

  let prelude = "";
  if (previousModule && priorBindings.length) {
    // Recreate carried bindings before running user code in this new cell.
    prelude += 'import * as __prev from "@prev";\n';
    prelude += priorBindings
      .map((b) => {
        const keyword =
          b.kind === "var" ? "var" : b.kind === "const" ? "const" : "let";
        return `${keyword} ${b.name} = __prev.${b.name};`;
      })
      .join("\n");
    prelude += "\n";
  }

  const mergedBindings = new Map();
  for (const binding of priorBindings) {
    mergedBindings.set(binding.name, binding.kind);
  }
  for (const binding of currentBindings) {
    mergedBindings.set(binding.name, binding.kind);
  }
  // Export the merged binding set so the next cell can import it through @prev.
  const exportNames = Array.from(mergedBindings.keys());
  const exportStmt = exportNames.length
    ? `\nexport { ${exportNames.join(", ")} };`
    : "";

  const nextBindings = Array.from(mergedBindings, ([name, kind]) => ({
    name,
    kind,
  }));
  return { source: `${prelude}${code}${exportStmt}`, nextBindings };
}

function send(message) {
  process.stdout.write(JSON.stringify(message));
  process.stdout.write("\n");
}

function formatErrorMessage(error) {
  if (error && typeof error === "object" && "message" in error) {
    return error.message ? String(error.message) : String(error);
  }
  return String(error);
}

function sendFatalExecResultSync(kind, error) {
  if (!activeExecId) {
    return;
  }
  const payload = {
    type: "exec_result",
    id: activeExecId,
    ok: false,
    output: "",
    error: `js_repl kernel ${kind}: ${formatErrorMessage(error)}; kernel reset. Catch or handle async errors (including Promise rejections and EventEmitter 'error' events) to avoid kernel termination.`,
  };
  try {
    fs.writeSync(process.stdout.fd, `${JSON.stringify(payload)}\n`);
  } catch {
    // Best effort only; the host will still surface stdout EOF diagnostics.
  }
}

function scheduleFatalExit(kind, error) {
  if (fatalExitScheduled) {
    process.exitCode = 1;
    return;
  }
  fatalExitScheduled = true;
  sendFatalExecResultSync(kind, error);

  try {
    fs.writeSync(
      process.stderr.fd,
      `js_repl kernel ${kind}: ${formatErrorMessage(error)}\n`,
    );
  } catch {
    // ignore
  }

  // The host will observe stdout EOF, reset kernel state, and restart on demand.
  setImmediate(() => {
    process.exit(1);
  });
}

function formatLog(args) {
  return args
    .map((arg) =>
      typeof arg === "string" ? arg : inspect(arg, { depth: 4, colors: false }),
    )
    .join(" ");
}

function withCapturedConsole(ctx, fn) {
  const logs = [];
  const original = ctx.console ?? console;
  const captured = {
    ...original,
    log: (...args) => {
      logs.push(formatLog(args));
    },
    info: (...args) => {
      logs.push(formatLog(args));
    },
    warn: (...args) => {
      logs.push(formatLog(args));
    },
    error: (...args) => {
      logs.push(formatLog(args));
    },
    debug: (...args) => {
      logs.push(formatLog(args));
    },
  };
  ctx.console = captured;
  return fn(logs).finally(() => {
    ctx.console = original;
  });
}

function isPlainObject(value) {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function toByteArray(value) {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  return null;
}

function encodeByteImage(bytes, mimeType, detail) {
  if (bytes.byteLength === 0) {
    throw new Error("codex.emitImage expected non-empty bytes");
  }
  if (typeof mimeType !== "string" || !mimeType) {
    throw new Error("codex.emitImage expected a non-empty mimeType");
  }
  const image_url = `data:${mimeType};base64,${Buffer.from(bytes).toString("base64")}`;
  return { image_url, detail };
}

function parseImageDetail(detail) {
  if (typeof detail === "undefined") {
    return undefined;
  }
  if (typeof detail !== "string" || !detail) {
    throw new Error("codex.emitImage expected detail to be a non-empty string");
  }
  if (
    detail !== "auto" &&
    detail !== "low" &&
    detail !== "high" &&
    detail !== "original"
  ) {
    throw new Error(
      'codex.emitImage expected detail to be one of "auto", "low", "high", or "original"',
    );
  }
  return detail;
}

function parseInputImageItem(value) {
  if (!isPlainObject(value) || value.type !== "input_image") {
    return null;
  }
  if (typeof value.image_url !== "string" || !value.image_url) {
    throw new Error("codex.emitImage expected a non-empty image_url");
  }
  return {
    images: [{ image_url: value.image_url, detail: parseImageDetail(value.detail) }],
    textCount: 0,
  };
}

function parseContentItems(items) {
  if (!Array.isArray(items)) {
    return null;
  }

  const images = [];
  let textCount = 0;
  for (const item of items) {
    if (!isPlainObject(item) || typeof item.type !== "string") {
      throw new Error("codex.emitImage received malformed content items");
    }
    if (item.type === "input_image") {
      if (typeof item.image_url !== "string" || !item.image_url) {
        throw new Error("codex.emitImage expected a non-empty image_url");
      }
      images.push({
        image_url: item.image_url,
        detail: parseImageDetail(item.detail),
      });
      continue;
    }
    if (item.type === "input_text" || item.type === "output_text") {
      textCount += 1;
      continue;
    }
    throw new Error(
      `codex.emitImage does not support content item type "${item.type}"`,
    );
  }

  return { images, textCount };
}

function parseByteImageValue(value) {
  if (!isPlainObject(value) || !("bytes" in value)) {
    return null;
  }
  const bytes = toByteArray(value.bytes);
  if (!bytes) {
    throw new Error(
      "codex.emitImage expected bytes to be Buffer, Uint8Array, ArrayBuffer, or ArrayBufferView",
    );
  }
  const detail = parseImageDetail(value.detail);
  return encodeByteImage(bytes, value.mimeType, detail);
}

function parseToolOutput(output) {
  if (typeof output === "string") {
    return {
      images: [],
      textCount: output.length > 0 ? 1 : 0,
    };
  }

  const parsedItems = parseContentItems(output);
  if (parsedItems) {
    return parsedItems;
  }

  throw new Error("codex.emitImage received an unsupported tool output shape");
}

function normalizeMcpImageData(data, mimeType) {
  if (typeof data !== "string" || !data) {
    throw new Error("codex.emitImage expected MCP image data");
  }
  if (data.startsWith("data:")) {
    return data;
  }
  const normalizedMimeType =
    typeof mimeType === "string" && mimeType ? mimeType : "application/octet-stream";
  return `data:${normalizedMimeType};base64,${data}`;
}

function parseMcpToolResult(result) {
  if (typeof result === "string") {
    return { images: [], textCount: result.length > 0 ? 1 : 0 };
  }

  if (!isPlainObject(result)) {
    throw new Error("codex.emitImage received an unsupported MCP result");
  }

  if ("Err" in result) {
    const error = result.Err;
    return { images: [], textCount: typeof error === "string" && error ? 1 : 0 };
  }

  if (!("Ok" in result)) {
    throw new Error("codex.emitImage received an unsupported MCP result");
  }

  const ok = result.Ok;
  if (!isPlainObject(ok) || !Array.isArray(ok.content)) {
    throw new Error("codex.emitImage received malformed MCP content");
  }

  const images = [];
  let textCount = 0;
  for (const item of ok.content) {
    if (!isPlainObject(item) || typeof item.type !== "string") {
      throw new Error("codex.emitImage received malformed MCP content");
    }
    if (item.type === "image") {
      images.push({
        image_url: normalizeMcpImageData(item.data, item.mimeType ?? item.mime_type),
      });
      continue;
    }
    if (item.type === "text") {
      textCount += 1;
      continue;
    }
    throw new Error(
      `codex.emitImage does not support MCP content type "${item.type}"`,
    );
  }

  return { images, textCount };
}

function requireSingleImage(parsed) {
  if (parsed.textCount > 0) {
    throw new Error("codex.emitImage does not accept mixed text and image content");
  }
  if (parsed.images.length !== 1) {
    throw new Error("codex.emitImage expected exactly one image");
  }
  return parsed.images[0];
}

function normalizeEmitImageValue(value) {
  if (typeof value === "string") {
    if (!value) {
      throw new Error("codex.emitImage expected a non-empty image URL");
    }
    return { image_url: value };
  }

  const directItem = parseInputImageItem(value);
  if (directItem) {
    return requireSingleImage(directItem);
  }

  const byteImage = parseByteImageValue(value);
  if (byteImage) {
    return byteImage;
  }

  const directItems = parseContentItems(value);
  if (directItems) {
    return requireSingleImage(directItems);
  }

  if (!isPlainObject(value)) {
    throw new Error("codex.emitImage received an unsupported value");
  }

  if (value.type === "message") {
    return requireSingleImage(parseContentItems(value.content));
  }

  if (
    value.type === "function_call_output" ||
    value.type === "custom_tool_call_output"
  ) {
    return requireSingleImage(parseToolOutput(value.output));
  }

  if (value.type === "mcp_tool_call_output") {
    return requireSingleImage(parseMcpToolResult(value.result));
  }

  if ("output" in value) {
    return requireSingleImage(parseToolOutput(value.output));
  }

  if ("content" in value) {
    return requireSingleImage(parseContentItems(value.content));
  }

  throw new Error("codex.emitImage received an unsupported value");
}

async function handleExec(message) {
  activeExecId = message.id;
  const pendingBackgroundTasks = new Set();
  const tool = (toolName, args) => {
    if (typeof toolName !== "string" || !toolName) {
      return Promise.reject(new Error("codex.tool expects a tool name string"));
    }
    const id = `${message.id}-tool-${toolCounter++}`;
    let argumentsJson = "{}";
    if (typeof args === "string") {
      argumentsJson = args;
    } else if (typeof args !== "undefined") {
      argumentsJson = JSON.stringify(args);
    }

    return new Promise((resolve, reject) => {
      const payload = {
        type: "run_tool",
        id,
        exec_id: message.id,
        tool_name: toolName,
        arguments: argumentsJson,
      };
      send(payload);
      pendingTool.set(id, (res) => {
        if (!res.ok) {
          reject(new Error(res.error || "tool failed"));
          return;
        }
        resolve(res.response);
      });
    });
  };
  const emitImage = (imageLike) => {
    const operation = (async () => {
      const normalized = normalizeEmitImageValue(await imageLike);
      const id = `${message.id}-emit-image-${emitImageCounter++}`;
      const payload = {
        type: "emit_image",
        id,
        exec_id: message.id,
        image_url: normalized.image_url,
        detail: normalized.detail ?? null,
      };
      send(payload);
      return new Promise((resolve, reject) => {
        pendingEmitImage.set(id, (res) => {
          if (!res.ok) {
            reject(new Error(res.error || "emitImage failed"));
            return;
          }
          resolve();
        });
      });
    })();

    const observation = { observed: false };
    const trackedOperation = operation.then(
      () => ({ ok: true, error: null, observation }),
      (error) => ({ ok: false, error, observation }),
    );
    pendingBackgroundTasks.add(trackedOperation);
    return {
      then(onFulfilled, onRejected) {
        observation.observed = true;
        return operation.then(onFulfilled, onRejected);
      },
      catch(onRejected) {
        observation.observed = true;
        return operation.catch(onRejected);
      },
      finally(onFinally) {
        observation.observed = true;
        return operation.finally(onFinally);
      },
    };
  };

  try {
    const code = typeof message.code === "string" ? message.code : "";
    const { source, nextBindings } = await buildModuleSource(code);
    let output = "";

    context.codex = { tmpDir, tool, emitImage };
    context.tmpDir = tmpDir;

    await withCapturedConsole(context, async (logs) => {
      const module = new SourceTextModule(source, {
        context,
        identifier: `cell-${cellCounter++}.mjs`,
        initializeImportMeta(meta, mod) {
          meta.url = `file://${mod.identifier}`;
        },
        importModuleDynamically(specifier) {
          return importResolved(resolveSpecifier(specifier));
        },
      });

      await module.link(async (specifier) => {
        if (specifier === "@prev" && previousModule) {
          const exportNames = previousBindings.map((b) => b.name);
          // Build a synthetic module snapshot of the prior cell's exports.
          // This is the bridge that carries values from cell N to cell N+1.
          const synthetic = new SyntheticModule(
            exportNames,
            function initSynthetic() {
              for (const binding of previousBindings) {
                this.setExport(
                  binding.name,
                  previousModule.namespace[binding.name],
                );
              }
            },
            { context },
          );
          return synthetic;
        }

        const resolved = resolveSpecifier(specifier);
        return importResolved(resolved);
      });

      await module.evaluate();
      if (pendingBackgroundTasks.size > 0) {
        const backgroundResults = await Promise.all([...pendingBackgroundTasks]);
        const firstUnhandledBackgroundError = backgroundResults.find(
          (result) => !result.ok && !result.observation.observed,
        );
        if (firstUnhandledBackgroundError) {
          throw firstUnhandledBackgroundError.error;
        }
      }
      previousModule = module;
      previousBindings = nextBindings;
      output = logs.join("\n");
    });

    send({
      type: "exec_result",
      id: message.id,
      ok: true,
      output,
      error: null,
    });
  } catch (error) {
    send({
      type: "exec_result",
      id: message.id,
      ok: false,
      output: "",
      error: error && error.message ? error.message : String(error),
    });
  } finally {
    if (activeExecId === message.id) {
      activeExecId = null;
    }
  }
}

function handleToolResult(message) {
  const resolver = pendingTool.get(message.id);
  if (resolver) {
    pendingTool.delete(message.id);
    resolver(message);
  }
}

function handleEmitImageResult(message) {
  const resolver = pendingEmitImage.get(message.id);
  if (resolver) {
    pendingEmitImage.delete(message.id);
    resolver(message);
  }
}

let queue = Promise.resolve();

process.on("uncaughtException", (error) => {
  scheduleFatalExit("uncaught exception", error);
});

process.on("unhandledRejection", (reason) => {
  scheduleFatalExit("unhandled rejection", reason);
});

const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
input.on("line", (line) => {
  if (!line.trim()) {
    return;
  }

  let message;
  try {
    message = JSON.parse(line);
  } catch {
    return;
  }

  if (message.type === "exec") {
    queue = queue.then(() => handleExec(message));
    return;
  }
  if (message.type === "run_tool_result") {
    handleToolResult(message);
    return;
  }
  if (message.type === "emit_image_result") {
    handleEmitImageResult(message);
  }
});
