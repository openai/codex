# codex-responses-api-proxy

A strict HTTP proxy that only forwards `POST` requests to `/v1/responses` to the OpenAI API (`https://api.openai.com`), injecting the `Authorization: Bearer $OPENAI_API_KEY` header. Everything else is rejected with `403 Forbidden`.

## Expected Usage

**IMPORTANT:** This is designed to be used with `CODEX_SECURE_MODE=1` so that an unprivileged user cannot inspect or tamper with this process. Though if `--http-shutdown` is specified, an unprivileged user _can_ shutdown the server.

A privileged user (i.e., `root` or a user with `sudo`) who has access to `OPENAI_API_KEY` would run the following to start the server:

```shell
printenv OPENAI_API_KEY | CODEX_SECURE_MODE=1 codex responses-api-proxy --http-shutdown --server-info /tmp/server-info.json
```

A non-privileged user would then run Codex as follows, specifying the `model_provider` dynamically:

```shell
PROXY_PORT=$(jq .port /tmp/server-info.json)
PROXY_BASE_URL="http://127.0.0.1:${PROXY_PORT}"
codex exec -c "model_providers.openai-proxy={ name = 'OpenAI Proxy', base_url = '${PROXY_BASE_URL}/v1', wire_api='responses' }" \
    -c model_provider="openai-proxy" \
    'Your prompt here'
```

When the unprivileged user was finished, they could shutdown the server using `curl` (since `kill -9` is not an option):

```shell
curl --fail --silent --show-error "${PROXY_BASE_URL}/shutdown"
```

## Behavior

- Reads the API key from `stdin`. All callers should pipe the key in (for example, `printenv OPENAI_API_KEY | codex responses-api-proxy`).
- Formats the header value as `Bearer <key>` and attempts to `mlock(2)` the memory holding that header so it is not swapped to disk.
- Listens on the provided port or an ephemeral port if `--port` is not specified.
- Accepts exactly `POST /v1/responses` (no query string). The request body is forwarded to `https://api.openai.com/v1/responses` with `Authorization: Bearer <key>` set. All original request headers (except any incoming `Authorization`) are forwarded upstream. For other requests, it responds with `403`.
- Optionally writes a single-line JSON file with server info, currently `{ "port": <u16> }`.
- Optional `--http-shutdown` enables `GET /shutdown` to terminate the process with exit code 0. This allows one user (e.g., root) to start the proxy and another unprivileged user on the host to shut it down.

## CLI

```
responses-api-proxy [--port <PORT>] [--server-info <FILE>] [--http-shutdown]
```

- `--port <PORT>`: Port to bind on `127.0.0.1`. If omitted, an ephemeral port is chosen.
- `--server-info <FILE>`: If set, the proxy writes a single line of JSON with `{ "port": <PORT> }` once listening.
- `--http-shutdown`: If set, enables `GET /shutdown` to exit the process with code `0`.

## Notes

- Only `POST /v1/responses` is permitted. No query strings are allowed.
- All request headers are forwarded to the upstream call (aside from overriding `Authorization`). Response status and content-type are mirrored from upstream.
