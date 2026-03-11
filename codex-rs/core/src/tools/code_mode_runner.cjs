'use strict';

const readline = require('node:readline');
const vm = require('node:vm');

const { SourceTextModule, SyntheticModule } = vm;
const DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL = 10000;

function normalizeMaxOutputTokensPerExecCall(value) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError('max_output_tokens_per_exec_call must be a non-negative safe integer');
  }
  return value;
}

function createProtocol() {
  const rl = readline.createInterface({
    input: process.stdin,
    crlfDelay: Infinity,
  });

  let nextId = 0;
  const pending = new Map();
  let initResolve;
  let initReject;
  const init = new Promise((resolve, reject) => {
    initResolve = resolve;
    initReject = reject;
  });

  rl.on('line', (line) => {
    if (!line.trim()) {
      return;
    }

    let message;
    try {
      message = JSON.parse(line);
    } catch (error) {
      initReject(error);
      return;
    }

    if (message.type === 'init') {
      initResolve(message);
      return;
    }

    if (message.type === 'response') {
      const entry = pending.get(message.id);
      if (!entry) {
        return;
      }
      pending.delete(message.id);
      entry.resolve(message.code_mode_result ?? '');
      return;
    }

    initReject(new Error(`Unknown protocol message type: ${message.type}`));
  });

  rl.on('close', () => {
    const error = new Error('stdin closed');
    initReject(error);
    for (const entry of pending.values()) {
      entry.reject(error);
    }
    pending.clear();
  });

  function send(message) {
    return new Promise((resolve, reject) => {
      process.stdout.write(`${JSON.stringify(message)}\n`, (error) => {
        if (error) {
          reject(error);
        } else {
          resolve();
        }
      });
    });
  }

  function request(type, payload) {
    const id = `msg-${++nextId}`;
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      void send({ type, id, ...payload }).catch((error) => {
        pending.delete(id);
        reject(error);
      });
    });
  }

  return { init, request, send };
}

function readContentItems(context) {
  try {
    const serialized = vm.runInContext('JSON.stringify(globalThis.__codexContentItems ?? [])', context);
    const contentItems = JSON.parse(serialized);
    return Array.isArray(contentItems) ? contentItems : [];
  } catch {
    return [];
  }
}

function formatErrorText(error) {
  return String(error && error.stack ? error.stack : error);
}

function cloneJsonValue(value) {
  return JSON.parse(JSON.stringify(value));
}

function createToolCaller(protocol) {
  return (name, input) =>
    protocol.request('tool_call', {
      name: String(name),
      input,
    });
}

function createToolsNamespace(callTool, enabledTools) {
  const tools = Object.create(null);

  for (const { tool_name } of enabledTools) {
    Object.defineProperty(tools, tool_name, {
      value: async (args) => callTool(tool_name, args),
      configurable: false,
      enumerable: true,
      writable: false,
    });
  }

  return Object.freeze(tools);
}

function createAllToolsMetadata(enabledTools) {
  return Object.freeze(
    enabledTools.map(({ module: modulePath, name, description }) =>
      Object.freeze({
        module: modulePath,
        name,
        description,
      })
    )
  );
}

function createToolsModule(context, callTool, enabledTools) {
  const tools = createToolsNamespace(callTool, enabledTools);
  const allTools = createAllToolsMetadata(enabledTools);
  const exportNames = ['ALL_TOOLS'];

  for (const { tool_name } of enabledTools) {
    if (tool_name !== 'ALL_TOOLS') {
      exportNames.push(tool_name);
    }
  }

  const uniqueExportNames = [...new Set(exportNames)];

  return new SyntheticModule(
    uniqueExportNames,
    function initToolsModule() {
      this.setExport('ALL_TOOLS', allTools);
      for (const exportName of uniqueExportNames) {
        if (exportName !== 'ALL_TOOLS') {
          this.setExport(exportName, tools[exportName]);
        }
      }
    },
    { context }
  );
}

function ensureContentItems(context) {
  if (!Array.isArray(context.__codexContentItems)) {
    context.__codexContentItems = [];
  }
  return context.__codexContentItems;
}

function serializeOutputText(value) {
  if (typeof value === 'string') {
    return value;
  }
  if (
    typeof value === 'undefined' ||
    value === null ||
    typeof value === 'boolean' ||
    typeof value === 'number' ||
    typeof value === 'bigint'
  ) {
    return String(value);
  }

  const serialized = JSON.stringify(value);
  if (typeof serialized === 'string') {
    return serialized;
  }

  return String(value);
}

function normalizeOutputImageUrl(value) {
  if (typeof value !== 'string' || !value) {
    throw new TypeError('output_image expects a non-empty image URL string');
  }
  if (/^(?:https?:\/\/|data:)/i.test(value)) {
    return value;
  }
  throw new TypeError('output_image expects an http(s) or data URL');
}

