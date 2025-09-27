import { describe, expect, test } from 'vitest';
import { execCodex, parseExecEvents, runExec } from '@openai/codex';
import { PassThrough } from 'node:stream';
import http from 'node:http';
import os from 'node:os';

describe('execCodex', () => {
  test('invokes the configured binary with args and returns exit code', async () => {
    const result = await execCodex(['-e', 'process.exit(0)'], {
      binaryPath: process.execPath,
      stdio: 'pipe',
    });
    expect(result.type).toBe('code');
    expect(result.exitCode).toBe(0);
  });

  test('parses newline-delimited JSON conversation events', async () => {
    const stream = new PassThrough();
    const events = [
      { type: 'session.created', session_id: 'abc' },
      {
        type: 'item.completed',
        item: {
          id: 'item_1',
          item_type: 'assistant_message',
          text: 'hello',
        },
      },
    ];

    const iter = parseExecEvents(stream);
    // Write two JSONL lines, plus some whitespace noise
    for (const ev of events) {
      stream.write(JSON.stringify(ev) + '\n');
    }
    stream.write('\n  \n');
    stream.end();

    const out: unknown[] = [];
    for await (const ev of iter) out.push(ev);
    expect(out).toEqual(events);
  });

  test('runExec streams events and respects baseUrl via OPENAI_BASE_URL', async () => {
    // Start a mock responses server
    let received = 0;
    const server = http.createServer((req, res) => {
      if (req.method === 'POST' && req.url?.endsWith('/v1/responses')) {
        received++;
        res.statusCode = 200;
        res.setHeader('content-type', 'text/event-stream');
        res.end('event: completed\n' + 'data: {"type":"response.completed"}\n\n');
        return;
      }
      res.statusCode = 404;
      res.end();
    });
    await new Promise<void>((resolve) => server.listen(0, resolve));
    const addr = server.address();
    if (!addr || typeof addr !== 'object') throw new Error('server address');
    const baseUrl = `http://127.0.0.1:${addr.port}/v1`;

    // If CODEX_BIN is provided, run the real codex binary with --experimental-json.
    const codexBin = process.env.CODEX_BIN;
    if (!codexBin) {
      // Skip if the binary isn't available (e.g., local dev without Rust build).
      return;
    }
    const { events, done } = runExec([
      '--skip-git-repo-check',
      '-s',
      'danger-full-access',
      'hello world',
    ], { binaryPath: codexBin, baseUrl, env: { OPENAI_API_KEY: 'dummy', CODEX_HOME: os.tmpdir() } });

    const collected: any[] = [];
    for await (const ev of events) collected.push(ev);
    const result = await done;

    expect(result.type).toBe('code');
    expect(result.exitCode).toBe(0);
    expect(received).toBe(1);
    // Just assert we received at least one event and the server saw a request.
    expect(collected.length).toBeGreaterThan(0);
    expect(received).toBe(1);

    await new Promise<void>((resolve, reject) => server.close((err) => (err ? reject(err) : resolve())));
  });
});
