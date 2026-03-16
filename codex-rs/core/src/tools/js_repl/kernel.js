// Node-based kernel for js_repl.
// Communicates over JSON lines on stdin/stdout.
// Requires Node started with --experimental-vm-modules.

const { Buffer } = require("node:buffer");
const { AsyncLocalStorage } = require("node:async_hooks");
const crypto = require("node:crypto");
const fs = require("node:fs");
const { builtinModules, createRequire } = require("node:module");
const { performance } = require("node:perf_hooks");
const path = require("node:path");
const { URL, URLSearchParams, fileURLToPath, pathToFileURL } = require(
  "node:url",
);
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
// - `previousModule` is the most recently committed module namespace.
// - `previousBindings` tracks which top-level names should be carried forward.
// Each new cell imports a synthetic view of the previous namespace and
// redeclares those names so user variables behave like a persistent REPL.
let previousModule = null;
/** @type {Binding[]} */
let previousBindings = [];
let cellCounter = 0;
let internalBindingCounter = 0;
const internalBindingSalt = (() => {
  const raw = process.env.CODEX_THREAD_ID ?? "";
  const sanitized = raw.replace(/[^A-Za-z0-9_$]/g, "_");
  return sanitized || "session";
})();
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
const execContextStorage = new AsyncLocalStorage();
const cwd = process.cwd();
const tmpDir = process.env.CODEX_JS_TMP_DIR || cwd;
const homeDir = process.env.HOME ?? null;
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
  if (!seen.has(cwd)) {
    bases.push(cwd);
  }
  return bases;
})();

const importResolveConditions = new Set(["node", "import"]);
const requireByBase = new Map();
const linkedFileModules = new Map();
const linkedNativeModules = new Map();
const linkedModuleEvaluations = new Map();

function clearLocalFileModuleCaches() {
  linkedFileModules.clear();
  linkedModuleEvaluations.clear();
}

function canonicalizePath(value) {
  try {
    return fs.realpathSync.native(value);
  } catch {
    return value;
  }
}

function resolveResultToUrl(resolved) {
  if (resolved.kind === "builtin") {
    return resolved.specifier;
  }
  if (resolved.kind === "file") {
    return pathToFileURL(resolved.path).href;
  }
  if (resolved.kind === "package") {
    return resolved.specifier;
  }
  throw new Error(`Unsupported module resolution kind: ${resolved.kind}`);
}

