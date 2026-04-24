from __future__ import annotations

import os

from hatchling.builders.hooks.plugin.interface import BuildHookInterface


def _platform_tag() -> str:
    from packaging.tags import sys_tags

    return next(iter(sys_tags())).platform


class RuntimeBuildHook(BuildHookInterface):
    def initialize(self, version: str, build_data: dict[str, object]) -> None:
        del version
        if self.target_name == "sdist":
            raise RuntimeError(
                "openai-codex-app-server-bin is wheel-only; build and publish platform wheels only."
            )

        platform_tag = self.config.get("platform-tag") or os.environ.get(
            "CODEX_APP_SERVER_BIN_PLATFORM_TAG"
        )
        if not isinstance(platform_tag, str) or not platform_tag:
            platform_tag = os.environ.get("CODEX_CLI_BIN_PLATFORM_TAG")
        if not isinstance(platform_tag, str) or not platform_tag:
            platform_tag = _platform_tag()

        build_data["pure_python"] = False
        build_data["infer_tag"] = False
        build_data["tag"] = f"py3-none-{platform_tag}"
