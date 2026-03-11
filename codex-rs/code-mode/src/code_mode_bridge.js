const __codexEnabledTools = __CODE_MODE_ENABLED_TOOLS_PLACEHOLDER__;
const __codexEnabledToolNames = __codexEnabledTools.map((tool) => tool.tool_name);
const __codexAllTools = Object.freeze(
  __codexEnabledTools.map((tool) =>
    Object.freeze({
      module: tool.module,
      name: tool.name,
      description: tool.description,
    }),
  ),
);
const __codexTools = Object.create(null);
const __codexContentItems = Array.isArray(globalThis.__codexContentItems)
  ? globalThis.__codexContentItems
  : [];
const __codexStoredValues = __CODE_MODE_STORED_VALUES_PLACEHOLDER__;

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

function __codexCloneJsonValue(value) {
  return JSON.parse(JSON.stringify(value));
}

function __codexSerializeOutputText(value) {
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

function __codexNormalizeOutputImageUrl(value) {
  if (typeof value !== 'string' || !value) {
    throw new TypeError('output_image expects a non-empty image URL string');
  }
  if (/^(?:https?:\/\/|data:)/i.test(value)) {
    return value;
  }
  throw new TypeError('output_image expects an http(s) or data URL');
}

Object.defineProperty(globalThis, '__codexContentItems', {
  value: __codexContentItems,
  configurable: true,
  enumerable: false,
  writable: false,
});
Object.defineProperty(globalThis, '__codexStoredValues', {
  value: __codexStoredValues,
  configurable: true,
  enumerable: false,
  writable: false,
});
Object.defineProperty(globalThis, 'ALL_TOOLS', {
  value: __codexAllTools,
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
globalThis.__codex_output_text = (value) => {
  const item = {
    type: 'input_text',
    text: __codexSerializeOutputText(value),
  };
  __codexContentItems.push(item);
  return item;
};
globalThis.__codex_output_image = (value) => {
  const item = {
    type: 'input_image',
    image_url: __codexNormalizeOutputImageUrl(value),
  };
  __codexContentItems.push(item);
  return item;
};
globalThis.__codex_store = (key, value) => {
  if (typeof key !== 'string') {
    throw new TypeError('store key must be a string');
  }
  __codexStoredValues[key] = __codexCloneJsonValue(value);
};
globalThis.__codex_load = (key) => {
  if (typeof key !== 'string') {
    throw new TypeError('load key must be a string');
  }
  if (!Object.prototype.hasOwnProperty.call(__codexStoredValues, key)) {
    return undefined;
  }
  return __codexCloneJsonValue(__codexStoredValues[key]);
};
globalThis.__codex_set_max_output_tokens_per_exec_call = (value) => {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError('max_output_tokens_per_exec_call must be a non-negative safe integer');
  }
  return __codex_set_max_output_tokens_per_exec_call_native(value);
};

globalThis.console = Object.freeze({
  log() {},
  info() {},
  warn() {},
  error() {},
  debug() {},
});

for (const name of __codexEnabledToolNames) {
  Object.defineProperty(__codexTools, name, {
    value: async (args) => __codex_tool_call(name, args),
    configurable: false,
    enumerable: true,
    writable: false,
  });
  if (!(name in globalThis)) {
    Object.defineProperty(globalThis, name, {
      value: __codexTools[name],
      configurable: true,
      enumerable: false,
      writable: false,
    });
  }
}

Object.defineProperty(globalThis, 'tools', {
  value: Object.freeze(__codexTools),
  configurable: true,
  enumerable: false,
  writable: false,
});