function setImportMeta(meta, mod, isMain = false) {
  meta.url = pathToFileURL(mod.identifier).href;
  meta.filename = mod.identifier;
  meta.dirname = path.dirname(mod.identifier);
  meta.main = isMain;
  meta.resolve = (specifier) =>
    resolveResultToUrl(resolveSpecifier(specifier, mod.identifier));
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

function isExplicitRelativePathSpecifier(specifier) {
  return (
    specifier.startsWith("./") ||
    specifier.startsWith("../") ||
    specifier.startsWith(".\\") ||
    specifier.startsWith("..\\")
  );
}

function isFileUrlSpecifier(specifier) {
  if (typeof specifier !== "string" || !specifier.startsWith("file:")) {
    return false;
  }
  try {
    return new URL(specifier).protocol === "file:";
  } catch {
    return false;
  }
}

function isPathSpecifier(specifier) {
  if (
    typeof specifier !== "string" ||
    !specifier ||
    specifier.trim() !== specifier
  ) {
    return false;
  }
  return (
    isExplicitRelativePathSpecifier(specifier) ||
    path.isAbsolute(specifier) ||
    isFileUrlSpecifier(specifier)
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

function resolvePathSpecifier(specifier, referrerIdentifier = null) {
  let candidate;
  if (isFileUrlSpecifier(specifier)) {
    try {
      candidate = fileURLToPath(new URL(specifier));
    } catch (err) {
      throw new Error(`Failed to resolve module "${specifier}": ${err.message}`);
    }
  } else {
    const baseDir =
      referrerIdentifier && path.isAbsolute(referrerIdentifier)
        ? path.dirname(referrerIdentifier)
        : process.cwd();
    candidate = path.isAbsolute(specifier)
      ? specifier
      : path.resolve(baseDir, specifier);
  }

  let resolvedPath;
  try {
    resolvedPath = fs.realpathSync.native(candidate);
  } catch (err) {
    if (err?.code === "ENOENT") {
      throw new Error(`Module not found: ${specifier}`);
    }
    throw new Error(`Failed to resolve module "${specifier}": ${err.message}`);
  }

  let stats;
  try {
    stats = fs.statSync(resolvedPath);
  } catch (err) {
    if (err?.code === "ENOENT") {
      throw new Error(`Module not found: ${specifier}`);
    }
    throw new Error(`Failed to inspect module "${specifier}": ${err.message}`);
  }

  if (!stats.isFile()) {
    throw new Error(
      `Unsupported import specifier "${specifier}" in js_repl. Directory imports are not supported.`,
    );
  }

  const extension = path.extname(resolvedPath).toLowerCase();
  if (extension !== ".js" && extension !== ".mjs") {
    throw new Error(
      `Unsupported import specifier "${specifier}" in js_repl. Only .js and .mjs files are supported.`,
    );
  }

  return { kind: "file", path: resolvedPath };
}

function resolveSpecifier(specifier, referrerIdentifier = null) {
  if (specifier.startsWith("node:") || builtinModuleSet.has(specifier)) {
    if (isDeniedBuiltin(specifier)) {
      throw new Error(
        `Importing module "${specifier}" is not allowed in js_repl`,
      );
    }
    return { kind: "builtin", specifier: toNodeBuiltinSpecifier(specifier) };
  }

  if (isPathSpecifier(specifier)) {
    return resolvePathSpecifier(specifier, referrerIdentifier);
  }

  if (!isBarePackageSpecifier(specifier)) {
    throw new Error(
      `Unsupported import specifier "${specifier}" in js_repl. Use a package name like "lodash" or "@scope/pkg", or a relative/absolute/file:// .js/.mjs path.`,
    );
  }

  const resolvedBare = resolveBareSpecifier(specifier);
  if (!resolvedBare) {
    throw new Error(`Module not found: ${specifier}`);
  }

  return { kind: "package", path: resolvedBare, specifier };
}

function importNativeResolved(resolved) {
  if (resolved.kind === "builtin") {
    return import(resolved.specifier);
  }
  if (resolved.kind === "package") {
    return import(pathToFileURL(resolved.path).href);
  }
  throw new Error(`Unsupported module resolution kind: ${resolved.kind}`);
}

async function loadLinkedNativeModule(resolved) {
  const key =
    resolved.kind === "builtin"
      ? `builtin:${resolved.specifier}`
      : `package:${resolved.path}`;
  let modulePromise = linkedNativeModules.get(key);
  if (!modulePromise) {
    modulePromise = (async () => {
      const namespace = await importNativeResolved(resolved);
      const exportNames = Object.getOwnPropertyNames(namespace);
      return new SyntheticModule(
        exportNames,
        function initSyntheticModule() {
          for (const name of exportNames) {
            this.setExport(name, namespace[name]);
          }
        },
        { context },
      );
    })();
    linkedNativeModules.set(key, modulePromise);
  }
  return modulePromise;
}

async function loadLinkedFileModule(modulePath) {
  let module = linkedFileModules.get(modulePath);
  if (!module) {
    const source = fs.readFileSync(modulePath, "utf8");
    module = new SourceTextModule(source, {
      context,
      identifier: modulePath,
      initializeImportMeta(meta, mod) {
        setImportMeta(meta, mod, false);
      },
      importModuleDynamically(specifier, referrer) {
        return importResolved(resolveSpecifier(specifier, referrer?.identifier));
      },
    });
    linkedFileModules.set(modulePath, module);
  }
  if (module.status === "unlinked") {
    await module.link(async (specifier, referencingModule) => {
      const resolved = resolveSpecifier(specifier, referencingModule?.identifier);
      if (resolved.kind !== "file") {
        throw new Error(
          `Static import "${specifier}" is not supported from js_repl local files. Use await import("${specifier}") instead.`,
        );
      }
      return loadLinkedFileModule(resolved.path);
    });
  }
  return module;
}

async function loadLinkedModule(resolved) {
  if (resolved.kind === "file") {
    return loadLinkedFileModule(resolved.path);
  }
  if (resolved.kind === "builtin" || resolved.kind === "package") {
    return loadLinkedNativeModule(resolved);
  }
  throw new Error(`Unsupported module resolution kind: ${resolved.kind}`);
}

async function importResolved(resolved) {
  if (resolved.kind === "file") {
    const module = await loadLinkedFileModule(resolved.path);
    let evaluation = linkedModuleEvaluations.get(resolved.path);
    if (!evaluation) {
      evaluation = module.evaluate();
      linkedModuleEvaluations.set(resolved.path, evaluation);
    }
    await evaluation;
    return module.namespace;
  }
  return importNativeResolved(resolved);
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

function collectPatternBindingNames(pattern) {
  const map = new Map();
  collectPatternNames(pattern, "binding", map);
  return Array.from(map.keys());
}

function nextInternalBindingName() {
  // We intentionally do not scan user-declared names here. Internal helpers use
  // a per-thread salt plus a counter instead. A user could still collide by
  // deliberately spelling the exact generated name, but the thread-id salt
  // keeps accidental collisions negligible while avoiding more AST bookkeeping.
  return `__codex_internal_commit_${internalBindingSalt}_${internalBindingCounter++}`;
}

function buildMarkCommittedExpression(names, markCommittedFnName) {
  const serializedNames = names.map((name) => JSON.stringify(name)).join(", ");
  return `(${markCommittedFnName}(${serializedNames}), undefined)`;
}

function tryReadBindingValue(module, bindingName) {
  if (!module) {
    return { ok: false, value: undefined };
  }

  try {
    return { ok: true, value: module.namespace[bindingName] };
  } catch {
    return { ok: false, value: undefined };
  }
}

function instrumentVariableDeclarationSource(
  code,
  declaration,
  markCommittedFnName,
) {
  if (!declaration.declarations?.length) {
    return code.slice(declaration.start, declaration.end);
  }

  const prefix = code.slice(declaration.start, declaration.declarations[0].start);
  const suffix = code.slice(
    declaration.declarations[declaration.declarations.length - 1].end,
    declaration.end,
  );
  const parts = [];

  for (const decl of declaration.declarations) {
    parts.push(code.slice(decl.start, decl.end));

    const names = collectPatternBindingNames(decl.id);
    if (names.length > 0) {
      const helperName = nextInternalBindingName();
      parts.push(
        `${helperName} = ${buildMarkCommittedExpression(names, markCommittedFnName)}`,
      );
    }
  }

  return `${prefix}${parts.join(", ")}${suffix}`;
}

function instrumentLoopBody(code, body, names, guardName, markCommittedFnName) {
  const marker = `if (${guardName}) { ${guardName} = false; ${markCommittedFnName}(${names
    .map((name) => JSON.stringify(name))
    .join(", ")}); }`;
  const bodyCode = code.slice(body.start, body.end);

  if (body.type === "BlockStatement") {
    return `{ ${marker}${bodyCode.slice(1)}`;
  }

  return `{ ${marker} ${bodyCode} }`;
}

function applyReplacements(code, replacements) {
  let instrumentedCode = code;

  for (const replacement of replacements.sort((a, b) => b.start - a.start)) {
    instrumentedCode =
      instrumentedCode.slice(0, replacement.start) +
      replacement.text +
      instrumentedCode.slice(replacement.end);
  }

  return instrumentedCode;
}

function collectHoistedVarDeclarationStarts(ast) {
  const varDeclarationStarts = new Map();

  const recordDeclarationStart = (map, name, start) => {
    const existingStart = map.get(name);
    if (existingStart === undefined || start < existingStart) {
      map.set(name, start);
    }
  };

  const recordVarDeclarationStarts = (declaration) => {
    for (const name of collectPatternBindingNames(declaration.id)) {
      recordDeclarationStart(varDeclarationStarts, name, declaration.start);
    }
  };

  for (const stmt of ast.body ?? []) {
    if (stmt.type === "VariableDeclaration" && stmt.kind === "var") {
      for (const declaration of stmt.declarations ?? []) {
        recordVarDeclarationStarts(declaration);
      }
      continue;
    }

    if (
      stmt.type === "ForStatement" &&
      stmt.init?.type === "VariableDeclaration" &&
      stmt.init.kind === "var"
    ) {
      for (const declaration of stmt.init.declarations ?? []) {
        recordVarDeclarationStarts(declaration);
      }
      continue;
    }

    if (
      (stmt.type === "ForInStatement" || stmt.type === "ForOfStatement") &&
      stmt.left?.type === "VariableDeclaration" &&
      stmt.left.kind === "var"
    ) {
      for (const declaration of stmt.left.declarations ?? []) {
        recordVarDeclarationStarts(declaration);
      }
    }
  }

  return varDeclarationStarts;
}

function collectFutureVarWriteReplacements(
  code,
  ast,
  {
    helperDeclarations = null,
    markCommittedFnName = null,
  } = {},
) {
  // Failed-cell hoisted tracking intentionally stays small here. We only mark
  // direct top-level writes to future `var` bindings, plus top-level
  // declaration-site markers handled later in `instrumentCurrentBindings`.
  // We do not recurse through nested statement structure because that quickly
  // requires real lexical-scope tracking for blocks, loop scopes, catch
  // bindings, and similar shadowing cases. Supported write recovery is limited
  // to direct top-level expression statements such as `x = 1`, `x += 1`,
  // `x++`, and logical assignments.
  const varDeclarationStarts = collectHoistedVarDeclarationStarts(ast);
  if (varDeclarationStarts.size === 0) {
    return [];
  }
  const replacements = [];
  const replacementKeys = new Set();

  if (!markCommittedFnName) {
    throw new Error(
      "collectFutureVarWriteReplacements expected a commit marker binding name",
    );
  }

  const addReplacement = (start, end, text) => {
    const key = `${start}:${end}`;
    if (!replacementKeys.has(key)) {
      replacementKeys.add(key);
      replacements.push({ start, end, text });
    }
  };

  const getFutureVarName = (identifier) => {
    if (!identifier || identifier.type !== "Identifier") {
      return null;
    }

    const declarationStart = varDeclarationStarts.get(identifier.name);
    if (
      declarationStart === undefined ||
      identifier.start >= declarationStart
    ) {
      return null;
    }

    return identifier.name;
  };

  const instrumentUpdateExpression = (node, identifier) => {
    const bindingName = getFutureVarName(identifier);
    if (!bindingName) {
      return false;
    }

    addReplacement(
      node.start,
      node.end,
      `(${markCommittedFnName}(${JSON.stringify(bindingName)}), ${code.slice(
        node.start,
        node.end,
      )})`,
    );
    return true;
  };

  const instrumentAssignmentExpression = (node) => {
    if (node.left.type !== "Identifier") {
      return false;
    }

    const bindingName = getFutureVarName(node.left);
    if (!bindingName) {
      return false;
    }

    if (
      node.operator === "&&=" ||
      node.operator === "||=" ||
      node.operator === "??="
    ) {
      if (!helperDeclarations) {
        throw new Error(
          "collectFutureVarWriteReplacements expected helperDeclarations for logical assignment rewriting",
        );
      }

      const helperName = nextInternalBindingName();
      helperDeclarations.push(`let ${helperName};`);
      const shortCircuitOperator =
        node.operator === "&&="
          ? "&&"
          : node.operator === "||="
            ? "||"
            : "??";
      addReplacement(
        node.start,
        node.end,
        `((${helperName} = ${node.left.name}), ${helperName} ${shortCircuitOperator} ((${node.left.name} = ${code.slice(node.right.start, node.right.end)}), ${buildMarkCommittedExpression([bindingName], markCommittedFnName)}, ${node.left.name}))`,
      );
      return true;
    }

    addReplacement(
      node.start,
      node.end,
      `((${code.slice(node.start, node.end)}), ${buildMarkCommittedExpression([bindingName], markCommittedFnName)}, ${node.left.name})`,
    );
    return true;
  };

  const unwrapParenthesizedExpression = (node) => {
    let current = node;
    while (current?.type === "ParenthesizedExpression") {
      current = current.expression;
    }
    return current;
  };

  for (const statement of ast.body ?? []) {
    if (statement.type !== "ExpressionStatement") {
      continue;
    }

    const expression = unwrapParenthesizedExpression(statement.expression);
    if (!expression) {
      continue;
    }

    if (
      expression.type === "UpdateExpression" &&
      expression.argument.type === "Identifier"
    ) {
      instrumentUpdateExpression(expression, expression.argument);
      continue;
    }

    if (expression.type === "AssignmentExpression") {
      instrumentAssignmentExpression(expression);
    }
  }

  return replacements;
}

function instrumentCurrentBindings(
  code,
  ast,
  currentBindings,
  priorBindings,
  markCommittedFnName,
) {
  if (currentBindings.length === 0) {
    return code;
  }

  const replacements = [];

  for (const stmt of ast.body ?? []) {
    if (stmt.type === "VariableDeclaration") {
      replacements.push({
        start: stmt.start,
        end: stmt.end,
        text: instrumentVariableDeclarationSource(
          code,
          stmt,
          markCommittedFnName,
        ),
      });
      continue;
    }

    if (stmt.type === "FunctionDeclaration" && stmt.id) {
      replacements.push({
        start: stmt.start,
        end: stmt.end,
        // Keep function source text stable for things like `foo.toString()`.
        // Pre-declaration uses are tracked separately by instrumenting the
        // top-level expressions that actually read the hoisted function value.
        text: `${code.slice(stmt.start, stmt.end)}\n;${markCommittedFnName}(${JSON.stringify(stmt.id.name)});`,
      });
      continue;
    }

    if (stmt.type === "ClassDeclaration" && stmt.id) {
      replacements.push({
        start: stmt.start,
        end: stmt.end,
        text: `${code.slice(stmt.start, stmt.end)}\n;${markCommittedFnName}(${JSON.stringify(stmt.id.name)});`,
      });
      continue;
    }

    if (
      stmt.type === "ForStatement" &&
      stmt.init &&
      stmt.init.type === "VariableDeclaration" &&
      stmt.init.kind === "var"
    ) {
      replacements.push({
        start: stmt.start,
        end: stmt.end,
        text: `${code.slice(stmt.start, stmt.init.start)}${instrumentVariableDeclarationSource(
          code,
          stmt.init,
          markCommittedFnName,
        )}${code.slice(stmt.init.end, stmt.end)}`,
      });
      continue;
    }

    if (
      (stmt.type === "ForInStatement" || stmt.type === "ForOfStatement") &&
      stmt.left &&
      stmt.left.type === "VariableDeclaration" &&
      stmt.left.kind === "var"
    ) {
      const names = stmt.left.declarations.flatMap((decl) =>
        collectPatternBindingNames(decl.id),
      );
      if (names.length > 0) {
        const guardName = nextInternalBindingName();
        replacements.push({
          start: stmt.start,
          end: stmt.end,
          // Mark top-level `for...in` / `for...of` vars on the first body
          // execution instead of every iteration. This keeps hot loops cheap
          // after the first pass while still preserving vars for the common
          // case where the loop actually ran before a later throw.
          //
          // The tradeoff is that `for (var x of []) {}` in a failed cell will
          // not carry `x` forward as `undefined`, because the body never runs
          // and the one-time marker never fires. We accept that edge case:
          // `var` is redeclarable, and the only lost state is an unassigned
          // `undefined` from an empty top-level loop in a cell that later
          // fails.
          text: `let ${guardName} = true;\n${code.slice(
            stmt.start,
            stmt.body.start,
          )}${instrumentLoopBody(
            code,
            stmt.body,
            names,
            guardName,
            markCommittedFnName,
          )}`,
        });
      }
    }
  }

  return applyReplacements(code, replacements);
}

async function buildModuleSource(code) {
  const meriyah = await meriyahPromise;
  const ast = meriyah.parseModule(code, {
    next: true,
    module: true,
    ranges: true,
    loc: false,
    disableWebCompat: true,
  });
  const currentBindings = collectBindings(ast);
  const priorBindings = previousModule ? previousBindings : [];
  const helperDeclarations = [];
  const markCommittedFnName = nextInternalBindingName();
  const markPreludeCompletedFnName = nextInternalBindingName();
  helperDeclarations.push(
    // `import.meta` is syntax-level and cannot be shadowed by user bindings
    // like `const globalThis = ...`, so alias the marker helper through it
    // once in the prelude and use that stable local binding everywhere.
    // Then delete the raw import.meta hooks so user code cannot spoof
    // committed bindings by calling them directly.
    `const ${markCommittedFnName} = import.meta.__codexInternalMarkCommittedBindings;`,
    `const ${markPreludeCompletedFnName} = import.meta.__codexInternalMarkPreludeCompleted;`,
    "delete import.meta.__codexInternalMarkCommittedBindings;",
    "delete import.meta.__codexInternalMarkPreludeCompleted;",
  );
  const writeInstrumentedCode = applyReplacements(
    code,
    collectFutureVarWriteReplacements(code, ast, {
      helperDeclarations,
      markCommittedFnName,
    }),
  );
  const instrumentedAst = meriyah.parseModule(writeInstrumentedCode, {
    next: true,
    module: true,
    ranges: true,
    loc: false,
    disableWebCompat: true,
  });
  const instrumentedCode = instrumentCurrentBindings(
    writeInstrumentedCode,
    instrumentedAst,
    currentBindings,
    priorBindings,
    markCommittedFnName,
  );

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
  if (helperDeclarations.length > 0) {
    prelude += `${helperDeclarations.join("\n")}\n`;
  }
  prelude += `${markPreludeCompletedFnName}();\n`;

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
  return {
    source: `${prelude}${instrumentedCode}${exportStmt}`,
    currentBindings,
    nextBindings,
    priorBindings,
  };
}

function canReadCommittedBinding(module, binding) {
  if (
    !module ||
    binding.kind === "var" ||
    binding.kind === "function"
  ) {
    return false;
  }

  return tryReadBindingValue(module, binding.name).ok;
}
// Failed cells keep prior bindings plus the current-cell bindings whose
// initialization definitely ran before the throw. That means:
// - lexical bindings (`const` / `let` / `class`) can fall back to namespace
//   readability, which preserves names whose initialization already completed
//   even when a later step in the same declarator throws
// - `var` / `function` bindings only persist when an explicit declaration-site
//   or write-site marker fired, so unreached hoisted bindings do not become
//   ghost bindings in later cells
function collectCommittedBindings(
  module,
  priorBindings,
  currentBindings,
  committedCurrentBindingNames,
) {
  const mergedBindings = new Map();
  let committedCurrentBindingCount = 0;

  for (const binding of priorBindings) {
    mergedBindings.set(binding.name, binding.kind);
  }

  for (const binding of currentBindings) {
    if (
      committedCurrentBindingNames.has(binding.name) ||
      canReadCommittedBinding(module, binding)
    ) {
      mergedBindings.set(binding.name, binding.kind);
      committedCurrentBindingCount += 1;
    }
  }

  return {
    bindings: Array.from(mergedBindings, ([name, kind]) => ({ name, kind })),
    committedCurrentBindingCount,
  };
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

function getCurrentExecState() {
  const execState = execContextStorage.getStore();
  if (!execState || typeof execState.id !== "string" || !execState.id) {
    throw new Error("js_repl exec context not found");
  }
  return execState;
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
  if (detail == null) {
    return undefined;
  }
  if (typeof detail !== "string" || !detail) {
    throw new Error("codex.emitImage expected detail to be a non-empty string");
  }
  if (detail !== "original") {
    throw new Error(
      'codex.emitImage only supports detail "original"; omit detail for default behavior',
    );
  }
  return detail;
}

function normalizeEmitImageUrl(value) {
  if (typeof value !== "string" || !value) {
    throw new Error("codex.emitImage expected a non-empty image_url");
  }
  if (!/^data:/i.test(value)) {
    throw new Error("codex.emitImage only accepts data URLs");
  }
  return value;
}

function parseInputImageItem(value) {
  if (!isPlainObject(value) || value.type !== "input_image") {
    return null;
  }
  return {
    images: [
      {
        image_url: normalizeEmitImageUrl(value.image_url),
        detail: parseImageDetail(value.detail),
      },
    ],
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
      images.push({
        image_url: normalizeEmitImageUrl(item.image_url),
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
  if (/^data:/i.test(data)) {
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
    return { image_url: normalizeEmitImageUrl(value) };
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

const codex = {
  cwd,
  homeDir,
  tmpDir,
  tool(toolName, args) {
    let execState;
    try {
      execState = getCurrentExecState();
    } catch (error) {
      return Promise.reject(error);
    }
    if (typeof toolName !== "string" || !toolName) {
      return Promise.reject(new Error("codex.tool expects a tool name string"));
    }
    const id = `${execState.id}-tool-${toolCounter++}`;
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
        exec_id: execState.id,
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
  },
  emitImage(imageLike) {
    let execState;
    try {
      execState = getCurrentExecState();
    } catch (error) {
      return {
        then(onFulfilled, onRejected) {
          return Promise.reject(error).then(onFulfilled, onRejected);
        },
        catch(onRejected) {
          return Promise.reject(error).catch(onRejected);
        },
        finally(onFinally) {
          return Promise.reject(error).finally(onFinally);
        },
      };
    }
    const operation = (async () => {
      const normalized = normalizeEmitImageValue(await imageLike);
      const id = `${execState.id}-emit-image-${emitImageCounter++}`;
      const payload = {
        type: "emit_image",
        id,
        exec_id: execState.id,
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
    execState.pendingBackgroundTasks.add(trackedOperation);
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
  },
};

async function handleExec(message) {
  clearLocalFileModuleCaches();
  activeExecId = message.id;
  const execState = {
    id: message.id,
    pendingBackgroundTasks: new Set(),
  };

  let module = null;
  /** @type {Binding[]} */
  let currentBindings = [];
  /** @type {Binding[]} */
  let nextBindings = [];
  /** @type {Binding[]} */
  let priorBindings = previousBindings;
  let moduleLinked = false;
  let preludeCompleted = false;
  const committedCurrentBindingNames = new Set();
  const markCommittedBindings = (...names) => {
    for (const name of names) {
      committedCurrentBindingNames.add(name);
    }
  };
  const markPreludeCompleted = () => {
    preludeCompleted = true;
  };

  try {
    const code = typeof message.code === "string" ? message.code : "";
    const builtSource = await buildModuleSource(code);
    const source = builtSource.source;
    currentBindings = builtSource.currentBindings;
    nextBindings = builtSource.nextBindings;
    priorBindings = builtSource.priorBindings;
    let output = "";

    context.codex = codex;
    context.tmpDir = tmpDir;

    await execContextStorage.run(execState, async () => {
      await withCapturedConsole(context, async (logs) => {
        const cellIdentifier = path.join(
          cwd,
          `.codex_js_repl_cell_${cellCounter++}.mjs`,
        );
        module = new SourceTextModule(source, {
          context,
          identifier: cellIdentifier,
          initializeImportMeta(meta, mod) {
            setImportMeta(meta, mod, true);
            meta.__codexInternalMarkCommittedBindings = markCommittedBindings;
            meta.__codexInternalMarkPreludeCompleted = markPreludeCompleted;
          },
          importModuleDynamically(specifier, referrer) {
            return importResolved(resolveSpecifier(specifier, referrer?.identifier));
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
          throw new Error(
            `Top-level static import "${specifier}" is not supported in js_repl. Use await import("${specifier}") instead.`,
          );
        });
        moduleLinked = true;

        await module.evaluate();
        if (execState.pendingBackgroundTasks.size > 0) {
          const backgroundResults = await Promise.all([
            ...execState.pendingBackgroundTasks,
          ]);
          const firstUnhandledBackgroundError = backgroundResults.find(
            (result) => !result.ok && !result.observation.observed,
          );
          if (firstUnhandledBackgroundError) {
            throw firstUnhandledBackgroundError.error;
          }
        }
        output = logs.join("\n");
      });
    });

    previousModule = module;
    previousBindings = nextBindings;

    send({
      type: "exec_result",
      id: message.id,
      ok: true,
      output,
      error: null,
    });
  } catch (error) {
    const { bindings: committedBindings, committedCurrentBindingCount } =
      collectCommittedBindings(
      moduleLinked ? module : null,
      priorBindings,
      currentBindings,
      committedCurrentBindingNames,
    );
    // Preserve the last successfully linked module across link-time failures.
    // A module whose link step failed cannot safely back @prev because reading
    // its namespace throws before evaluation ever begins. Likewise, if a
    // linked module failed before its prelude recreated carried bindings, keep
    // the old module so @prev still points at the last cell whose prelude and
    // body actually established the carried values. Once the prelude has run,
    // promote the failed module even if it only updated existing bindings.
    if (
      module &&
      moduleLinked &&
      (committedCurrentBindingCount > 0 ||
        (preludeCompleted && priorBindings.length > 0))
    ) {
      previousModule = module;
      previousBindings = committedBindings;
    }
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
let pendingInputSegments = [];

process.on("uncaughtException", (error) => {
  scheduleFatalExit("uncaught exception", error);
});

process.on("unhandledRejection", (reason) => {
  scheduleFatalExit("unhandled rejection", reason);
});

function handleInputLine(line) {
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
}

function takePendingInputFrame() {
  if (pendingInputSegments.length === 0) {
    return null;
  }

  // Keep raw stdin chunks queued until a full JSONL frame is ready so we only
  // assemble the frame bytes once.
  const frame =
    pendingInputSegments.length === 1
      ? pendingInputSegments[0]
      : Buffer.concat(pendingInputSegments);
  pendingInputSegments = [];
  return frame;
}

function handleInputFrame(frame) {
  if (!frame) {
    return;
  }

  if (frame[frame.length - 1] === 0x0d) {
    frame = frame.subarray(0, frame.length - 1);
  }
  handleInputLine(frame.toString("utf8"));
}

process.stdin.on("data", (chunk) => {
  const input = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
  let segmentStart = 0;
  let frameEnd = input.indexOf(0x0a);
  while (frameEnd !== -1) {
    pendingInputSegments.push(input.subarray(segmentStart, frameEnd));
    handleInputFrame(takePendingInputFrame());
    segmentStart = frameEnd + 1;
    frameEnd = input.indexOf(0x0a, segmentStart);
  }
  if (segmentStart < input.length) {
    pendingInputSegments.push(input.subarray(segmentStart));
  }
});

process.stdin.on("end", () => {
  handleInputFrame(takePendingInputFrame());
});
