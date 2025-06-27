"""Script that spawns a local webserver for retrieving an OpenAI API key.

- Listens on 127.0.0.1:1455
- Opens http://localhost:1455/auth/callback in the browser
- If the user successfully navigates the auth flow,
  $CODEX_HOME/auth.json will be written with the API key.
- User will be redirected to http://localhost:1455/success upon success.

The script should exit with a non-zero code if the user fails to navigate the
auth flow.

To test this script locally without overwriting your existing auth.json file:

```
rm -rf /tmp/codex_home && mkdir /tmp/codex_home
CODEX_HOME=/tmp/codex_home python3 codex-rs/login/src/login_with_chatgpt.py
```
"""

from __future__ import annotations

import argparse
import base64
import datetime
import errno
import hashlib
import http.server
import json
import os
import secrets
import sys
import threading
import time
import urllib.parse
import urllib.request
import webbrowser
from dataclasses import dataclass
from typing import Any, Dict  # for type hints

# Required port for OAuth client.
REQUIRED_PORT = 1455
URL_BASE = f"http://localhost:{REQUIRED_PORT}"
DEFAULT_ISSUER = "https://auth.openai.com"
DEFAULT_CLIENT_ID = "app_EMoamEEZ73f0CkXaXp7hrann"

EXIT_CODE_WHEN_ADDRESS_ALREADY_IN_USE = 13


@dataclass
class TokenData:
    id_token: str
    access_token: str
    refresh_token: str


@dataclass
class AuthBundle:
    """Aggregates authentication data produced after successful OAuth flow."""

    api_key: str
    token_data: TokenData
    last_refresh: str


def main() -> None:
    parser = argparse.ArgumentParser(description="Retrieve API key via local HTTP flow")
    parser.add_argument(
        "--no-browser",
        action="store_true",
        help="Do not automatically open the browser",
    )
    parser.add_argument("--verbose", action="store_true", help="Enable request logging")
    args = parser.parse_args()

    codex_home = os.environ.get("CODEX_HOME")
    if not codex_home:
        eprint("ERROR: CODEX_HOME environment variable is not set")
        sys.exit(1)

    # Spawn server.
    try:
        httpd = _ApiKeyHTTPServer(
            ("127.0.0.1", REQUIRED_PORT),
            _ApiKeyHTTPHandler,
            codex_home=codex_home,
            verbose=args.verbose,
        )
    except OSError as e:
        eprint(f"ERROR: {e}")
        if e.errno == errno.EADDRINUSE:
            # Caller might want to handle this case specially.
            sys.exit(EXIT_CODE_WHEN_ADDRESS_ALREADY_IN_USE)
        else:
            sys.exit(1)

    auth_url = httpd.auth_url()

    with httpd:
        eprint(f"Starting local login server on {URL_BASE}")
        if not args.no_browser:
            try:
                webbrowser.open(auth_url, new=1, autoraise=True)
            except Exception as e:
                eprint(f"Failed to open browser: {e}")

        eprint(
            f"If your browser did not open, navigate to this URL to authenticate:\n\n{auth_url}"
        )

        # Run the server in the main thread until `shutdown()` is called by the
        # request handler.
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            eprint("\nKeyboard interrupt received, exiting.")

        # Server has been shut down by the request handler. Exit with the code
        # it set (0 on success, non-zero on failure).
        sys.exit(httpd.exit_code)


