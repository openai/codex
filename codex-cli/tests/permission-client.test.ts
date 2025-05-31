import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { ReviewDecision } from "../src/utils/agent/review";

/* ---------------------------------------------------------------- *\
   ──── HELPER: minimal in-memory socket stub ───────────────────────
\* ---------------------------------------------------------------- */

type Handler = (data?: unknown) => void;

/**
 * Creates a fresh mock socket that is “close enough” to what
 * socket.io-client gives us for the purposes of these tests.
 */
function createMockSocket(connected = true) {
  const handlers: Record<string, Array<Handler>> = {};

  const sock = {
    /* state ------------------------------------------------------- */
    connected,

    /* event registration ----------------------------------------- */
    on: (event: string, cb: Handler) => {
      (handlers[event] ||= []).push(cb);
      return sock;
    },
    once: (event: string, cb: Handler) => {
      const wrapper: Handler = (d) => {
        cb(d);
        handlers[event] = handlers[event]!.filter((h) => h !== wrapper);
      };
      return sock.on(event, wrapper);
    },

    /* outbound ---------------------------------------------------- */
    emit: vi.fn(),

    /* test-only helpers ------------------------------------------ */
    _fire(event: string, data?: unknown) {
      handlers[event]?.forEach((h) => h(data));
    },
    _connect() {
      sock.connected = true;
      sock._fire("connect");
    },
  };

  return sock;
}

const flushMicrotasks = () => Promise.resolve();

/* ---------------------------------------------------------------- *\
   ──── MODULE Mocks (socket.io-client & ReviewDecision enum) ───────
\* ---------------------------------------------------------------- */
let socketInstance: ReturnType<typeof createMockSocket>;
let ioMock: ReturnType<typeof vi.fn>;

vi.mock("socket.io-client", () => {
  ioMock = vi.fn(() => socketInstance);
  return { io: ioMock };
});

/* ---------------------------------------------------------------- *\
   ──── Tests ───────────────────────────────────────────────────────
\* ---------------------------------------------------------------- */

describe("requestRemotePermission", () => {
  // Module under test – imported *after* the mocks.
  // eslint-disable-next-line @typescript-eslint/consistent-type-imports
  let requestRemotePermission: typeof import("../src/permission-client").requestRemotePermission;
  beforeEach(async () => {
    // Fresh socket for every test so listeners don’t bleed over.
    socketInstance = createMockSocket(true);
    vi.resetModules(); // make the SUT pick up the fresh mocks

    ({ requestRemotePermission } = await import("../src/permission-client"));
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("sends the correct payload and resolves on a valid server response", async () => {
    const promise = requestRemotePermission(
      "agent-1",
      "http://perm-server",
      "Let me do the thing",
    );
    await flushMicrotasks();

    // grab the payload we just emitted…
    expect(socketInstance.emit).toHaveBeenCalledWith("permission_request", {
      agentId: "agent-1",
      message: "Let me do the thing",
    });

    // Simulate the server telling us ReviewDecision.YES,”
    socketInstance._fire("permission_response", {
      type: "permission-response",
      agentId: "agent-1",
      decision: ReviewDecision.YES,
      customDenyMessage: "",
    });

    await expect(promise).resolves.toEqual({
      decision: ReviewDecision.YES,
      customDenyMessage: "",
    });
  });

  it("waits for the socket to connect when it isn’t yet connected", async () => {
    // Start disconnected → requestRemotePermission must wait for "connect"
    socketInstance = createMockSocket(false);

    const { requestRemotePermission: coldReq } = await import(
      "../src/permission-client"
    );

    const p = coldReq("agent-2", "http://server", "hi");

    await flushMicrotasks();

    // Nothing should be emitted until we’re connected
    expect(socketInstance.emit).not.toHaveBeenCalled();

    // Simulate transport connection
    socketInstance._connect();

    await flushMicrotasks();

    // Now emit + respond
    expect(socketInstance.emit).toHaveBeenCalled();

    socketInstance._fire("permission_response", {
      type: "permission-response",
      agentId: "agent-2",
      decision: ReviewDecision.YES,
      customDenyMessage: "",
    });

    await expect(p).resolves.toMatchObject({ decision: ReviewDecision.YES });
  });

  it("rejects if a second request is made while one is already pending", async () => {
    const first = requestRemotePermission("agent-1", "url", "prompt 1");
    await flushMicrotasks();

    await expect(
      requestRemotePermission("agent-2", "url", "prompt 2"),
    ).rejects.toThrow(/already active/i);

    // Clean-up: finish the first request so leak checking in Vitest passes
    socketInstance._fire("permission_response", {
      type: "permission-response",
      agentId: "agent-1",
      decision: ReviewDecision.YES,
      customDenyMessage: "",
    });
    await first;
  });

  it("rejects when the server returns an unknown decision value", async () => {
    const p = requestRemotePermission("agent-x", "url", "prompt");

    // Wait until requestRemotePermission has passed the `await waitUntilConnected(...)`
    // and actually set activeRequest:
    await flushMicrotasks();

    socketInstance._fire("permission_response", {
      type: "permission-response",
      agentId: "agent-x",
      decision: "totally-unexpected",
      customDenyMessage: "",
    });

    await expect(p).rejects.toThrow(/Unexpected decision value/i);
  });

  it("propagates an error when socket.emit throws", async () => {
    socketInstance.emit.mockImplementation(() => {
      throw new Error("socket down");
    });

    await expect(
      requestRemotePermission("agent-1", "url", "prompt"),
    ).rejects.toThrow(/socket down/);
  });

  it("re-uses the same socket instance across multiple calls", async () => {
    const p1 = requestRemotePermission("a1", "url", "p1");

    // wait one microtask so that `activeRequest` is set before we fire the response:
    await flushMicrotasks();

    socketInstance._fire("permission_response", {
      type: "permission-response",
      agentId: "a1",
      decision: ReviewDecision.YES,
      customDenyMessage: "",
    });
    await p1;

    // Second call should reuse the same socket instance (i.e. ioMock called only once)
    const p2 = requestRemotePermission("a2", "url", "p2");
    expect(ioMock).toHaveBeenCalledTimes(1);

    // again wait one microtask so that activeRequest is set for the second request
    await flushMicrotasks();

    socketInstance._fire("permission_response", {
      type: "permission-response",
      agentId: "a2",
      decision: ReviewDecision.YES,
      customDenyMessage: "",
    });
    await p2;
  });
});
