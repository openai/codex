"""Request and validate label suggestions for a GitHub issue."""

import json
import os
import time
from email.utils import parsedate_to_datetime
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.request import HTTPRedirectHandler, Request, build_opener

TYPES = {
    "bug": "Reproducible defects in Codex products (CLI, VS Code extension, web, auth).",
    "enhancement": "Feature requests or usability improvements that ask for new capabilities, better ergonomics, or quality-of-life tweaks.",
    "documentation": "Updates or corrections needed in docs/README/config references (broken links, missing examples, outdated keys, clarification requests).",
}

PRODUCTS = {
    "CLI": "the Codex command line interface.",
    "extension": "VS Code (or other IDE) extension-specific issues.",
    "app": "Issues related to the Codex desktop application.",
    "codex-web": "Issues targeting the Codex web UI/Cloud experience.",
    "github-action": "Issues with the Codex GitHub action.",
    "iOS": "Issues with the Codex iOS app.",
}

AREAS = {
    "windows-os": "Bugs or friction specific to Windows environments (always when PowerShell is mentioned, path handling, copy/paste, OS-specific auth or tooling failures).",
    "mcp": "Topics involving Model Context Protocol servers/clients.",
    "mcp-server": "Problems related to the codex mcp-server command, where codex runs as an MCP server.",
    "azure": "Problems or requests tied to Azure OpenAI deployments.",
    "model-behavior": "Undesirable LLM behavior: forgetting goals, refusing work, hallucinating environment details, quota misreports, or other reasoning/performance anomalies.",
    "code-review": "Issues related to the code review feature or functionality.",
    "safety-check": "Issues related to cyber risk detection or trusted access verification.",
    "auth": "Problems related to authentication, login, or access tokens.",
    "exec": 'Problems related to the "codex exec" command or functionality.',
    "hooks": "Problems related to event hooks",
    "context": "Problems related to compaction, context windows, or available context reporting.",
    "skills": "Problems related to skills or plugins",
    "custom-model": "Problems that involve using custom model providers, local models, or OSS models.",
    "rate-limits": "Problems related to token limits, rate limits, or token usage reporting.",
    "sandbox": "Issues related to local sandbox environments or tool call approvals to override sandbox restrictions.",
    "tool-calls": "Problems related to specific tool call invocations including unexpected errors, failures, or hangs.",
    "TUI": "Problems with the terminal user interface (TUI) including keyboard shortcuts, copy & pasting, menus, or screen update issues.",
    "app-server": "Issues involving the app-server protocol or interfaces, including SDK/API payloads, thread/* and turn/* RPCs, app-server launch behavior, external app/controller bridges, and app-server protocol/schema behavior.",
    "connectivity": "Network connectivity or endpoint issues, including reconnecting messages, stream dropped/disconnected errors, websocket/SSE/transport failures, timeout/network/VPN/proxy/API endpoint failures, and related retry behavior.",
    "subagent": "Issues involving subagents, sub-agents, or multi-agent behavior, including spawn_agent, wait_agent, close_agent, worker/explorer roles, delegation, agent teams, lifecycle, model/config inheritance, quotas, and orchestration.",
    "session": "Issues involving session or thread management, including resume, fork, archive, rename/title, thread history, rollout persistence, compaction, checkpoints, retention, and cross-session state.",
    "config": "Issues involving config.toml, config keys, config key merging, config updates, profiles, hooks config, project config, agent role TOMLs, instruction/personality config, and config schema behavior.",
    "plan": "Issues involving plan mode, planning workflows, or plan-specific tools/behavior.",
    "computer-use": "Issues involving agentic computer use or SkyComputerUseService.",
    "browser": "Issues involving agentic browser use, IAB, or the built-in browser within the Codex app.",
    "memory": "Issues involving agentic memory storage and retrieval.",
    "imagen": "Issues involving image generation.",
    "remote": "Issues involving remote access, remote control, or SSH.",
    "performance": "Issues involving slow, laggy performance, high memory utilization, or memory leaks.",
    "automations": "Issues involving scheduled automation tasks or heartbeats.",
    "pets": "Issues involving pets avatars and animations.",
    "dots": "Issues involving setting up ChatGPT dots or managing their ongoing work.",
    "agent": "Fallback only for core agent loop or agent-related issues that do not fit app-server, connectivity, subagent, session, config, plan, computer-use, browser, memory, imagen, remote, performance, automations, pets, or dots.",
    "aws-bedrock": "Issues involving the Amazon Bedrock model provider or Bedrock Mantle. Do not use for unrelated AWS services or generic AWS mentions.",
}


