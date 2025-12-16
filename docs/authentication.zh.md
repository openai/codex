# 身份验证与登录（Authentication）

> 本文是 `docs/authentication.md` 的中文概览版本，并非逐字翻译。最新和完整说明以英文原文为准。

Codex 支持两类主要身份验证方式：

1. 通过 ChatGPT 账户登录（推荐）。
2. 使用 API Key（适合用量计费或自托管/第三方模型场景）。

## 使用 ChatGPT 账户登录

推荐做法是在终端中运行：

```bash
codex
```

在 TUI 中按照提示选择 **Sign in with ChatGPT**：

- 浏览器会打开登录页面，完成登录并授权后，凭据会安全存储在本机。
- 之后再次运行 `codex` 时，无需重复登录。

如果你只想检查当前登录状态，可以使用：

```bash
codex login status
```

退出登录则使用：

```bash
codex logout
```

## 使用 API Key

如果你更希望使用 API Key（例如用量计费或自建/第三方兼容 OpenAI API 的服务）：

- 将 API Key 设置到环境变量中，例如：

```bash
export OPENAI_API_KEY="你的 API Key"
```

- 也可以使用 `codex login --with-api-key` 从 stdin 读取：

```bash
printenv OPENAI_API_KEY | codex login --with-api-key
```

> 注意：具体变量名和 provider 相关，使用非 OpenAI provider 时，需要查看对应的配置说明。

## 设备码与“无头”环境

在没有浏览器的环境（例如远程服务器、CI）中，可以使用设备码流程：

```bash
codex login --device-auth
```

这会在终端中打印一段代码和一个 URL，你可以在有浏览器的设备上完成授权。

## 多种 provider 与自定义 endpoint

Codex 支持多种兼容 OpenAI Chat Completions API 的 provider：

- 通过配置文件中的 provider 设置，或 CLI 参数选择。
- 对于第三方 provider，通常需要同时设置：
  - `*_API_KEY`
  - `*_BASE_URL`（自定义 API endpoint）

具体 provider 列表和配置示例请参考：

- `docs/authentication.md`
- `docs/config.md` 中的相关章节。

