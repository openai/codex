from __future__ import annotations

import base64
import json

from app_server_harness import AppServerHarness

from openai_codex import AppServerConfig, Codex
from openai_codex.generated.v2_all import (
    ChatgptAuthTokensLoginAccountParams,
    LoginAccountParams,
)


def _app_server_config(harness: AppServerHarness) -> AppServerConfig:
    """Build an isolated login config without inheriting ambient API-key auth."""
    config = harness.app_server_config()
    config.env = {**(config.env or {}), "OPENAI_API_KEY": ""}
    return config


def test_api_key_login_authenticates_follow_up_model_requests(tmp_path) -> None:
    """API-key login should authorize the next Responses request with that key."""
    with AppServerHarness(tmp_path, requires_openai_auth=True) as harness:
        harness.responses.enqueue_assistant_message("api key auth", response_id="api-key-auth")

        with Codex(config=_app_server_config(harness)) as codex:
            codex.login_api_key("sk-sdk-login-test")
            result = codex.thread_start().run("prove api key auth")
            request = harness.responses.single_request()

    assert {
        "final_response": result.final_response,
        "authorization": request.header("authorization"),
    } == {
        "final_response": "api key auth",
        "authorization": "Bearer sk-sdk-login-test",
    }


def test_chatgpt_token_login_authenticates_follow_up_model_requests(tmp_path) -> None:
    """ChatGPT token handoff should authorize later Responses requests with that token."""
    account_id = "workspace-sdk-chatgpt"

    def _encode(payload: dict[str, object]) -> str:
        raw = json.dumps(payload, separators=(",", ":"), sort_keys=True).encode("utf-8")
        return base64.urlsafe_b64encode(raw).rstrip(b"=").decode("ascii")

    # App-server parses claims from the access token before persisting external ChatGPT auth.
    header = _encode({"alg": "none", "typ": "JWT"})
    claims = _encode(
        {
            "email": "sdk-chatgpt@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": account_id,
                "chatgpt_plan_type": "pro",
            },
        }
    )
    access_token = f"{header}.{claims}.sig"

    with AppServerHarness(tmp_path, requires_openai_auth=True) as harness:
        harness.responses.enqueue_assistant_message(
            "chatgpt token auth",
            response_id="chatgpt-token-auth",
        )

        with Codex(config=_app_server_config(harness)) as codex:
            login = codex._client.account_login_start(
                LoginAccountParams(
                    root=ChatgptAuthTokensLoginAccountParams(
                        access_token=access_token,
                        chatgpt_account_id=account_id,
                        chatgpt_plan_type="pro",
                        type="chatgptAuthTokens",
                    )
                )
            )
            result = codex.thread_start().run("prove chatgpt token auth")
            request = harness.responses.single_request()

    assert {
        "login_type": login.root.type,
        "final_response": result.final_response,
        "authorization": request.header("authorization"),
    } == {
        "login_type": "chatgptAuthTokens",
        "final_response": "chatgpt token auth",
        "authorization": f"Bearer {access_token}",
    }
