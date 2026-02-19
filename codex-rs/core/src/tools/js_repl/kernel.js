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
let toolCounter = 0;
const tmpDir = process.env.CODEX_JS_TMP_DIR || process.cwd();
// Explicit long-lived mutable store exposed as `codex.state`. This is useful
// when callers want shared state without relying on lexical binding carry-over.
const state = {};
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

async function handleExec(message) {
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

  try {
    const code = typeof message.code === "string" ? message.code : "";
    const { source, nextBindings } = await buildModuleSource(code);
    let output = "";

    context.state = state;
    context.codex = { state, tmpDir, tool };
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
  }
}

function handleToolResult(message) {
  const resolver = pendingTool.get(message.id);
  if (resolver) {
    pendingTool.delete(message.id);
    resolver(message);
  }
}

let queue = Promise.resolve();

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
  }
});
