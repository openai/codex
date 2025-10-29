#!/usr/bin/env node
// Local OpenAI proxy replicating the upstream-merge workflow behavior.
//
// Features:
// - Injects real OPENAI_API_KEY server-side; clients can use a dummy key
// - Allows only /v1/chat/completions and /v1/responses
// - Adds OpenAI-Beta: responses=v1 for Responses API
// - Sets Accept: text/event-stream by default to stabilize streaming
// - Scrubs sensitive headers from logs; logs are structured JSON
// - Reuses connections and sets generous timeouts for SSE
//
// Usage (local):
//   # 1) Export your real API key for the proxy process
//   export OPENAI_API_KEY="sk-..."
//   # 2) Start the proxy (default port 5055)
//   node scripts/openai-proxy.js
//   #    or choose a port / upstream base
//   PORT=5055 OPENAI_BASE_URL=https://api.openai.com/v1 node scripts/openai-proxy.js
//
//   # 3) Run the local Codex binary with the proxy (dummy key to client)
//   #    Build the binary first: ./build-fast.sh
//   OPENAI_API_KEY="x" OPENAI_BASE_URL="http://127.0.0.1:${PORT:-5055}/v1" \
//     ./code-rs/target/dev-fast/code llm request \
//       --developer "Say 'pong' as plain text" \
//       --message "ping" \
//       --format-type json_schema \
//       --schema-json '{"type":"object","properties":{},"additionalProperties":false}' \
//       --model gpt-4o-mini
//
//   # Or run your usual `code` subcommand (Exec/TUI) with the same env vars.

const http = require('http');
const https = require('https');
const crypto = require('crypto');
const { URL } = require('url');

const PORT = Number(process.env.PORT || 5055);
const API_KEY = process.env.OPENAI_API_KEY || '';
const UPSTREAM = new URL(process.env.OPENAI_BASE_URL || 'https://api.openai.com/v1');
const ALLOWED = ['/v1/chat/completions', '/v1/responses'];

// CI options:
//   EXIT_ON_5XX=1       -> Exit process non‑zero if a 5xx response head is seen
//   READY_FILE=path      -> Touch this file when the server is listening
//   LOG_DEST=stdout|stderr (default stdout)
// Debug toggles:
//   DISABLE_KEEPALIVE=1  -> Do not reuse upstream connections
//   FORCE_CLOSE=1         -> Send 'Connection: close' to upstream
//   NO_GZIP=1             -> Set 'accept-encoding: identity'
//   STRIP_SESSION_ID=1    -> Remove 'session_id' from upstream request headers
//   IDEMPOTENCY_KEY=auto  -> Add an Idempotency-Key header per request (uuid)
//   LOG_ERROR_BODY=1      -> Log non-2xx response body (truncated)
//   LOG_ERROR_BODY_BYTES=1024 -> Max bytes to log from error body
//   STRICT_HEADERS=1      -> Rebuild upstream headers from a minimal allowlist
//   RESPONSES_BETA="responses=v1" (override beta header)
const EXIT_ON_5XX = process.env.EXIT_ON_5XX === '1' || false;
const READY_FILE = process.env.READY_FILE || '';
const LOG_DEST = (process.env.LOG_DEST || 'stdout').toLowerCase();
const DISABLE_KEEPALIVE = process.env.DISABLE_KEEPALIVE === '1' || false;
const FORCE_CLOSE = process.env.FORCE_CLOSE === '1' || false;
const NO_GZIP = process.env.NO_GZIP === '1' || false;
const STRIP_SESSION_ID = process.env.STRIP_SESSION_ID === '1' || false;
const IDEMPOTENCY_KEY = (process.env.IDEMPOTENCY_KEY || '').toLowerCase();
const LOG_ERROR_BODY = process.env.LOG_ERROR_BODY === '1' || false;
const LOG_ERROR_BODY_BYTES = Number(process.env.LOG_ERROR_BODY_BYTES || 2048);
const STRICT_HEADERS = process.env.STRICT_HEADERS === '1' || false;
const RESPONSES_BETA = process.env.RESPONSES_BETA || 'responses=v1';

function outWrite(s) {
  if (LOG_DEST === 'stderr') process.stderr.write(s + '\n');
  else process.stdout.write(s + '\n');
}

if (!API_KEY) {
  console.error('[fatal] OPENAI_API_KEY missing');
  process.exit(1);
}

function redactHeaders(h) {
  const out = {};
  for (const [k, v] of Object.entries(h || {})) {
    const key = k.toLowerCase();
    if (key === 'authorization' || key === 'proxy-authorization' || key === 'cookie' || key === 'set-cookie') {
      out[key] = 'REDACTED';
    } else {
      out[key] = Array.isArray(v) ? v.map(() => '<omitted>') : String(v);
    }
  }
  return out;
}

function log(ev) {
  try { outWrite(JSON.stringify({ ts: new Date().toISOString(), ...ev })); } catch {}
}

