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
from openai_codex import Codex, Sandbox

with Codex() as codex:
    thread = codex.thread_start(sandbox=Sandbox.workspace_write)
    result = thread.run("Explain this repository in three bullets.")
    print(result.final_response)
```

Use `Sandbox.workspace_write` for the normal workspace-editing experience.
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

Use `login_chatgpt_device_code()` for device-code login, or
`login_api_key("sk-...")` for API-key authentication.

## Sandbox Access

Choose a named sandbox preset when you create a thread or start a later turn:

| Preset | Access |
| --- | --- |
| `Sandbox.read_only` | Read files without writing. |
| `Sandbox.workspace_write` | Read files and write within the workspace and configured writable roots. This is the default for workspace work. |
| `Sandbox.full_access` | Run without filesystem access restrictions. |

When `sandbox=` is omitted, Codex uses its configured default. A sandbox
passed to `run(...)` or `turn(...)` applies to that turn and subsequent turns
on that thread.

## Errors And Retries

SDK errors derive from `CodexError`. Use `retry_on_overload(...)` only for
transient overload failures; invalid input and unsupported operations should
be corrected rather than retried.

## Built-In Help

Use Python's standard `help(openai_codex)`, `help(Codex)`, or
`python -m pydoc openai_codex` documentation tools.

## Runtime And Versioning

The SDK package version and runtime package version are independent.
`openai-codex==0.1.0b1` pins the compatible runtime dependency
`openai-codex-cli-bin==0.132.0`.

Most users should let the SDK select that runtime automatically.

## Documentation

- [Getting started](https://github.com/openai/codex/blob/main/sdk/python/docs/getting-started.md)
- [API reference](https://github.com/openai/codex/blob/main/sdk/python/docs/api-reference.md)
- [FAQ](https://github.com/openai/codex/blob/main/sdk/python/docs/faq.md)
- [Examples](https://github.com/openai/codex/blob/main/sdk/python/examples/README.md)

The package is licensed under the
[repository Apache License 2.0](https://github.com/openai/codex/blob/main/LICENSE).
