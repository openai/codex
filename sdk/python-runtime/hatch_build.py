from __future__ import annotations

from hatchling.builders.hooks.plugin.interface import BuildHookInterface


class RuntimeBuildHook(BuildHookInterface):
    def initialize(self, version: str, build_data: dict[str, object]) -> None:
        del version
        if self.target_name == "sdist":
            raise RuntimeError(
                "codex-cli-bin is wheel-only; build and publish platform wheels only."
            )

        build_data["pure_python"] = False
        build_data["infer_tag"] = True
