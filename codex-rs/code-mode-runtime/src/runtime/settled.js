// Runtime-owned promise combinators; all state stays in the cell's V8 heap.
// Observe each input once and queue settlements while the consumer is busy.
// Repeated races can lose that order and attach extra reactions to pending inputs.
(() => {
  function as_settled(promises) {
    let ready = [];
    let pending = 0;
    let wake;
    let closed = false;

    function settle(result) {
      pending--;
      if (closed) return;
      ready.push(result);
      if (wake) {
        const resolve = wake;
        wake = undefined;
        resolve();
      }
    }

    const keyed = promises instanceof Map;
    let position = 0;
    try {
      for (const entry of promises) {
        const [index, promise] = keyed ? entry : [position++, entry];
        pending++;
        Promise.resolve(promise).then(
          (value) => settle({ index, status: "fulfilled", value }),
          (reason) => settle({ index, status: "rejected", reason }),
        );
      }
    } catch (error) {
      closed = true;
      throw error;
    }

    return (async function* () {
      try {
        while (pending > 0 || ready.length > 0) {
          if (ready.length === 0) {
            await new Promise((resolve) => {
              wake = resolve;
            });
          }
          const batch = ready;
          ready = [];
          yield* batch;
        }
      } finally {
        closed = true;
        ready = [];
      }
    })();
  }

  async function stream_settled(promises, emit) {
    if (typeof emit !== "function") {
      throw new TypeError("stream_settled requires a callback");
    }
    for await (const result of as_settled(promises)) {
      await emit(result);
    }
  }

  Object.assign(globalThis, { as_settled, stream_settled });
})();