class _ApiKeyHTTPHandler(http.server.BaseHTTPRequestHandler):
    """A minimal request handler that captures an *api key* from query/post."""

    # We store the result in the server instance itself.
    server: "_ApiKeyHTTPServer"  # type: ignore[override]  - helpful annotation

    def do_GET(self) -> None:  # noqa: N802 – required by BaseHTTPRequestHandler
        path = urllib.parse.urlparse(self.path).path

        if path == "/success":
            # Serve confirmation page then gracefully shut down the server so
            # the main thread can exit with the previously captured exit code.
            self._send_html(LOGIN_SUCCESS_HTML)

            # Ensure the data is flushed to the client before we stop.
            try:
                self.wfile.flush()
            except Exception as e:
                eprint(f"Failed to flush response: {e}")

            self.request_shutdown()
        elif path == "/auth/callback":
            query = urllib.parse.urlparse(self.path).query
            params = urllib.parse.parse_qs(query)

            # Validate state -------------------------------------------------
            if params.get("state", [None])[0] != self.server.state:
                self.send_error(400, "State parameter mismatch")
                return

            # Standard OAuth flow -----------------------------------------
            code = params.get("code", [None])[0]
            if not code:
                self.send_error(400, "Missing authorization code")
                return

            try:
                auth_bundle, success_url = self._exchange_code_for_api_key(code)
            except Exception as exc:  # noqa: BLE001 – propagate to client
                self.send_error(500, f"Token exchange failed: {exc}")
                return

            # Persist API key along with additional token metadata.
            if _write_auth_file(
                auth=auth_bundle,
                codex_home=self.server.codex_home,
            ):
                self.server.exit_code = 0
                self._send_redirect(success_url)
            else:
                self.send_error(500, "Unable to persist auth file")
        else:
            self.send_error(404, "Endpoint not supported")

    def do_POST(self) -> None:  # noqa: N802 – required by BaseHTTPRequestHandler
        self.send_error(404, "Endpoint not supported")

    def send_error(self, code, message=None, explain=None) -> None:
        """Send an error response and stop the server.

        We avoid calling `sys.exit()` directly from the request-handling thread
        so that the response has a chance to be written to the socket. Instead
        we shut the server down; the main thread will then exit with the
        appropriate status code.
        """
        super().send_error(code, message, explain)
        try:
            self.wfile.flush()
        except Exception as e:
            eprint(f"Failed to flush response: {e}")

        self.request_shutdown()

    def _send_redirect(self, url: str) -> None:
        self.send_response(302)
        self.send_header("Location", url)
        self.end_headers()

    def _send_html(self, body: str) -> None:
        encoded = body.encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    # Silence logging for cleanliness unless --verbose flag is used.
    def log_message(self, fmt: str, *args):  # type: ignore[override]
        if getattr(self.server, "verbose", False):  # type: ignore[attr-defined]
            super().log_message(fmt, *args)

    def _exchange_code_for_api_key(self, code: str) -> tuple[AuthBundle, str]:
        """Perform token + token-exchange to obtain an OpenAI API key.

        Returns (AuthBundle, success_url).
        """

        token_endpoint = f"{self.server.issuer}/oauth/token"

        # 1. Authorization-code -> (id_token, access_token, refresh_token)
        data = urllib.parse.urlencode(
            {
                "grant_type": "authorization_code",
                "code": code,
                "redirect_uri": self.server.redirect_uri,
                "client_id": self.server.client_id,
                "code_verifier": self.server.pkce.code_verifier,
            }
        ).encode()

        token_data: TokenData

        with urllib.request.urlopen(
            urllib.request.Request(
                token_endpoint,
                data=data,
                method="POST",
                headers={"Content-Type": "application/x-www-form-urlencoded"},
            )
        ) as resp:
            payload = json.loads(resp.read().decode())
            token_data = TokenData(
                id_token=payload["id_token"],
                access_token=payload["access_token"],
                refresh_token=payload["refresh_token"],
            )

        id_token_parts = token_data.id_token.split(".")
        if len(id_token_parts) != 3:
            raise ValueError("Invalid ID token")
        access_token_parts = token_data.access_token.split(".")
        if len(access_token_parts) != 3:
            raise ValueError("Invalid access token")

        id_token_claims = _decode_jwt_segment(id_token_parts[1])
        access_token_claims = _decode_jwt_segment(access_token_parts[1])

        token_claims = id_token_claims.get("https://api.openai.com/auth", {})
        access_claims = access_token_claims.get("https://api.openai.com/auth", {})

        org_id = token_claims.get("organization_id")
        if not org_id:
            raise ValueError("Missing organization in id_token claims")

        project_id = token_claims.get("project_id")
        if not project_id:
            raise ValueError("Missing project in id_token claims")

        random_id = secrets.token_hex(6)

        # 2. Token exchange to obtain API key
        today = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")
        exchange_data = urllib.parse.urlencode(
            {
                "grant_type": "urn:ietf:params:oauth:grant-type:token-exchange",
                "client_id": self.server.client_id,
                "requested_token": "openai-api-key",
                "subject_token": token_data.id_token,
                "subject_token_type": "urn:ietf:params:oauth:token-type:id_token",
                "name": f"Codex CLI [auto-generated] ({today}) [{random_id}]",
            }
        ).encode()

        exchanged_access_token: str
        with urllib.request.urlopen(
            urllib.request.Request(
                token_endpoint,
                data=exchange_data,
                method="POST",
                headers={"Content-Type": "application/x-www-form-urlencoded"},
            )
        ) as resp:
            exchange_payload = json.loads(resp.read().decode())
            exchanged_access_token = exchange_payload["access_token"]

        # Determine whether the organization still requires additional
        # setup (e.g., adding a payment method) based on the ID-token
        # claim provided by the auth service.
        completed_onboarding = token_claims.get("completed_platform_onboarding") == True
        chatgpt_plan_type = access_claims.get("chatgpt_plan_type")
        is_org_owner = token_claims.get("is_org_owner") == True
        needs_setup = not completed_onboarding and is_org_owner

        # Build the success URL on the same host/port as the callback and
        # include the required query parameters for the front-end page.
        success_url_query = {
            "id_token": token_data.id_token,
            "needs_setup": "true" if needs_setup else "false",
            "org_id": org_id,
            "project_id": project_id,
            "plan_type": chatgpt_plan_type,
            "platform_url": (
                "https://platform.openai.com"
                if self.server.issuer == "https://auth.openai.com"
                else "https://platform.api.openai.org"
            ),
        }
        success_url = f"{URL_BASE}/success?{urllib.parse.urlencode(success_url_query)}"

        # Attempt to redeem complimentary API credits for eligible ChatGPT
        # Plus / Pro subscribers. Any errors are logged but do not interrupt
        # the login flow.

        try:
            maybe_redeem_credits(
                issuer=self.server.issuer,
                client_id=self.server.client_id,
                id_token=token_data.id_token,
                refresh_token=token_data.refresh_token,
                codex_home=self.server.codex_home,
            )
        except Exception as exc:  # pragma: no cover – best-effort only
            eprint(f"Unable to redeem ChatGPT subscriber API credits: {exc}")

        # Persist refresh_token/id_token for future use (redeem credits etc.)
        last_refresh_str = (
            datetime.datetime.now(datetime.timezone.utc)
            .isoformat()
            .replace("+00:00", "Z")
        )

        auth_bundle = AuthBundle(
            api_key=exchanged_access_token,
            token_data=token_data,
            last_refresh=last_refresh_str,
        )

        return (auth_bundle, success_url)

    def request_shutdown(self) -> None:
        # shutdown() must be invoked from another thread to avoid
        # deadlocking the serve_forever() loop, which is running in this
        # same thread. A short-lived helper thread does the trick.
        threading.Thread(target=self.server.shutdown, daemon=True).start()


