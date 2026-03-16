const __codexContentItems = Array.isArray(globalThis.__codexContentItems)
  ? globalThis.__codexContentItems
  : [];
const __codexRuntime = globalThis.__codexRuntime;

delete globalThis.__codexRuntime;

Object.defineProperty(globalThis, '__codexContentItems', {
  value: __codexContentItems,
  configurable: true,
  enumerable: false,
  writable: false,
});

(() => {
  if (!__codexRuntime || typeof __codexRuntime !== 'object') {
    throw new Error('code mode runtime is unavailable');
  }

  function defineGlobal(name, value) {
    Object.defineProperty(globalThis, name, {
      value,
      configurable: true,
      enumerable: true,
      writable: false,
    });
  }

  defineGlobal('ALL_TOOLS', __codexRuntime.ALL_TOOLS);
  defineGlobal('exit', __codexRuntime.exit);
  defineGlobal('image', __codexRuntime.image);
  defineGlobal('load', __codexRuntime.load);
  defineGlobal('notify', __codexRuntime.notify);
  defineGlobal('store', __codexRuntime.store);
  defineGlobal('text', __codexRuntime.text);
  defineGlobal('tools', __codexRuntime.tools);
  defineGlobal('yield_control', __codexRuntime.yield_control);

  defineGlobal(
    'console',
    Object.freeze({
      log() {},
      info() {},
      warn() {},
      error() {},
      debug() {},
    })
  );
})();

__CODE_MODE_USER_CODE_PLACEHOLDER__
