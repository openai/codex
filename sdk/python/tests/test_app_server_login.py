from __future__ import annotations

import base64
import json

from app_server_harness import AppServerHarness

from openai_codex import AppServerConfig, Codex
from openai_codex.generated.v2_all import (
    ChatgptAuthTokensLoginAccountParams,
    LoginAccountParams,
)
from openai_codex.types import CancelLoginAccountStatus


def _app_server_config(harness: AppServerHarness) -> AppServerConfig:
    """Build an isolated login config without inheriting ambient API-key auth."""
    config = harness.app_server_config()
    config.env = {**(config.env or {}), "OPENAI_API_KEY": ""}
    return config


def test_api_key_login_account_read_and_logout_round_trip(tmp_path) -> None:
    """The public sync auth helpers should persist and clear API-key auth."""
    with AppServerHarness(tmp_path) as harness:
        with Codex(config=_app_server_config(harness)) as codex:
            codex.login_api_key("sk-sdk-login-test")
            authenticated = codex.account()
            codex.logout()
            logged_out = codex.account()

    assert {
        "authenticated_type": None
        if authenticated.account is None
        else authenticated.account.root.type,
        "logged_out_account": logged_out.account,
    } == {
        "authenticated_type": "apiKey",
        "logged_out_account": None,
    }


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


def test_browser_login_waiters_stay_scoped_across_replaced_attempts(tmp_path) -> None:
    """Replacing one browser login should complete the matching handle only."""
    with AppServerHarness(tmp_path) as harness:
        with Codex(config=_app_server_config(harness)) as codex:
            first = codex.login_chatgpt()
            second = codex.login_chatgpt()
            first_completed = first.wait()
            second_canceled = second.cancel()
            second_completed = second.wait()

    assert {
        "distinct_attempts": first.login_id != second.login_id,
        "first_completion": {
            "login_id": first_completed.login_id,
            "success": first_completed.success,
            "error_present": bool(first_completed.error),
        },
        "second_cancel_status": second_canceled.status,
        "second_completion": {
            "login_id": second_completed.login_id,
            "success": second_completed.success,
            "error_present": bool(second_completed.error),
        },
    } == {
        "distinct_attempts": True,
        "first_completion": {
            "login_id": first.login_id,
            "success": False,
            "error_present": True,
        },
        "second_cancel_status": CancelLoginAccountStatus.canceled,
        "second_completion": {
            "login_id": second.login_id,
            "success": False,
            "error_present": True,
        },
    }