def _write_auth_file(*, auth: AuthBundle, codex_home: str) -> bool:
    """Persist *api_key* to $CODEX_HOME/auth.json.

    Returns True on success, False otherwise.  Any error is printed to
    *stderr* so that the Rust layer can surface the problem.
    """
    if not os.path.isdir(codex_home):
        try:
            os.makedirs(codex_home, exist_ok=True)
        except Exception as exc:  # pragma: no cover – unlikely
            eprint(f"ERROR: unable to create CODEX_HOME directory: {exc}")
            return False

    auth_path = os.path.join(codex_home, "auth.json")
    auth_json_contents = {
        "OPENAI_API_KEY": auth.api_key,
        "tokens": {
            "id_token": auth.token_data.id_token,
            "access_token": auth.token_data.access_token,
            "refresh_token": auth.token_data.refresh_token,
        },
        "last_refresh": auth.last_refresh,
    }
    try:
        with open(auth_path, "w", encoding="utf-8") as fp:
            if hasattr(os, "fchmod"):  # POSIX-safe
                os.fchmod(fp.fileno(), 0o600)
            json.dump(auth_json_contents, fp, indent=2)
    except Exception as exc:  # pragma: no cover – permissions/filesystem
        eprint(f"ERROR: unable to write auth file: {exc}")
        return False

    return True


