from __future__ import annotations

from .models import InitializeResponse, ServerInfo


def _split_user_agent(user_agent: str) -> tuple[str | None, str | None]:
    raw = user_agent.strip()
    if not raw:
        return None, None
    if "/" in raw:
        name, version = raw.split("/", 1)
        return (name or None), (version or None)
    parts = raw.split(maxsplit=1)
    if len(parts) == 2:
        return parts[0], parts[1]
    return raw, None


def validate_initialize_metadata(payload: InitializeResponse) -> InitializeResponse:
    user_agent = (payload.userAgent or "").strip()
    server = payload.serverInfo

    server_name: str | None = None
    server_version: str | None = None

    if server is not None:
        server_name = (server.name or "").strip() or None
        server_version = (server.version or "").strip() or None

    if (server_name is None or server_version is None) and user_agent:
        parsed_name, parsed_version = _split_user_agent(user_agent)
        if server_name is None:
            server_name = parsed_name
        if server_version is None:
            server_version = parsed_version

    normalized_server_name = (server_name or "").strip()
    normalized_server_version = (server_version or "").strip()
    if not user_agent or not normalized_server_name or not normalized_server_version:
        raise RuntimeError(
            "initialize response missing required metadata "
            f"(user_agent={user_agent!r}, server_name={normalized_server_name!r}, server_version={normalized_server_version!r})"
        )

    if server is None:
        payload.serverInfo = ServerInfo(
            name=normalized_server_name,
            version=normalized_server_version,
        )
    else:
        server.name = normalized_server_name
        server.version = normalized_server_version

    return payload
