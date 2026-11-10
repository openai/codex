(() => {
  const emitBlock = globalThis.content;
  const iteratorKey = Symbol.iterator;
  const apply = Reflect.apply;

  globalThis.content = function content(value) {
    const iterate =
      typeof value === "string" ? undefined : value?.[iteratorKey];
    if (typeof iterate !== "function") {
      return emitBlock(value);
    }

    // Reuse the method from a potentially stateful getter, with its original
    // receiver. The for-of loop closes the iterator if the host rejects a block.
    const blocks = { [iteratorKey]: () => apply(iterate, value, []) };
    for (const block of blocks) {
      emitBlock(block);
    }
  };
})();