@dataclass
class PkceCodes:
    code_verifier: str
    code_challenge: str


class _ApiKeyHTTPServer(http.server.HTTPServer):
    """HTTPServer with shutdown helper & self-contained OAuth configuration."""

    def __init__(
        self,
        server_address: tuple[str, int],
        request_handler_class: type[http.server.BaseHTTPRequestHandler],
        *,
        codex_home: str,
        verbose: bool = False,
    ) -> None:
        super().__init__(server_address, request_handler_class, bind_and_activate=True)

        self.exit_code = 1
        self.codex_home = codex_home
        self.verbose: bool = verbose

        self.issuer: str = DEFAULT_ISSUER
        self.client_id: str = DEFAULT_CLIENT_ID
        port = server_address[1]
        self.redirect_uri: str = f"http://localhost:{port}/auth/callback"
        self.pkce: PkceCodes = _generate_pkce()
        self.state: str = secrets.token_hex(32)

    def auth_url(self) -> str:
        """Return fully-formed OpenID authorization URL."""
        params = {
            "response_type": "code",
            "client_id": self.client_id,
            "redirect_uri": self.redirect_uri,
            "scope": "openid profile email offline_access",
            "code_challenge": self.pkce.code_challenge,
            "code_challenge_method": "S256",
            "id_token_add_organizations": "true",
            "state": self.state,
        }
        return f"{self.issuer}/oauth/authorize?" + urllib.parse.urlencode(params)