const server = http.createServer((req, res) => {
  const rid = crypto.randomUUID();
  if (!ALLOWED.some(p => req.url.startsWith(p))) {
    res.writeHead(403, { 'content-type': 'application/json' });
    res.end(JSON.stringify({ error: 'blocked', path: req.url }));
    return;
  }

  const chunks = [];
  let size = 0;
  req.on('data', c => { chunks.push(c); size += c.length; if (size > 2 * 1024 * 1024) { req.destroy(new Error('body too large')); } });
  req.on('error', err => {
    log({ level: 'error', rid, msg: 'client request error', err: String(err) });
    if (!res.headersSent) res.writeHead(400, { 'content-type': 'application/json' });
    res.end(JSON.stringify({ error: 'bad_request' }));
  });
  req.on('end', () => {
    const body = Buffer.concat(chunks);
    const up = new URL(req.url, UPSTREAM);
    let incoming;
    if (STRICT_HEADERS) {
      incoming = {};
      incoming['authorization'] = `Bearer ${API_KEY}`;
      incoming['content-type'] = 'application/json';
      incoming['accept'] = 'text/event-stream';
      if (NO_GZIP) incoming['accept-encoding'] = 'identity';
      if (up.pathname.startsWith('/v1/responses')) incoming['openai-beta'] = RESPONSES_BETA;
      incoming['user-agent'] = 'code-proxy/1.0';
      incoming['originator'] = 'codex_cli_rs';
      if (IDEMPOTENCY_KEY === 'auto') incoming['idempotency-key'] = crypto.randomUUID();
    } else {
      incoming = { ...req.headers };
      if (!incoming['content-type']) incoming['content-type'] = 'application/json';
      if (!incoming['accept']) incoming['accept'] = 'text/event-stream';
      if (NO_GZIP) incoming['accept-encoding'] = 'identity';
      incoming['authorization'] = `Bearer ${API_KEY}`; // replace with real key
      if (up.pathname.startsWith('/v1/responses')) {
        if (!incoming['openai-beta']) incoming['openai-beta'] = RESPONSES_BETA;
      }
      if (!incoming['originator']) incoming['originator'] = 'codex_cli_rs';
      if (STRIP_SESSION_ID && 'session_id' in incoming) delete incoming['session_id'];
      if (IDEMPOTENCY_KEY === 'auto') incoming['idempotency-key'] = crypto.randomUUID();
    }
    delete incoming['host']; incoming['host'] = up.host;
    incoming['connection'] = FORCE_CLOSE ? 'close' : 'keep-alive';
    if (incoming['content-length']) incoming['content-length'] = String(body.length);
    delete incoming['proxy-connection'];
    delete incoming['proxy-authorization'];

    log({ level: 'info', rid, phase: 'request', method: req.method, url: up.toString(), headers: redactHeaders(incoming), body_bytes: body.length });

    const opts = {
      protocol: up.protocol,
      hostname: up.hostname,
      port: up.port || (up.protocol === 'https:' ? 443 : 80),
      path: up.pathname + up.search,
      method: req.method,
      headers: incoming,
      servername: up.hostname,
      agent: (up.protocol === 'https:'
        ? new https.Agent({ keepAlive: !DISABLE_KEEPALIVE, maxSockets: 64, servername: up.hostname })
        : new http.Agent({ keepAlive: !DISABLE_KEEPALIVE, maxSockets: 64 }))
    };

    const upstream = (up.protocol === 'https:' ? https : http).request(opts, (upr) => {
      const resHeaders = { ...upr.headers };
      res.writeHead(upr.statusCode || 500, resHeaders);
      log({ level: 'info', rid, phase: 'response_head', status: upr.statusCode, headers: redactHeaders(resHeaders) });
      if (EXIT_ON_5XX && upr.statusCode && upr.statusCode >= 500) {
        // Propagate response then schedule exit(2) once stream completes
        upr.on('end', () => { try { process.exitCode = 2; } catch {} });
      }
      let total = 0;
      const chunks = [];
      upr.on('data', (chunk) => { total += chunk.length; if (LOG_ERROR_BODY && upr.statusCode && upr.statusCode >= 400) { if (chunks.length < 32 && total <= LOG_ERROR_BODY_BYTES) chunks.push(chunk); } });
      upr.on('end', () => {
        if (LOG_ERROR_BODY && upr.statusCode && upr.statusCode >= 400) {
          let bodyStr = '';
          try { bodyStr = Buffer.concat(chunks).toString('utf8'); } catch {}
          log({ level: 'error', rid, phase: 'response_error_body', status: upr.statusCode, body: bodyStr.slice(0, LOG_ERROR_BODY_BYTES) });
        }
        log({ level: 'info', rid, phase: 'response_end', status: upr.statusCode, bytes: total, request_id: resHeaders['x-request-id'] || null });
      });
      upr.on('error', (e) => { log({ level: 'error', rid, phase: 'response_error', err: String(e) }); });
      upr.pipe(res);
    });
    upstream.setTimeout(600000, () => { upstream.destroy(new Error('upstream timeout')); });
    upstream.on('error', (e) => {
      log({ level: 'error', rid, phase: 'upstream_error', err: String(e) });
      if (!res.headersSent) res.writeHead(502, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ error: 'upstream_error' }));
    });
    upstream.end(body);
  });
});
server.headersTimeout = 650000;
server.keepAliveTimeout = 650000;
server.listen(PORT, '127.0.0.1', () => {
  log({ level: 'info', msg: 'proxy listening', addr: '127.0.0.1', port: PORT });
  if (READY_FILE) {
    try { require('fs').writeFileSync(READY_FILE, String(Date.now())); } catch {}
  }
});
