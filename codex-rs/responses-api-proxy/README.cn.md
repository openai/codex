# codex-responses-api-proxy（中文说明）

一个严格的 HTTP 代理，只允许将 `POST /v1/responses` 请求转发到 OpenAI API（`https://api.openai.com`），并自动注入 `Authorization: Bearer $OPENAI_API_KEY` 请求头。其他所有请求都会返回 `403 Forbidden`。

## 预期用法

**重要：** `codex-responses-api-proxy` 设计为由拥有 `OPENAI_API_KEY` 的高权限用户运行（例如 `root` 或具有 `sudo` 的用户），这样无权限用户无法检查或篡改该进程。如果加上 `--http-shutdown`，无权限用户 _可以_ 通过访问 `/shutdown` 来关闭服务，因为他们本身无法向该进程发送 `SIGTERM`。

有权访问 `OPENAI_API_KEY` 的高权限用户可以这样启动代理（代理从 `stdin` 读取 token）：

```shell
printenv OPENAI_API_KEY | env -u OPENAI_API_KEY codex-responses-api-proxy --http-shutdown --server-info /tmp/server-info.json
```

然后，无权限用户可以通过动态指定 `model_provider` 来运行 Codex：

```shell
PROXY_PORT=$(jq .port /tmp/server-info.json)
PROXY_BASE_URL="http://127.0.0.1:${PROXY_PORT}"
codex exec -c "model_providers.openai-proxy={ name = 'OpenAI Proxy', base_url = '${PROXY_BASE_URL}/v1', wire_api='responses' }" \
    -c model_provider="openai-proxy" \
    'Your prompt here'
```

当无权限用户不再需要时，可以使用 `curl` 关闭代理（因为无法发送 `SIGTERM`）：

```shell
curl --fail --silent --show-error "${PROXY_BASE_URL}/shutdown"
```

## 行为说明

- 从 `stdin` 读取 API key，调用方应通过管道传入（例如 `printenv OPENAI_API_KEY | codex-responses-api-proxy`）。
- 将 header 格式化为 `Bearer <key>`，并试图使用 `mlock(2)` 锁定这块持有 header 的内存，避免其被交换到磁盘。
- 在指定端口上监听（如未指定 `--port`，则使用临时端口）。
- 只接受 `POST /v1/responses`（不含 query string）。请求体会转发到 `https://api.openai.com/v1/responses`，并带上 `Authorization: Bearer <key>`。除了覆盖 `Authorization` 与 `Host` 外，其它请求头会原样透传。所有其他请求都返回 `403`。
- 可选地写出一行 JSON 到 `--server-info` 指定的文件，内容为 `{ "port": <u16>, "pid": <u32> }`。
- `--http-shutdown` 选项允许通过 `GET /shutdown` 以退出码 `0` 终止进程，方便高权限用户启动代理、低权限用户自行关闭。

## CLI

```shell
codex-responses-api-proxy [--port <PORT>] [--server-info <FILE>] [--http-shutdown] [--upstream-url <URL>]
```

- `--port <PORT>`：在 `127.0.0.1` 上绑定的端口；若省略，则使用随机端口。
- `--server-info <FILE>`：若设置，则在服务启动时写入 `{ "port": <PORT>, "pid": <PID> }`。
- `--http-shutdown`：允许通过 `GET /shutdown` 以退出码 `0` 关闭进程。
- `--upstream-url <URL>`：上游转发地址，默认 `https://api.openai.com/v1/responses`。
- 认证方式固定为 `Authorization: Bearer <key>`，与 Codex CLI 的预期一致。

示例（Azure 环境，确保你的部署接受 `Authorization: Bearer <key>`）：

```shell
printenv AZURE_OPENAI_API_KEY | env -u AZURE_OPENAI_API_KEY codex-responses-api-proxy \
  --http-shutdown \
  --server-info /tmp/server-info.json \
  --upstream-url "https://YOUR_PROJECT_NAME.openai.azure.com/openai/deployments/YOUR_DEPLOYMENT/responses?api-version=2025-04-01-preview"
```

## 加固细节

我们尽量减少 `OPENAI_API_KEY` 在内存中泄漏或被复制的机会：

- 使用 [`codex_process_hardening`](https://github.com/openai/codex/blob/main/codex-rs/process-hardening/README.md) 提供的标准进程加固。
- 启动时在栈上分配一个 1024 字节缓冲区，将 `"Bearer "` 写入开头。
- 之后从 `stdin` 读入 key 并追加到该缓冲区。
- 在验证 key 符合 `/^[a-zA-Z0-9_-]+$/` 且长度不超过缓冲区后，将其拷贝到堆上构造 `String`。
- 使用 <https://crates.io/crates/zeroize> 将栈缓冲区清零，避免被编译器优化掉。
- 调用 `.leak()` 将 `String` 转为 `&'static str`，使其在进程生命周期内一直有效。
- 在 UNIX 上，使用 `mlock(2)` 锁定该 `&'static str` 所在内存页。
- 构造 HTTP 请求时使用 `HeaderValue::from_static()` 直接引用该 `&str`，避免再次拷贝。
- 对该 `HeaderValue` 调用 `.set_sensitive(true)`，提示下游 HTTP 栈对该 header 进行敏感处理。