def maybe_redeem_credits(
    *,
    issuer: str,
    client_id: str,
    id_token: str | None,
    refresh_token: str,
    codex_home: str,
) -> None:
    """Attempt to redeem complimentary API credits for ChatGPT subscribers.

    The operation is best-effort: any error results in a warning being printed
    and the function returning early without raising.
    """
    id_claims: Dict[str, Any] | None = parse_id_token_claims(id_token or "")

    # Refresh expired ID token, if possible
    token_expired = True
    if id_claims and isinstance(id_claims.get("exp"), int):
        token_expired = _current_timestamp_ms() >= int(id_claims["exp"]) * 1000

    if token_expired:
        eprint("Refreshing credentials...")
        new_refresh_token: str | None = None
        new_id_token: str | None = None

        try:
            payload = json.dumps(
                {
                    "client_id": client_id,
                    "grant_type": "refresh_token",
                    "refresh_token": refresh_token,
                    "scope": "openid profile email",
                }
            ).encode()

            req = urllib.request.Request(
                url="https://auth.openai.com/oauth/token",
                data=payload,
                method="POST",
                headers={"Content-Type": "application/json"},
            )

            with urllib.request.urlopen(req) as resp:
                refresh_data = json.loads(resp.read().decode())
                new_id_token = refresh_data.get("id_token")
                new_id_claims = parse_id_token_claims(new_id_token or "")
                new_refresh_token = refresh_data.get("refresh_token")
        except Exception as err:
            eprint("Unable to refresh ID token via token-exchange:", err)
            return

        if not new_id_token or not new_refresh_token:
            return

        # Update auth.json with new tokens.
        try:
            auth_dir = codex_home
            auth_path = os.path.join(auth_dir, "auth.json")
            with open(auth_path, "r", encoding="utf-8") as fp:
                existing = json.load(fp)

            tokens = existing.setdefault("tokens", {})
            tokens["id_token"] = new_id_token
            # Note this does not touch the access_token?
            tokens["refresh_token"] = new_refresh_token
            tokens["last_refresh"] = (
                datetime.datetime.now(datetime.timezone.utc)
                .isoformat()
                .replace("+00:00", "Z")
            )

            with open(auth_path, "w", encoding="utf-8") as fp:
                if hasattr(os, "fchmod"):
                    os.fchmod(fp.fileno(), 0o600)
                json.dump(existing, fp, indent=2)
        except Exception as err:
            eprint("Unable to update refresh token in auth file:", err)

        if not new_id_claims:
            # Still couldn't parse claims.
            return

        id_token = new_id_token
        id_claims = new_id_claims

    # Done refreshing credentials: now try to redeem credits.
    if not id_token:
        eprint("No ID token available, cannot redeem credits.")
        return

    auth_claims = id_claims.get("https://api.openai.com/auth", {})

    # Subscription eligibility check (Plus or Pro, >7 days active)
    sub_start_str = auth_claims.get("chatgpt_subscription_active_start")
    if isinstance(sub_start_str, str):
        try:
            sub_start_ts = datetime.datetime.fromisoformat(sub_start_str.rstrip("Z"))
            if datetime.datetime.now(
                datetime.timezone.utc
            ) - sub_start_ts < datetime.timedelta(days=7):
                eprint(
                    "Sorry, your subscription must be active for more than 7 days to redeem credits."
                )
                return
        except ValueError:
            # Malformed; ignore
            pass

    completed_onboarding = bool(auth_claims.get("completed_platform_onboarding"))
    is_org_owner = bool(auth_claims.get("is_org_owner"))
    needs_setup = not completed_onboarding and is_org_owner
    plan_type = auth_claims.get("chatgpt_plan_type")

    if needs_setup or plan_type not in {"plus", "pro"}:
        eprint("Only users with Plus or Pro subscriptions can redeem free API credits.")
        return

    api_host = (
        "https://api.openai.com"
        if issuer == "https://auth.openai.com"
        else "https://api.openai.org"
    )

    try:
        redeem_payload = json.dumps({"id_token": id_token}).encode()
        req = urllib.request.Request(
            url=f"{api_host}/v1/billing/redeem_credits",
            data=redeem_payload,
            method="POST",
            headers={"Content-Type": "application/json"},
        )

        with urllib.request.urlopen(req) as resp:
            redeem_data = json.loads(resp.read().decode())

        granted = redeem_data.get("granted_chatgpt_subscriber_api_credits", 0)
        if granted and granted > 0:
            eprint(
                f"""Thanks for being a ChatGPT {'Plus' if plan_type=='plus' else 'Pro'} subscriber!
If you haven't already redeemed, you should receive {'$5' if plan_type=='plus' else '$50'} in API credits.

Credits: https://platform.openai.com/settings/organization/billing/credit-grants
More info: https://help.openai.com/en/articles/11381614""",
            )
        else:
            eprint(
                f"""It looks like no credits were granted:

{json.dumps(redeem_data, indent=2)}

Credits: https://platform.openai.com/settings/organization/billing/credit-grants
More info: https://help.openai.com/en/articles/11381614"""
            )
    except Exception as err:
        eprint("Credit redemption request failed:", err)


def _generate_pkce() -> PkceCodes:
    """Generate PKCE *code_verifier* and *code_challenge* (S256)."""
    code_verifier = secrets.token_hex(64)
    digest = hashlib.sha256(code_verifier.encode()).digest()
    code_challenge = base64.urlsafe_b64encode(digest).rstrip(b"=").decode()
    return PkceCodes(code_verifier, code_challenge)


def eprint(*args, **kwargs) -> None:
    print(*args, file=sys.stderr, **kwargs)


