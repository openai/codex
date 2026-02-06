// Node-based kernel for js_repl.
// Communicates over JSON lines on stdin/stdout.
// Requires Node started with --experimental-vm-modules.

const { Buffer } = require("node:buffer");
const crypto = require("node:crypto");
const { builtinModules } = require("node:module");
const { createInterface } = require("node:readline");
const { performance } = require("node:perf_hooks");
const path = require("node:path");
const { URL, URLSearchParams, pathToFileURL } = require("node:url");
const { inspect, TextDecoder, TextEncoder } = require("node:util");
const fs = require("node:fs/promises");
const vm = require("node:vm");

const { SourceTextModule, SyntheticModule } = vm;
const meriyahPromise = import("./meriyah.umd.min.js").then((m) => m.default ?? m);

const context = vm.createContext({});
context.globalThis = context;
context.global = context;
context.Buffer = Buffer;
context.console = console;
context.process = process;
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

/** @type {Map<string, (msg: any) => void>} */
const pendingShell = new Map();
let shellCounter = 0;
/** @type {Map<string, (msg: any) => void>} */
const pendingTool = new Map();
let toolCounter = 0;
const tmpDir = process.env.CODEX_JS_TMP_DIR || process.cwd();
// Explicit long-lived mutable store exposed as `codex.state`. This is useful
// when callers want shared state without relying on lexical binding carry-over.
const state = {};

function resolveSpecifier(specifier) {
  if (specifier.startsWith("node:") || builtinModuleSet.has(specifier)) {
    return { kind: "builtin", specifier };
  }

  if (specifier.startsWith("file:")) {
    return { kind: "url", url: specifier };
  }

  if (specifier.startsWith("./") || specifier.startsWith("../") || path.isAbsolute(specifier)) {
    return { kind: "path", path: path.resolve(process.cwd(), specifier) };
  }

  return { kind: "bare", specifier };
}

function importResolved(resolved) {
  if (resolved.kind === "builtin") {
    return import(resolved.specifier);
  }
  if (resolved.kind === "url") {
    return import(resolved.url);
  }
  if (resolved.kind === "path") {
    return import(pathToFileURL(resolved.path).href);
  }
  if (resolved.kind === "bare") {
    return import(resolved.specifier);
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
      if (stmt.init && stmt.init.type === "VariableDeclaration" && stmt.init.kind === "var") {
        for (const decl of stmt.init.declarations) {
          collectPatternNames(decl.id, "var", map);
        }
      }
    } else if (stmt.type === "ForInStatement" || stmt.type === "ForOfStatement") {
      if (stmt.left && stmt.left.type === "VariableDeclaration" && stmt.left.kind === "var") {
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
        const keyword = b.kind === "var" ? "var" : b.kind === "const" ? "const" : "let";
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
  const exportStmt = exportNames.length ? `\nexport { ${exportNames.join(", ")} };` : "";

  const nextBindings = Array.from(mergedBindings, ([name, kind]) => ({ name, kind }));
  return { source: `${prelude}${code}${exportStmt}`, nextBindings };
}

function send(message) {
  process.stdout.write(JSON.stringify(message));
  process.stdout.write("\n");
}

function formatLog(args) {
  return args
    .map((arg) => (typeof arg === "string" ? arg : inspect(arg, { depth: 4, colors: false })))
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

async function readBytes(input) {
  if (typeof input === "string") {
    const bytes = await fs.readFile(input);
    return new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }
  if (input instanceof ArrayBuffer) {
    return new Uint8Array(input);
  }
  if (ArrayBuffer.isView(input)) {
    return new Uint8Array(input.buffer.slice(input.byteOffset, input.byteOffset + input.byteLength));
  }
  throw new Error("emitImage accepts a file path or bytes");
}

async function handleExec(message) {
  const artifacts = [];

  const emitImage = async (input, meta) => {
    const bytes = await readBytes(input);
    artifacts.push({
      kind: "image",
      data: Buffer.from(bytes).toString("base64"),
      mime: meta?.mime,
      caption: meta?.caption,
      name: meta?.name,
    });
  };

  const sh = (command, opts = {}) => {
    if (typeof command !== "string") {
      return Promise.reject(new Error("codex.sh expects the first argument to be a string"));
    }
    const id = `${message.id}-sh-${shellCounter++}`;
    const timeoutMs =
      typeof opts?.timeout_ms === "number" && Number.isFinite(opts.timeout_ms)
        ? opts.timeout_ms
        : null;

    return new Promise((resolve, reject) => {
      const payload = {
        type: "run_shell",
        id,
        exec_id: message.id,
        command,
        cwd: opts?.cwd,
        timeout_ms: opts?.timeout_ms,
        sandbox_permissions: opts?.sandbox_permissions,
        justification: opts?.justification,
      };
      send(payload);
      let guard;
      if (timeoutMs !== null) {
        guard = setTimeout(() => {
          if (pendingShell.delete(id)) {
            reject(new Error("shell request timed out"));
          }
        }, timeoutMs + 1_000);
      }
      pendingShell.set(id, (res) => {
        if (guard) clearTimeout(guard);
        resolve(res);
      });
    }).then((res) => {
      if (!res.ok) {
        throw new Error(res.error || "shell failed");
      }
      return { stdout: res.stdout, stderr: res.stderr, exitCode: res.exit_code };
    });
  };

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
    context.codex = { state, tmpDir, sh, emitImage, tool };
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
                this.setExport(binding.name, previousModule.namespace[binding.name]);
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
      artifacts,
      error: null,
    });
  } catch (error) {
    send({
      type: "exec_result",
      id: message.id,
      ok: false,
      output: "",
      artifacts,
      error: error && error.message ? error.message : String(error),
    });
  }
}

function handleShellResult(message) {
  const resolver = pendingShell.get(message.id);
  if (resolver) {
    pendingShell.delete(message.id);
    resolver(message);
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
  if (message.type === "run_shell_result") {
    handleShellResult(message);
    return;
  }
  if (message.type === "run_tool_result") {
    handleToolResult(message);
  }
});
