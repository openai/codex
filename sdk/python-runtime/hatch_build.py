from __future__ import annotations

from hatchling.builders.hooks.plugin.interface import BuildHookInterface


class RuntimeWheelBuildHook(BuildHookInterface):
    def initialize(self, version: str, build_data: dict[str, object]) -> None:
        del version
        build_data["pure_python"] = False
        build_data["infer_tag"] = True
