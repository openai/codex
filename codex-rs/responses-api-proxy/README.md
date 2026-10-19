# codex-responses-api-proxy

#### tl;dr:

```
# Launch the proxy, dump request/response pairs to /tmp/proxy
cd path/to/codex/codex-rs
cargo build
echo $OPENAI_API_KEY | ./target/debug/codex-responses-api-proxy \
    --port 60001 \
    --dump-dir /tmp/proxy


# Add this to ~/.codex/config.toml:

[model_providers.codex-responses-api-proxy]
name = 'codex-responses-api-proxy'
base_url = 'http://127.0.0.1:60001/v1'
wire_api='responses'

[profiles.proxy]
model_provider = "codex-responses-api-proxy"


# Use it
codex -p proxy
```

# Detailed docs

A strict HTTP proxy that forwards `POST /v1/responses` to the OpenAI API
(`https://api.openai.com`). It authenticates with an API key by default, or with
short-lived OpenAI credentials obtained through GitHub Actions OIDC. Other
requests are rejected with `403 Forbidden`, except for the optional shutdown
endpoint described below.

## API-key usage (default)

**IMPORTANT:** `codex-responses-api-proxy` is designed to be run by a privileged user with access to `OPENAI_API_KEY` so that an unprivileged user cannot inspect or tamper with the process. Though if `--http-shutdown` is specified, an unprivileged user _can_ make a `GET` request to `/shutdown` to shutdown the server, as an unprivileged user could not send `SIGTERM` to kill the process.

A privileged user (i.e., `root` or a user with `sudo`) who has access to `OPENAI_API_KEY` would run the following to start the server, as `codex-responses-api-proxy` reads the auth token from `stdin`:

```shell
printenv OPENAI_API_KEY | env -u OPENAI_API_KEY codex-responses-api-proxy --http-shutdown --server-info /tmp/server-info.json
```

A non-privileged user would then run Codex as follows, specifying the `model_provider` dynamically:

```shell
PROXY_PORT=$(jq .port /tmp/server-info.json)
PROXY_BASE_URL="http://127.0.0.1:${PROXY_PORT}"
codex exec -c "model_providers.openai-proxy={ name = 'OpenAI Proxy', base_url = '${PROXY_BASE_URL}/v1', wire_api='responses' }" \
    -c model_provider="openai-proxy" \
    'Your prompt here'
```

When the unprivileged user was finished, they could shutdown the server using `curl` (since `kill -SIGTERM` is not an option):

```shell
curl --fail --silent --show-error "${PROXY_BASE_URL}/shutdown"
```

## GitHub Actions OIDC (OpenAI API)