# Parse ID-token claims (if provided)
#
# interface IDTokenClaims {
#   "exp": number; // specifically, an int
#   "https://api.openai.com/auth": {
#     organization_id: string;
#     project_id: string;
#     completed_platform_onboarding: boolean;
#     is_org_owner: boolean;
#     chatgpt_subscription_active_start: string;
#     chatgpt_subscription_active_until: string;
#     chatgpt_plan_type: string;
#   };
# }
def parse_id_token_claims(id_token: str) -> Dict[str, Any] | None:
    if id_token:
        parts = id_token.split(".")
        if len(parts) == 3:
            return _decode_jwt_segment(parts[1])
    return None


def _decode_jwt_segment(segment: str) -> Dict[str, Any]:
    """Return the decoded JSON payload from a JWT segment.

    Adds required padding for urlsafe_b64decode.
    """
    padded = segment + "=" * (-len(segment) % 4)
    try:
        data = base64.urlsafe_b64decode(padded.encode())
        return json.loads(data.decode())
    except Exception:
        return {}


def _current_timestamp_ms() -> int:
    return int(time.time() * 1000)


LOGIN_SUCCESS_HTML = """<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>Sign into Codex CLI</title>
    <link rel="icon" href='data:image/svg+xml,%3Csvg xmlns="http://www.w3.org/2000/svg" width="32" height="32" fill="none" viewBox="0 0 32 32"%3E%3Cpath stroke="%23000" stroke-linecap="round" stroke-width="2.484" d="M22.356 19.797H17.17M9.662 12.29l1.979 3.576a.511.511 0 0 1-.005.504l-1.974 3.409M30.758 16c0 8.15-6.607 14.758-14.758 14.758-8.15 0-14.758-6.607-14.758-14.758C1.242 7.85 7.85 1.242 16 1.242c8.15 0 14.758 6.608 14.758 14.758Z"/%3E%3C/svg%3E' type="image/svg+xml">
    <style>
      .container {
        margin: auto;
        height: 100%;
        display: flex;
        align-items: center;
        justify-content: center;
        position: relative;
        background: white;
        font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, 'Open Sans', 'Helvetica Neue', sans-serif;
      }
      .inner-container {
        width: 400px;
        flex-direction: column;
        justify-content: flex-start;
        align-items: center;
        gap: 20px;
        display: inline-flex;
      }
      .content {
        align-self: stretch;
        flex-direction: column;
        justify-content: flex-start;
        align-items: center;
        gap: 20px;
        display: flex;
      }
      .svg-wrapper {
        position: relative;
      }
      .title {
        text-align: center;
        color: var(--text-primary, #0D0D0D);
        font-size: 28px;
        font-weight: 400;
        line-height: 36.40px;
        word-wrap: break-word;
      }
      .setup-box {
        width: 600px;
        padding: 16px 20px;
        background: var(--bg-primary, white);
        box-shadow: 0px 4px 16px rgba(0, 0, 0, 0.05);
        border-radius: 16px;
        outline: 1px var(--border-default, rgba(13, 13, 13, 0.10)) solid;
        outline-offset: -1px;
        justify-content: flex-start;
        align-items: center;
        gap: 16px;
        display: inline-flex;
      }
      .setup-content {
        flex: 1 1 0;
        justify-content: flex-start;
        align-items: center;
        gap: 24px;
        display: flex;
      }
      .setup-text {
        flex: 1 1 0;
        flex-direction: column;
        justify-content: flex-start;
        align-items: flex-start;
        gap: 4px;
        display: inline-flex;
      }
      .setup-title {
        align-self: stretch;
        color: var(--text-primary, #0D0D0D);
        font-size: 14px;
        font-weight: 510;
        line-height: 20px;
        word-wrap: break-word;
      }
      .setup-description {
        align-self: stretch;
        color: var(--text-secondary, #5D5D5D);
        font-size: 14px;
        font-weight: 400;
        line-height: 20px;
        word-wrap: break-word;
      }
      .redirect-box {
        justify-content: flex-start;
        align-items: center;
        gap: 8px;
        display: flex;
      }
      .close-button,
      .redirect-button {
        height: 28px;
        padding: 8px 16px;
        background: var(--interactive-bg-primary-default, #0D0D0D);
        border-radius: 999px;
        justify-content: center;
        align-items: center;
        gap: 4px;
        display: flex;
      }
      .close-button,
      .redirect-text {
        color: var(--interactive-label-primary-default, white);
        font-size: 14px;
        font-weight: 510;
        line-height: 20px;
        word-wrap: break-word;
        text-decoration: none;
      }
    </style>
  </head>
  <body>
    <div class="container">
      <div class="inner-container">
        <div class="content">
          <div data-svg-wrapper class="svg-wrapper">
            <svg width="56" height="56" viewBox="0 0 56 56" fill="none" xmlns="http://www.w3.org/2000/svg">
              <path d="M4.6665 28.0003C4.6665 15.1137 15.1132 4.66699 27.9998 4.66699C40.8865 4.66699 51.3332 15.1137 51.3332 28.0003C51.3332 40.887 40.8865 51.3337 27.9998 51.3337C15.1132 51.3337 4.6665 40.887 4.6665 28.0003ZM37.5093 18.5088C36.4554 17.7672 34.9999 18.0203 34.2583 19.0742L24.8508 32.4427L20.9764 28.1808C20.1095 27.2272 18.6338 27.1569 17.6803 28.0238C16.7267 28.8906 16.6565 30.3664 17.5233 31.3199L23.3566 37.7366C23.833 38.2606 24.5216 38.5399 25.2284 38.4958C25.9353 38.4517 26.5838 38.089 26.9914 37.5098L38.0747 21.7598C38.8163 20.7059 38.5632 19.2504 37.5093 18.5088Z" fill="var(--green-400, #04B84C)"/>
            </svg>
          </div>
          <div class="title">Signed in to Codex CLI</div>
        </div>
        <div class="close-box" style="display: none;">
          <div class="setup-description">You may now close this page</div>
        </div>
        <div class="setup-box" style="display: none;">
          <div class="setup-content">
            <div class="setup-text">
              <div class="setup-title">Finish setting up your API organization</div>
              <div class="setup-description">Add a payment method to use your organization.</div>
            </div>
            <div class="redirect-box">
              <div data-hasendicon="false" data-hasstarticon="false" data-ishovered="false" data-isinactive="false" data-ispressed="false" data-size="large" data-type="primary" class="redirect-button">
                <div class="redirect-text">Redirecting in 3s...</div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
    <script>
      (function () {
        const params = new URLSearchParams(window.location.search);
        const needsSetup = params.get('needs_setup') === 'true';
        const platformUrl = params.get('platform_url') || 'https://platform.openai.com';
        const orgId = params.get('org_id');
        const projectId = params.get('project_id');
        const planType = params.get('plan_type');
        const idToken = params.get('id_token');
        // Show different message and optional redirect when setup is required
        if (needsSetup) {
          const setupBox = document.querySelector('.setup-box');
          setupBox.style.display = 'flex';
          const redirectUrlObj = new URL('/org-setup', platformUrl);
          redirectUrlObj.searchParams.set('p', planType);
          redirectUrlObj.searchParams.set('t', idToken);
          redirectUrlObj.searchParams.set('with_org', orgId);
          redirectUrlObj.searchParams.set('project_id', projectId);
          const redirectUrl = redirectUrlObj.toString();
          const message = document.querySelector('.redirect-text');
          let countdown = 3;
          function tick() {
            message.textContent =
              'Redirecting in ' + countdown + 's…';
            if (countdown === 0) {
              window.location.replace(redirectUrl);
            } else {
              countdown -= 1;
              setTimeout(tick, 1000);
            }
          }
          tick();
        } else {
          const closeBox = document.querySelector('.close-box');
          closeBox.style.display = 'flex';
        }
      })();
    </script>
  </body>
</html>"""

# Unconditionally call `main()` instead of gating it behind
# `if __name__ == "__main__"` because this script is either:
#
# - invoked as a string passed to `python3 -c`
# - run via `python3 login_with_chatgpt.py` for testing as part of local
#   development
main()
