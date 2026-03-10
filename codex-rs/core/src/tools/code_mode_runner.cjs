'use strict';

const readline = require('node:readline');
const vm = require('node:vm');

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
      entry.resolve(Array.isArray(message.content_items) ? message.content_items : []);
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
    const serialized = vm.runInContext(
      'JSON.stringify(globalThis.__codexContentItems ?? [])',
      context
    );
    const contentItems = JSON.parse(serialized);
    return Array.isArray(contentItems) ? contentItems : [];
  } catch {
    return [];
  }
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
    await vm.runInContext(request.source, context, {
      displayErrors: true,
      microtaskMode: 'afterEvaluate',
    });
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
