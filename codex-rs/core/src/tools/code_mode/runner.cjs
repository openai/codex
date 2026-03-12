'use strict';

const readline = require('node:readline');
const { Worker } = require('node:worker_threads');

const DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL = 10000;

function normalizeMaxOutputTokensPerExecCall(value) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError('max_output_tokens_per_exec_call must be a non-negative safe integer');
  }
  return value;
}

function normalizeYieldTime(value) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError('yield_time must be a non-negative safe integer');
  }
  return value;
}

function formatErrorText(error) {
  return String(error && error.stack ? error.stack : error);
}

function cloneJsonValue(value) {
  return JSON.parse(JSON.stringify(value));
}

function clearTimer(timer) {
  if (timer !== null) {
    clearTimeout(timer);
  }
  return null;
}

function takeContentItems(session) {
  const clonedContentItems = cloneJsonValue(session.content_items);
  session.content_items.splice(0, session.content_items.length);
  return Array.isArray(clonedContentItems) ? clonedContentItems : [];
}

function codeModeWorkerMain() {
  'use strict';

  const { parentPort, workerData } = require('node:worker_threads');
  const vm = require('node:vm');
  const { SourceTextModule, SyntheticModule } = vm;

  const DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL = 10000;

  function normalizeMaxOutputTokensPerExecCall(value) {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new TypeError('max_output_tokens_per_exec_call must be a non-negative safe integer');
    }
    return value;
  }

  function normalizeYieldTime(value) {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new TypeError('yield_time must be a non-negative safe integer');
    }
    return value;
  }

  function formatErrorText(error) {
    return String(error && error.stack ? error.stack : error);
  }

  function cloneJsonValue(value) {
    return JSON.parse(JSON.stringify(value));
  }

  function createToolCaller() {
    let nextId = 0;
    const pending = new Map();

    parentPort.on('message', (message) => {
      if (message.type === 'tool_response') {
        const entry = pending.get(message.id);
        if (!entry) {
          return;
        }
        pending.delete(message.id);
        entry.resolve(message.result ?? '');
        return;
      }

      if (message.type === 'tool_response_error') {
        const entry = pending.get(message.id);
        if (!entry) {
          return;
        }
        pending.delete(message.id);
        entry.reject(new Error(message.error_text ?? 'tool call failed'));
        return;
      }
    });

    return (name, input) => {
      const id = 'msg-' + ++nextId;
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        parentPort.postMessage({
          type: 'tool_call',
          id,
          name: String(name),
          input,
        });
      });
    };
  }

  function createContentItems() {
    const contentItems = [];
    const push = contentItems.push.bind(contentItems);
    contentItems.push = (...items) => {
      for (const item of items) {
        parentPort.postMessage({
          type: 'content_item',
          item: cloneJsonValue(item),
        });
      }
      return push(...items);
    };
    parentPort.on('message', (message) => {
      if (message.type === 'clear_content') {
        contentItems.splice(0, contentItems.length);
      }
    });
    return contentItems;
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
      [
        'load',
        'output_text',
        'output_image',
        'set_max_output_tokens_per_exec_call',
        'set_yield_time',
        'store',
        'yield_control',
      ],
      function initCodeModeModule() {
        this.setExport('load', load);
        this.setExport('output_text', outputText);
        this.setExport('output_image', outputImage);
        this.setExport('set_max_output_tokens_per_exec_call', (value) => {
          const normalized = normalizeMaxOutputTokensPerExecCall(value);
          state.maxOutputTokensPerExecCall = normalized;
          parentPort.postMessage({
            type: 'set_max_output_tokens_per_exec_call',
            value: normalized,
          });
          return normalized;
        });
        this.setExport('set_yield_time', (value) => {
          const normalized = normalizeYieldTime(value);
          parentPort.postMessage({
            type: 'set_yield_time',
            value: normalized,
          });
          return normalized;
        });
        this.setExport('store', store);
        this.setExport('yield_control', () => {
          parentPort.postMessage({ type: 'yield' });
        });
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
        throw new Error('Unsupported import in exec: ' + specifier);
      }

      const namespace = namespacedMatch[1]
        .split('/')
        .filter((segment) => segment.length > 0);
      if (namespace.length === 0) {
        throw new Error('Unsupported import in exec: ' + specifier);
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

  async function runModule(context, start, state, callTool) {
    const resolveModule = createModuleResolver(
      context,
      callTool,
      start.enabled_tools ?? [],
      state
    );
    const mainModule = new SourceTextModule(start.source, {
      context,
      identifier: 'exec_main.mjs',
      importModuleDynamically: async (specifier) => resolveModule(specifier),
    });

    await mainModule.link(resolveModule);
    await mainModule.evaluate();
  }

  async function main() {
    const start = workerData ?? {};
    const state = {
      maxOutputTokensPerExecCall: DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL,
      storedValues: cloneJsonValue(start.stored_values ?? {}),
    };
    const callTool = createToolCaller();
    const context = vm.createContext({
      __codexContentItems: createContentItems(),
      __codex_tool_call: callTool,
    });

    try {
      await runModule(context, start, state, callTool);
      parentPort.postMessage({
        type: 'result',
        stored_values: state.storedValues,
      });
    } catch (error) {
      parentPort.postMessage({
        type: 'result',
        stored_values: state.storedValues,
        error_text: formatErrorText(error),
      });
    }
  }

  void main().catch((error) => {
    parentPort.postMessage({
      type: 'result',
      stored_values: {},
      error_text: formatErrorText(error),
    });
  });
}

function createProtocol() {
  const rl = readline.createInterface({
    input: process.stdin,
    crlfDelay: Infinity,
  });

  let nextId = 0;
  const pending = new Map();
  const sessions = new Map();
  let closedResolve;
  const closed = new Promise((resolve) => {
    closedResolve = resolve;
  });

  rl.on('line', (line) => {
    if (!line.trim()) {
      return;
    }

    let message;
    try {
      message = JSON.parse(line);
    } catch (error) {
      process.stderr.write(formatErrorText(error) + '\n');
      return;
    }

    if (message.type === 'start') {
      startSession(protocol, sessions, message);
      return;
    }

    if (message.type === 'poll') {
      const session = sessions.get(message.cell_id);
      if (session) {
        session.request_id = String(message.request_id);
        if (session.pending_result) {
          void completeSession(protocol, sessions, session, session.pending_result);
        } else {
          schedulePollYield(protocol, session, normalizeYieldTime(message.yield_time_ms ?? 0));
        }
      } else {
        void protocol.send({
          type: 'result',
          request_id: message.request_id,
          content_items: [],
          stored_values: {},
          error_text: `exec cell ${message.cell_id} not found`,
          max_output_tokens_per_exec_call: DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL,
        });
      }
      return;
    }

    if (message.type === 'terminate') {
      const session = sessions.get(message.cell_id);
      if (session) {
        session.request_id = String(message.request_id);
        void terminateSession(protocol, sessions, session);
      } else {
        void protocol.send({
          type: 'result',
          request_id: message.request_id,
          content_items: [],
          stored_values: {},
          error_text: `exec cell ${message.cell_id} not found`,
          max_output_tokens_per_exec_call: DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL,
        });
      }
      return;
    }

    if (message.type === 'response') {
      const entry = pending.get(message.request_id + ':' + message.id);
      if (!entry) {
        return;
      }
      pending.delete(message.request_id + ':' + message.id);
      entry.resolve(message.code_mode_result ?? '');
      return;
    }

    process.stderr.write('Unknown protocol message type: ' + message.type + '\n');
  });

  rl.on('close', () => {
    const error = new Error('stdin closed');
    for (const entry of pending.values()) {
      entry.reject(error);
    }
    pending.clear();
    for (const session of sessions.values()) {
      session.initial_yield_timer = clearTimer(session.initial_yield_timer);
      session.poll_yield_timer = clearTimer(session.poll_yield_timer);
      void session.worker.terminate().catch(() => {});
    }
    sessions.clear();
    closedResolve();
  });

  function send(message) {
    return new Promise((resolve, reject) => {
      process.stdout.write(JSON.stringify(message) + '\n', (error) => {
        if (error) {
          reject(error);
        } else {
          resolve();
        }
      });
    });
  }

  function request(type, payload) {
    const requestId = 'req-' + ++nextId;
    const id = 'msg-' + ++nextId;
    const pendingKey = requestId + ':' + id;
    return new Promise((resolve, reject) => {
      pending.set(pendingKey, { resolve, reject });
      void send({ type, request_id: requestId, id, ...payload }).catch((error) => {
        pending.delete(pendingKey);
        reject(error);
      });
    });
  }

  const protocol = { closed, request, send };
  return protocol;
}

function sessionWorkerSource() {
  return '(' + codeModeWorkerMain.toString() + ')();';
}

function startSession(protocol, sessions, start) {
  const session = {
    completed: false,
    content_items: [],
    default_yield_time_ms: normalizeYieldTime(start.default_yield_time_ms),
    id: start.cell_id,
    initial_yield_timer: null,
    initial_yield_triggered: false,
    max_output_tokens_per_exec_call: DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL,
    pending_result: null,
    poll_yield_timer: null,
    request_id: String(start.request_id),
    worker: new Worker(sessionWorkerSource(), {
      eval: true,
      workerData: start,
    }),
  };
  sessions.set(session.id, session);
  scheduleInitialYield(protocol, session, session.default_yield_time_ms);

  session.worker.on('message', (message) => {
    void handleWorkerMessage(protocol, sessions, session, message).catch((error) => {
      void completeSession(protocol, sessions, session, {
        type: 'result',
        stored_values: {},
        error_text: formatErrorText(error),
      });
    });
  });
  session.worker.on('error', (error) => {
    void completeSession(protocol, sessions, session, {
      type: 'result',
      stored_values: {},
      error_text: formatErrorText(error),
    });
  });
  session.worker.on('exit', (code) => {
    if (code !== 0 && !session.completed) {
      void completeSession(protocol, sessions, session, {
        type: 'result',
        stored_values: {},
        error_text: 'exec worker exited with code ' + code,
      });
    }
  });
}

async function handleWorkerMessage(protocol, sessions, session, message) {
  if (session.completed) {
    return;
  }

  if (message.type === 'content_item') {
    session.content_items.push(cloneJsonValue(message.item));
    return;
  }

  if (message.type === 'set_yield_time') {
    scheduleInitialYield(protocol, session, normalizeYieldTime(message.value ?? 0));
    return;
  }

  if (message.type === 'set_max_output_tokens_per_exec_call') {
    session.max_output_tokens_per_exec_call = normalizeMaxOutputTokensPerExecCall(message.value);
    return;
  }

  if (message.type === 'yield') {
    void sendYielded(protocol, session);
    return;
  }

  if (message.type === 'tool_call') {
    void forwardToolCall(protocol, session, message);
    return;
  }

  if (message.type === 'result') {
    const result = {
      type: 'result',
      stored_values: cloneJsonValue(message.stored_values ?? {}),
      error_text:
        typeof message.error_text === 'string' ? message.error_text : undefined,
    };
    if (session.request_id === null) {
      session.pending_result = result;
      session.initial_yield_timer = clearTimer(session.initial_yield_timer);
      session.poll_yield_timer = clearTimer(session.poll_yield_timer);
      return;
    }
    await completeSession(protocol, sessions, session, result);
    return;
  }

  process.stderr.write('Unknown worker message type: ' + message.type + '\n');
}

async function forwardToolCall(protocol, session, message) {
  try {
    const result = await protocol.request('tool_call', {
      name: String(message.name),
      input: message.input,
    });
    if (session.completed) {
      return;
    }
    try {
      session.worker.postMessage({
        type: 'tool_response',
        id: message.id,
        result,
      });
    } catch {}
  } catch (error) {
    if (session.completed) {
      return;
    }
    try {
      session.worker.postMessage({
        type: 'tool_response_error',
        id: message.id,
        error_text: formatErrorText(error),
      });
    } catch {}
  }
}

async function sendYielded(protocol, session) {
  if (session.completed || session.request_id === null) {
    return;
  }
  session.initial_yield_timer = clearTimer(session.initial_yield_timer);
  session.initial_yield_triggered = true;
  session.poll_yield_timer = clearTimer(session.poll_yield_timer);
  const contentItems = takeContentItems(session);
  const requestId = session.request_id;
  try {
    session.worker.postMessage({ type: 'clear_content' });
  } catch {}
  await protocol.send({
    type: 'yielded',
    request_id: requestId,
    content_items: contentItems,
  });
  session.request_id = null;
}

function scheduleInitialYield(protocol, session, yieldTime) {
  if (session.completed || session.initial_yield_triggered) {
    return yieldTime;
  }
  session.initial_yield_timer = clearTimer(session.initial_yield_timer);
  session.initial_yield_timer = setTimeout(() => {
    session.initial_yield_timer = null;
    session.initial_yield_triggered = true;
    void sendYielded(protocol, session);
  }, yieldTime);
  return yieldTime;
}

function schedulePollYield(protocol, session, yieldTime) {
  if (session.completed) {
    return;
  }
  session.poll_yield_timer = clearTimer(session.poll_yield_timer);
  session.poll_yield_timer = setTimeout(() => {
    session.poll_yield_timer = null;
    void sendYielded(protocol, session);
  }, yieldTime);
}

async function completeSession(protocol, sessions, session, message) {
  if (session.completed) {
    return;
  }
  if (session.request_id === null) {
    session.pending_result = message;
    session.initial_yield_timer = clearTimer(session.initial_yield_timer);
    session.poll_yield_timer = clearTimer(session.poll_yield_timer);
    return;
  }
  const requestId = session.request_id;
  session.completed = true;
  session.initial_yield_timer = clearTimer(session.initial_yield_timer);
  session.poll_yield_timer = clearTimer(session.poll_yield_timer);
  sessions.delete(session.id);
  const contentItems = takeContentItems(session);
  session.pending_result = null;
  try {
    session.worker.postMessage({ type: 'clear_content' });
  } catch {}
  await protocol.send({
    ...message,
    request_id: requestId,
    content_items: contentItems,
    max_output_tokens_per_exec_call: session.max_output_tokens_per_exec_call,
  });
}

async function terminateSession(protocol, sessions, session) {
  if (session.completed) {
    return;
  }
  session.completed = true;
  session.initial_yield_timer = clearTimer(session.initial_yield_timer);
  session.poll_yield_timer = clearTimer(session.poll_yield_timer);
  sessions.delete(session.id);
  const contentItems = takeContentItems(session);
  try {
    await session.worker.terminate();
  } catch {}
  await protocol.send({
    type: 'terminated',
    request_id: session.request_id,
    content_items: contentItems,
  });
}

async function main() {
  const protocol = createProtocol();
  await protocol.closed;
}

void main().catch(async (error) => {
  try {
    process.stderr.write(formatErrorText(error) + '\n');
  } finally {
    process.exitCode = 1;
  }
});
