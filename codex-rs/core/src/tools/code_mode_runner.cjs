'use strict';

const readline = require('node:readline');
const vm = require('node:vm');

const { SourceTextModule, SyntheticModule } = vm;

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

function isValidIdentifier(name) {
  return /^[A-Za-z_$][0-9A-Za-z_$]*$/.test(name);
}

function createToolsNamespace(protocol, enabledTools) {
  const tools = Object.create(null);

  for (const { name } of enabledTools) {
    const callTool = async (args) =>
      protocol.request('tool_call', {
        name: String(name),
        input: args,
      });
    Object.defineProperty(tools, name, {
      value: callTool,
      configurable: false,
      enumerable: true,
      writable: false,
    });
  }

  return Object.freeze(tools);
}

function parseMcpToolName(name) {
  const parts = String(name).split('__');
  if (parts.length < 3 || parts[0] !== 'mcp') {
    return null;
  }

  const serverName = parts[1];
  const exportName = parts.slice(2).join('__');
  if (!serverName || !exportName) {
    return null;
  }

  return { serverName, exportName };
}

function createToolsModule(context, protocol, enabledTools) {
  const tools = createToolsNamespace(protocol, enabledTools);
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

function createMcpToolsNamespace(protocol, enabledTools, serverName) {
  const tools = Object.create(null);

  for (const { name } of enabledTools) {
    const parsed = parseMcpToolName(name);
    if (!parsed || parsed.serverName !== serverName) {
      continue;
    }

    const callTool = async (args) =>
      protocol.request('tool_call', {
        name: String(name),
        input: args,
      });
    Object.defineProperty(tools, parsed.exportName, {
      value: callTool,
      configurable: false,
      enumerable: true,
      writable: false,
    });
  }

  return Object.freeze(tools);
}

function createMcpToolsModule(context, protocol, enabledTools, serverName) {
  const tools = createMcpToolsNamespace(protocol, enabledTools, serverName);
  const exportNames = ['tools'];

  for (const exportName of Object.keys(tools)) {
    if (exportName !== 'tools' && isValidIdentifier(exportName)) {
      exportNames.push(exportName);
    }
  }

  const uniqueExportNames = [...new Set(exportNames)];

  return new SyntheticModule(
    uniqueExportNames,
    function initMcpToolsModule() {
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

function createModuleResolver(context, protocol, enabledTools) {
  const toolsModule = createToolsModule(context, protocol, enabledTools);
  const mcpModules = new Map();

  return function resolveModule(specifier) {
    if (specifier === 'tools.js') {
      return toolsModule;
    }

    const mcpMatch = /^tools\/mcp\/([^/]+)\.js$/.exec(specifier);
    if (!mcpMatch) {
      throw new Error(`Unsupported import in code_mode: ${specifier}`);
    }

    const serverName = mcpMatch[1];
    if (!mcpModules.has(serverName)) {
      mcpModules.set(
        serverName,
        createMcpToolsModule(context, protocol, enabledTools, serverName)
      );
    }
    return mcpModules.get(serverName);
  };
}

async function runModule(context, protocol, request) {
  const resolveModule = createModuleResolver(context, protocol, request.enabled_tools ?? []);
  const mainModule = new SourceTextModule(request.source, {
    context,
    identifier: 'code_mode_main.mjs',
    importModuleDynamically(specifier) {
      return resolveModule(specifier);
    },
  });

  await mainModule.link(async (specifier) => {
    return resolveModule(specifier);
  });
  await mainModule.evaluate();
}

async function main() {
  const protocol = createProtocol();
  const request = await protocol.init;
  const context = vm.createContext({
    __codex_tool_call: async (name, input) =>
      protocol.request('tool_call', {
        name: String(name),
        input,
      }),
  });

  try {
    await runModule(context, protocol, request);
    await protocol.send({
      type: 'result',
      content_items: readContentItems(context),
    });
    process.exit(0);
  } catch (error) {
    process.stderr.write(`${String(error && error.stack ? error.stack : error)}\n`);
    await protocol.send({
      type: 'result',
      content_items: readContentItems(context),
    });
    process.exit(1);
  }
}

void main().catch(async (error) => {
  try {
    process.stderr.write(`${String(error && error.stack ? error.stack : error)}\n`);
  } finally {
    process.exitCode = 1;
  }
});