The proxy can exchange GitHub Actions OIDC identities for short-lived OpenAI
API service account credentials. This removes the need to store a long-lived
OpenAI API key in GitHub secrets. It requires a workflow/job with
`permissions: { id-token: write, contents: read }` and an OpenAI organization
owner to configure a workload identity provider and service account mapping.
See the [OpenAI GitHub Actions federation guide](https://developers.openai.com/api/docs/guides/workload-identity-federation/github-actions).
Set the provider audience to the exact value used by the workflow, and restrict
the mapping to trusted repository, ref, and workflow claims.

In this mode, stdin supplies GitHub's `ACTIONS_ID_TOKEN_REQUEST_TOKEN`, **not**
the minted GitHub OIDC JWT or an OpenAI API key. The proxy needs that request
credential to request fresh subject tokens during long runs.

For the npm-installed proxy, use Node.js 24 or newer for the example below.
It sets `NODE_OPTIONS=--disable-sigusr1` to protect the launcher's credential
input from Node's debugger. Some older Node versions accepted by the npm package
do not support this option and will exit before starting the proxy. To run
without Node.js, replace `codex-responses-api-proxy` in the example with the
path to the native binary; keep the same pipe and environment handling.

Supply the audience, provider ID, and service account ID as GitHub Actions
variables (they are not secrets). Start the proxy as a privileged user, as in
the API-key example:

```bash
printenv ACTIONS_ID_TOKEN_REQUEST_TOKEN | (
  exec env -u ACTIONS_ID_TOKEN_REQUEST_TOKEN -u NODE_OPTIONS \
    NODE_OPTIONS=--disable-sigusr1 \
    codex-responses-api-proxy \
    --github-oidc \
    --oidc-audience "$OPENAI_WIF_AUDIENCE" \
    --identity-provider-id "$OPENAI_IDENTITY_PROVIDER_ID" \
    --service-account-id "$OPENAI_SERVICE_ACCOUNT_ID" \
    --github-oidc-request-url "$ACTIONS_ID_TOKEN_REQUEST_URL" \
    --http-shutdown --server-info /tmp/server-info.json
)
```

All four OIDC settings are required with `--github-oidc` and are rejected
without it. Pass `--github-oidc-request-url` directly from
`ACTIONS_ID_TOKEN_REQUEST_URL` supplied by the trusted GitHub runner, as shown
above. Do not set or overwrite it from workflow inputs, repository content, or
an untrusted subprocess. The proxy sends the GitHub request credential to that
URL. It requires HTTPS and refuses redirects, but it does not verify that GitHub
owns the destination. Do not substitute an OIDC issuer or discovery URL.

In OIDC mode only the default `https://api.openai.com/v1/responses` upstream is
allowed, and redirects on both token and Responses requests are disabled.
Azure/custom provider URLs continue to work in API-key mode.

Use a real pipe for either credential type, as in the examples above. Bash
here-strings and here-documents can create a seekable temporary file containing
the credential. If you background the proxy, do not wrap the entire pipeline
in another shell: it could retain the original credential environment while
it waits.

The proxy performs an initial exchange before writing server info. It keeps
the access token in process memory, renews it using a fresh GitHub JWT before
expiry, and serializes concurrent renewals. A failed renewal fails closed with
HTTP 502; it does not reuse an expired token or replay a Responses POST. It
shares the failure with waiting requests and delays further exchange attempts
by one second, doubling after each failure up to 30 seconds. A successful
exchange resets that delay. If the Responses endpoint returns 401, the proxy
passes through that response and discards only the credential used for it;
the next request can obtain a fresh one.

The proxy never returns tokens over its local HTTP API. Token endpoint error
bodies and credential-bearing URL errors are suppressed in diagnostics.

**Caller responsibilities:** run the proxy with the existing privileged-process
protections. Remove `ACTIONS_ID_TOKEN_REQUEST_TOKEN` and
`ACTIONS_ID_TOKEN_REQUEST_URL` from the environment **before** launching Codex
or any untrusted subprocess. The proxy cannot sanitize the parent GitHub
runner's environment for other steps. Never put GitHub or OpenAI bearer tokens
in arguments, Codex config, server-info files, `GITHUB_OUTPUT`, or
`GITHUB_ENV`. Retain the same privilege separation as API-key mode. The
OIDC mode uses sensitive HTTP headers and zeroizes temporary credential
buffers; it does not make the API-key path's single static, mlocked-allocation
guarantee for tokens passing through the HTTP/JSON client.

## Behavior

- By default reads the API key from `stdin`. With `--github-oidc`, reads the GitHub OIDC request credential instead; see above.
- In API-key mode, formats the header value as `Bearer <key>` and attempts to `mlock(2)` the memory holding that header so it is not swapped to disk.
- Listens on the provided port or an ephemeral port if `--port` is not specified.
- Accepts exactly `POST /v1/responses` (no query string). The request body is forwarded to the configured upstream (by default `https://api.openai.com/v1/responses`) with `Authorization` set to the selected API key or OpenAI access token. All original request headers (except any incoming `Authorization`) are forwarded upstream, with `Host` overridden to the upstream host. For other requests, it responds with `403`, aside from the optional shutdown endpoint.
- Optionally writes a single-line JSON file with server info, currently `{ "port": <u16>, "pid": <u32> }`.
- Optionally writes request/response JSON dumps to a directory. Each accepted request gets a pair of files that share a sequence/timestamp prefix, for example `000001-1846179912345-request.json` and `000001-1846179912345-response.json`. Header values are dumped in full except `Authorization` and any header whose name includes `cookie`, which are redacted. Bodies are written as parsed JSON when possible, otherwise as UTF-8 text.
- Optional `--http-shutdown` enables `GET /shutdown` to terminate the process with exit code `0`. This allows one user (e.g., `root`) to start the proxy and another unprivileged user on the host to shut it down.

## CLI

```
codex-responses-api-proxy [--port <PORT>] [--server-info <FILE>] [--http-shutdown] [--upstream-url <URL>] [--dump-dir <DIR>]
  [--github-oidc --oidc-audience <AUDIENCE> --identity-provider-id <ID>
   --service-account-id <ID> --github-oidc-request-url <URL>]
```

- `--port <PORT>`: Port to bind on `127.0.0.1`. If omitted, an ephemeral port is chosen.
- `--server-info <FILE>`: If set, the proxy writes a single line of JSON with `{ "port": <PORT>, "pid": <PID> }` once listening.
- `--http-shutdown`: If set, enables `GET /shutdown` to exit the process with code `0`.
- `--upstream-url <URL>`: Absolute URL to forward requests to. Defaults to `https://api.openai.com/v1/responses`.
- `--dump-dir <DIR>`: If set, writes one request JSON file and one response JSON file per accepted proxy call under this directory. Filenames use a shared sequence/timestamp prefix so each pair is easy to correlate.
- Authentication is `Authorization: Bearer <key>` in API-key mode and `Authorization: Bearer <OpenAI access token>` in OIDC mode.

For Azure, for example (ensure your deployment accepts `Authorization: Bearer <key>`):

```shell
printenv AZURE_OPENAI_API_KEY | env -u AZURE_OPENAI_API_KEY codex-responses-api-proxy \
  --http-shutdown \
  --server-info /tmp/server-info.json \
  --upstream-url "https://YOUR_PROJECT_NAME.openai.azure.com/openai/deployments/YOUR_DEPLOYMENT/responses?api-version=2025-04-01-preview"
```

## Notes

- Only `POST /v1/responses` is permitted. No query strings are allowed.
- All request headers are forwarded to the upstream call (aside from overriding `Authorization` and `Host`). Response status and content-type are mirrored from upstream.

## Hardening Details

Care is taken to restrict access/copying to the value of `OPENAI_API_KEY` retained in memory:

- We leverage [`codex_process_hardening`](https://github.com/openai/codex/blob/main/codex-rs/process-hardening/README.md) so `codex-responses-api-proxy` is run with standard process-hardening techniques.
- At startup, we allocate a `1024` byte buffer on the stack and copy `"Bearer "` into the start of the buffer.
- We then read from `stdin`, copying the contents into the buffer after `"Bearer "`.
- After verifying the key matches `/^[a-zA-Z0-9_-]+$/` (and does not exceed the buffer), we create a `String` from that buffer (so the data is now on the heap).
- We zero out the stack-allocated buffer using https://crates.io/crates/zeroize so it is not optimized away by the compiler.
- We invoke `.leak()` on the `String` so we can treat its contents as a `&'static str`, as it will live for the rest of the process.
- On UNIX, we `mlock(2)` the memory backing the `&'static str`.
- When using the `&'static str` when building an HTTP request, we use `HeaderValue::from_static()` to avoid copying the `&str`.
- We also invoke `.set_sensitive(true)` on the `HeaderValue`, which in theory indicates to other parts of the HTTP stack that the header should be treated with "special care" to avoid leakage:

https://github.com/hyperium/http/blob/439d1c50d71e3be3204b6c4a1bf2255ed78e1f93/src/header/value.rs#L346-L376
