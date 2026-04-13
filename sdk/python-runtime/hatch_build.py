from __future__ import annotations

import os

from hatchling.builders.hooks.plugin.interface import BuildHookInterface

PLATFORM_TAG_BY_TARGET = {
    "aarch64-apple-darwin": "macosx_11_0_arm64",
    "x86_64-apple-darwin": "macosx_10_12_x86_64",
    "aarch64-unknown-linux-musl": "musllinux_1_2_aarch64",
    "x86_64-unknown-linux-musl": "musllinux_1_2_x86_64",
    "aarch64-pc-windows-msvc": "win_arm64",
    "x86_64-pc-windows-msvc": "win_amd64",
}


class RuntimeBuildHook(BuildHookInterface):
    def initialize(self, version: str, build_data: dict[str, object]) -> None:
        del version
        if self.target_name == "sdist":
            raise RuntimeError(
                "openai-codex-cli-bin is wheel-only; build and publish platform wheels only."
            )

        build_data["pure_python"] = False
        target = os.environ.get("CODEX_PYTHON_RUNTIME_TARGET")
        if target is None:
            build_data["infer_tag"] = True
            return

        platform_tag = PLATFORM_TAG_BY_TARGET.get(target)
        if platform_tag is None:
            raise RuntimeError(f"Unsupported Codex Python runtime target: {target}")
        build_data["tag"] = f"py3-none-{platform_tag}"
