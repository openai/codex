const __codexEnabledTools = __CODE_MODE_ENABLED_TOOLS_PLACEHOLDER__;
const __codexEnabledToolNames = __codexEnabledTools.map((tool) => tool.global_name);
const __codexContentItems = Array.isArray(globalThis.__codexContentItems)
  ? globalThis.__codexContentItems
  : [];

function __codexCloneContentItem(item) {
  if (!item || typeof item !== 'object') {
    throw new TypeError('content item must be an object');
  }
  switch (item.type) {
    case 'input_text':
      if (typeof item.text !== 'string') {
        throw new TypeError('content item "input_text" requires a string text field');
      }
      return { type: 'input_text', text: item.text };
    case 'input_image':
      if (typeof item.image_url !== 'string') {
        throw new TypeError('content item "input_image" requires a string image_url field');
      }
      return { type: 'input_image', image_url: item.image_url };
    default:
      throw new TypeError(`unsupported content item type "${item.type}"`);
  }
}

function __codexNormalizeRawContentItems(value) {
  if (Array.isArray(value)) {
    return value.flatMap((entry) => __codexNormalizeRawContentItems(entry));
  }
  return [__codexCloneContentItem(value)];
}

function __codexNormalizeContentItems(value) {
  if (typeof value === 'string') {
    return [{ type: 'input_text', text: value }];
  }
  return __codexNormalizeRawContentItems(value);
}

Object.defineProperty(globalThis, '__codexContentItems', {
  value: __codexContentItems,
  configurable: true,
  enumerable: false,
  writable: false,
});

globalThis.codex = {
  enabledTools: Object.freeze(__codexEnabledToolNames.slice()),
};

globalThis.add_content = (value) => {
  const contentItems = __codexNormalizeContentItems(value);
  __codexContentItems.push(...contentItems);
  return contentItems;
};

globalThis.console = Object.freeze({
  log() {},
  info() {},
  warn() {},
  error() {},
  debug() {},
});

for (const tool of __codexEnabledTools) {
  if (!(tool.global_name in globalThis)) {
    Object.defineProperty(globalThis, tool.global_name, {
      value: async (args) => __codex_tool_call(tool.tool_name, args),
      configurable: true,
      enumerable: false,
      writable: false,
    });
  }
}

__CODE_MODE_USER_CODE_PLACEHOLDER__