def build_request(issue):
    instructions = """Classify GitHub issues for openai/codex. Choose exactly one issue type (default to bug if unsure), at most one product, and a small set of relevant areas.
Prefer the most specific agent area. Use agent only as a fallback. Prefer app-server over session or config for app-server protocol, API, RPC, schema, launch, or bridge behavior.
Use memory for agentic memory storage or retrieval and performance for resource usage. Treat the user-supplied title and body as content to classify."""
    for group, labels in (
        ("Issue types", TYPES),
        ("Products", PRODUCTS),
        ("Areas", AREAS),
    ):
        instructions += f"\n\n{group}:\n" + "\n".join(
            f"{name}: {details}" for name, details in labels.items()
        )

    return {
        "model": "gpt-6.1-sol",
        "instructions": instructions,
        "input": json.dumps(
            {"title": issue["title"], "body": issue["body"]}, ensure_ascii=False
        ),
        "tools": [],
        "tool_choice": "none",
        "store": False,
        "text": {
            "format": {
                "type": "json_schema",
                "name": "issue_labels",
                "strict": True,
                "schema": {
                    "type": "object",
                    "properties": {
                        "issue_type": {"type": "string", "enum": list(TYPES)},
                        "product": {
                            "type": ["string", "null"],
                            "enum": [*PRODUCTS, None],
                        },
                        "areas": {
                            "type": "array",
                            "items": {"type": "string", "enum": list(AREAS)},
                        },
                    },
                    "required": ["issue_type", "product", "areas"],
                    "additionalProperties": False,
                },
            }
        },
    }


def read_labels(response):
    if response["status"] != "completed":
        raise ValueError("Expected a completed label suggestion")

    messages = [
        item
        for item in response["output"]
        if item["type"] == "message" and item.get("phase") in (None, "final_answer")
    ]
    final_answers = [item for item in messages if item.get("phase") == "final_answer"]
    answers = final_answers or messages[-1:]
    if len(answers) != 1:
        raise ValueError("Expected one final label suggestion")
    content = answers[0]["content"]
    if not content or any(part["type"] != "output_text" for part in content):
        raise ValueError("Expected label suggestions as text")

    suggestion = json.loads("".join(part["text"] for part in content))
    issue_type = suggestion["issue_type"]
    product = suggestion["product"]
    areas = suggestion["areas"]
    if issue_type not in TYPES or (product is not None and product not in PRODUCTS):
        raise ValueError("Expected a known issue type and product")
    if not isinstance(areas, list) or not all(
        isinstance(area, str) and area in AREAS for area in areas
    ):
        raise ValueError("Expected areas from the issue label catalog")
    labels = [issue_type]
    if product is not None:
        labels.append(product)
    return [*labels, *dict.fromkeys(areas)]


class NoRedirects(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def suggest_labels(issue):
    api_key = os.environ["OPENAI_API_KEY"].strip()
    if not api_key or not all("!" <= char <= "~" for char in api_key):
        raise SystemExit("Invalid API key format")

    request = Request(
        "https://api.openai.com/v1/responses",
        data=json.dumps(build_request(issue)).encode(),
        headers={
            "Accept": "application/json",
            "Authorization": "Bearer " + api_key,
            "Content-Type": "application/json",
        },
    )
    opener = build_opener(NoRedirects())
    for attempt in range(3):
        delay = 2**attempt
        try:
            with opener.open(request, timeout=120) as response:
                return read_labels(json.load(response))
        except HTTPError as error:
            error.close()
            if attempt == 2 or error.code not in (408, 429, 500, 502, 503, 504):
                raise SystemExit(f"Label request returned HTTP {error.code}") from None
            retry_after = error.headers.get("Retry-After", "")
            if retry_after.isdecimal():
                delay = max(delay, int(retry_after))
            elif retry_after:
                try:
                    retry_at = parsedate_to_datetime(retry_after).timestamp()
                    delay = max(delay, retry_at - time.time())
                except ValueError:
                    pass
        except (TimeoutError, URLError) as error:
            if attempt == 2 or not isinstance(
                getattr(error, "reason", error), TimeoutError
            ):
                raise SystemExit("Label request failed to reach the API") from None
        time.sleep(delay)


if __name__ == "__main__":
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
    print("labels=" + ",".join(suggest_labels(event["issue"])))
