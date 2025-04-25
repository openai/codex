import { describe, it, expect, vi } from 'vitest';
import net from 'net';
import { connectIpc } from '../src/security/interactive';

// Helper to create a mock socket
function createMockSocket(onFirstError, onConnect) {
  const socket = new net.Socket();
  socket.setEncoding = () => {};
  // stub methods
  socket.once = function(event, listener) {
    if (event === 'connect') {
      onConnect && onConnect(listener);
    } else if (event === 'error') {
      onFirstError && onFirstError(listener);
    }
    return this;
  };
  return socket;
}

describe('connectIpc retry logic', () => {
  it('retries on ECONNREFUSED and then connects', async () => {
    let calls = 0;
    // Spy on createConnection
    vi.spyOn(net, 'createConnection').mockImplementation(({ path }) => {
      calls++;
      if (calls <= 2) {
        // first two attempts: ECONNREFUSED
        return createMockSocket(
          (errorListener) => errorListener({ code: 'ECONNREFUSED' }),
          null
        );
      }
      // third attempt: succeed
      return createMockSocket(
        null,
        (connectListener) => connectListener()
      );
    });

    const socket = await connectIpc('/tmp/fake.sock');
    expect(socket).toBeDefined();
    expect(calls).toBe(3);
  });
});