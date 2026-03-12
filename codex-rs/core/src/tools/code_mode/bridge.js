const __codexEnabledTools = __CODE_MODE_ENABLED_TOOLS_PLACEHOLDER__;
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

__CODE_MODE_USER_CODE_PLACEHOLDER__