function createCodeModeModule(context, state) {
  const load = (key) => {
    if (typeof key !== 'string') {
      throw new TypeError('load key must be a string');
    }
    if (!Object.prototype.hasOwnProperty.call(state.storedValues, key)) {
      return undefined;
    }
    return cloneJsonValue(state.storedValues[key]);
  };
  const store = (key, value) => {
    if (typeof key !== 'string') {
      throw new TypeError('store key must be a string');
    }
    state.storedValues[key] = cloneJsonValue(value);
  };
  const outputText = (value) => {
    const item = {
      type: 'input_text',
      text: serializeOutputText(value),
    };
    ensureContentItems(context).push(item);
    return item;
  };
  const outputImage = (value) => {
    const item = {
      type: 'input_image',
      image_url: normalizeOutputImageUrl(value),
    };
    ensureContentItems(context).push(item);
    return item;
  };

  return new SyntheticModule(
    ['load', 'output_text', 'output_image', 'set_max_output_tokens_per_exec_call', 'store'],
    function initCodeModeModule() {
      this.setExport('load', load);
      this.setExport('output_text', outputText);
      this.setExport('output_image', outputImage);
      this.setExport('set_max_output_tokens_per_exec_call', (value) => {
        const normalized = normalizeMaxOutputTokensPerExecCall(value);
        state.maxOutputTokensPerExecCall = normalized;
        return normalized;
      });
      this.setExport('store', store);
    },
    { context }
  );
}

function namespacesMatch(left, right) {
  if (left.length !== right.length) {
    return false;
  }
  return left.every((segment, index) => segment === right[index]);
}

function createNamespacedToolsNamespace(callTool, enabledTools, namespace) {
  const tools = Object.create(null);

  for (const tool of enabledTools) {
    const toolNamespace = Array.isArray(tool.namespace) ? tool.namespace : [];
    if (!namespacesMatch(toolNamespace, namespace)) {
      continue;
    }

    Object.defineProperty(tools, tool.name, {
      value: async (args) => callTool(tool.tool_name, args),
      configurable: false,
      enumerable: true,
      writable: false,
    });
  }

  return Object.freeze(tools);
}

function createNamespacedToolsModule(context, callTool, enabledTools, namespace) {
  const tools = createNamespacedToolsNamespace(callTool, enabledTools, namespace);
  const exportNames = [];

  for (const exportName of Object.keys(tools)) {
    if (exportName !== 'ALL_TOOLS') {
      exportNames.push(exportName);
    }
  }

  const uniqueExportNames = [...new Set(exportNames)];

  return new SyntheticModule(
    uniqueExportNames,
    function initNamespacedToolsModule() {
      for (const exportName of uniqueExportNames) {
        this.setExport(exportName, tools[exportName]);
      }
    },
    { context }
  );
}

function createModuleResolver(context, callTool, enabledTools, state) {
  const toolsModule = createToolsModule(context, callTool, enabledTools);
  const codeModeModule = createCodeModeModule(context, state);
  const namespacedModules = new Map();

  return function resolveModule(specifier) {
    if (specifier === 'tools.js') {
      return toolsModule;
    }
    if (specifier === '@openai/code_mode' || specifier === 'openai/code_mode') {
      return codeModeModule;
    }
    const namespacedMatch = /^tools\/(.+)\.js$/.exec(specifier);
    if (!namespacedMatch) {
      throw new Error(`Unsupported import in exec: ${specifier}`);
    }

    const namespace = namespacedMatch[1]
      .split('/')
      .filter((segment) => segment.length > 0);
    if (namespace.length === 0) {
      throw new Error(`Unsupported import in exec: ${specifier}`);
    }

    const cacheKey = namespace.join('/');
    if (!namespacedModules.has(cacheKey)) {
      namespacedModules.set(
        cacheKey,
        createNamespacedToolsModule(context, callTool, enabledTools, namespace)
      );
    }
    return namespacedModules.get(cacheKey);
  };
}

async function runModule(context, request, state, callTool) {
  const resolveModule = createModuleResolver(
    context,
    callTool,
    request.enabled_tools ?? [],
    state
  );
  const mainModule = new SourceTextModule(request.source, {
    context,
    identifier: 'exec_main.mjs',
    importModuleDynamically: async (specifier) => resolveModule(specifier),
  });

  await mainModule.link(resolveModule);
  await mainModule.evaluate();
}

async function main() {
  const protocol = createProtocol();
  const request = await protocol.init;
  const state = {
    maxOutputTokensPerExecCall: DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL,
    storedValues: cloneJsonValue(request.stored_values ?? {}),
  };
  const callTool = createToolCaller(protocol);
  const context = vm.createContext({
    __codexContentItems: [],
    __codex_tool_call: callTool,
  });

  try {
    await runModule(context, request, state, callTool);
    await protocol.send({
      type: 'result',
      content_items: readContentItems(context),
      stored_values: state.storedValues,
      max_output_tokens_per_exec_call: state.maxOutputTokensPerExecCall,
    });
    process.exit(0);
  } catch (error) {
    await protocol.send({
      type: 'result',
      content_items: readContentItems(context),
      stored_values: state.storedValues,
      error_text: formatErrorText(error),
      max_output_tokens_per_exec_call: state.maxOutputTokensPerExecCall,
    });
    process.exit(1);
  }
}

void main().catch(async (error) => {
  try {
    process.stderr.write(`${formatErrorText(error)}\n`);
  } finally {
    process.exitCode = 1;
  }
});
