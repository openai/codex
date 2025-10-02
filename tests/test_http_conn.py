import pytest

from scripts.connectors.http_conn import http_post


def test_http_post_path_allowlist_blocks():
    # Using a bogus base_url is fine; function should reject before curl executes
    with pytest.raises(PermissionError):
        http_post(
            base_url="https://example.invalid",
            path="/not-allowed",
            data={"x": 1},
            allowed_paths=["/ci/notify"],
        )

