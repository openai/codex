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

function isValidIdentifier(name) {
  return /^[A-Za-z_$][0-9A-Za-z_$]*$/.test(name);
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

  for (const { name } of enabledTools) {
    Object.defineProperty(tools, name, {
      value: async (args) => callTool(name, args),
      configurable: false,
      enumerable: true,
      writable: false,
    });
  }

  return Object.freeze(tools);
}

function createToolsModule(context, callTool, enabledTools) {
  const tools = createToolsNamespace(callTool, enabledTools);
  const exportNames = ['tools'];

  for (const { name } of enabledTools) {
    if (name !== 'tools' && isValidIdentifier(name)) {
      exportNames.push(name);
    }
  }

  const uniqueExportNames = [...new Set(exportNames)];

  return new SyntheticModule(
    uniqueExportNames,
    function initToolsModule() {
      this.setExport('tools', tools);
      for (const exportName of uniqueExportNames) {
        if (exportName !== 'tools') {
          this.setExport(exportName, tools[exportName]);
        }
      }
    },
    { context }
  );
}

function createCodeModeModule(context, state) {
  return new SyntheticModule(
    ['set_max_output_tokens_per_exec_call'],
    function initCodeModeModule() {
      this.setExport('set_max_output_tokens_per_exec_call', (value) => {
        const normalized = normalizeMaxOutputTokensPerExecCall(value);
        state.maxOutputTokensPerExecCall = normalized;
        return normalized;
      });
    },
    { context }
  );
}

async function runModule(context, protocol, request, state, callTool) {
  const toolsModule = createToolsModule(context, callTool, request.enabled_tools ?? []);
  const codeModeModule = createCodeModeModule(context, state);
  const resolveModule = async (specifier) => {
    if (specifier === 'tools.js') {
      return toolsModule;
    }
    if (specifier === '@openai/code_mode') {
      return codeModeModule;
    }
    throw new Error(`Unsupported import in code_mode: ${specifier}`);
  };
  const mainModule = new SourceTextModule(request.source, {
    context,
    identifier: 'code_mode_main.mjs',
    importModuleDynamically: resolveModule,
  });

  await mainModule.link(resolveModule);
  await mainModule.evaluate();
}

async function main() {
  const protocol = createProtocol();
  const request = await protocol.init;
  const state = {
    maxOutputTokensPerExecCall: DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL,
  };
  const callTool = createToolCaller(protocol);
  const context = vm.createContext({
    __codex_tool_call: callTool,
  });

  try {
    await runModule(context, protocol, request, state, callTool);
    await protocol.send({
      type: 'result',
      content_items: readContentItems(context),
      max_output_tokens_per_exec_call: state.maxOutputTokensPerExecCall,
    });
    process.exit(0);
  } catch (error) {
    await protocol.send({
      type: 'result',
      content_items: readContentItems(context),
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
