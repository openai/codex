# OpenAI Codex Python SDK (Beta)

Build Python applications that start Codex threads, run turns, stream progress,
and control workspace access.

> [!NOTE]
> `openai-codex` is in beta. Public APIs may change before `1.0`.

## Install

Install the SDK:

```bash
pip install openai-codex
```

For reproducible environments, install this release exactly:

```bash
pip install openai-codex==0.1.0b1
```

The SDK requires Python `>=3.10` and installs its compatible Codex runtime
dependency automatically. While beta releases are the only published SDK
releases, the normal install command selects the latest beta. After a stable
release exists, use `pip install --pre openai-codex` to explicitly select a
newer prerelease.

## Quickstart

The SDK reuses your existing Codex authentication when one is already
available:

```python
from openai_codex import Codex

with Codex() as codex:
    thread = codex.thread_start()
    result = thread.run("Explain this repository in three bullets.")
    print(result.final_response)
```

`thread.run(...)` returns a `TurnResult` containing the final response,
collected items, and token usage.

## Authentication

Existing Codex authentication is reused automatically. To start ChatGPT
browser login explicitly:

```python
from openai_codex import Codex

with Codex() as codex:
    login = codex.login_chatgpt()
    print(login.auth_url)
    print(login.wait().success)
```

For device-code login:

```python
with Codex() as codex:
    login = codex.login_chatgpt_device_code()
    print(login.verification_url, login.user_code)
    login.wait()
```

For API-key login:

```python
with Codex() as codex:
    codex.login_api_key("sk-...")
```

## Built-In Help

Use Python's standard `help(openai_codex)`, `help(Codex)`, or
`python -m pydoc openai_codex` documentation tools.

## Documentation

- [Getting started](https://github.com/openai/codex/blob/main/sdk/python/docs/getting-started.md)
- [API reference](https://github.com/openai/codex/blob/main/sdk/python/docs/api-reference.md)
- [FAQ](https://github.com/openai/codex/blob/main/sdk/python/docs/faq.md)
- [Examples](https://github.com/openai/codex/blob/main/sdk/python/examples/README.md)

The package is licensed under the
[repository Apache License 2.0](https://github.com/openai/codex/blob/main/LICENSE).
