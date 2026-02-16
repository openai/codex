from __future__ import annotations

import os
import shutil

import pytest

from codex_app_server import AppServerClient


pytestmark = pytest.mark.skipif(
    os.getenv("RUN_REAL_CODEX_TESTS") != "1" or shutil.which("codex") is None,
    reason="Set RUN_REAL_CODEX_TESTS=1 and ensure `codex` is available",
)


def test_real_initialize_and_model_list():
    with AppServerClient() as client:
        out = client.initialize()
        assert isinstance(out, dict)
        models = client.model_list(include_hidden=True)
        assert "data" in models
