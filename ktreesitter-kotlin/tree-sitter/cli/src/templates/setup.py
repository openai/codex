from os import name as os_name, path
from sysconfig import get_config_var

from setuptools import Extension, find_packages, setup
from setuptools.command.build import build
from setuptools.command.egg_info import egg_info
from wheel.bdist_wheel import bdist_wheel

sources = [
    "bindings/python/tree_sitter_LOWER_PARSER_NAME/binding.c",
    "src/parser.c",
]
if path.exists("src/scanner.c"):
    sources.append("src/scanner.c")

macros: list[tuple[str, str | None]] = [
    ("PY_SSIZE_T_CLEAN", None),
    ("TREE_SITTER_HIDE_SYMBOLS", None),
]
if limited_api := not get_config_var("Py_GIL_DISABLED"):
    macros.append(("Py_LIMITED_API", "0x030A0000"))

if os_name != "nt":
    cflags = ["-std=c11", "-fvisibility=hidden"]
else:
    cflags = ["/std:c11", "/utf-8"]


class Build(build):
    def run(self):
        if path.isdir("queries"):
            dest = path.join(self.build_lib, "tree_sitter_PARSER_NAME", "queries")
            self.copy_tree("queries", dest)
        super().run()


class BdistWheel(bdist_wheel):
    def get_tag(self):
        python, abi, platform = super().get_tag()
        if python.startswith("cp"):
            python, abi = "cp310", "abi3"
        return python, abi, platform


class EggInfo(egg_info):
    def find_sources(self):
        super().find_sources()
        self.filelist.recursive_include("queries", "*.scm")
        self.filelist.include("src/tree_sitter/*.h")


setup(
    packages=find_packages("bindings/python"),
    package_dir={"": "bindings/python"},
    package_data={
        "tree_sitter_LOWER_PARSER_NAME": ["*.pyi", "py.typed"],
        "tree_sitter_LOWER_PARSER_NAME.queries": ["*.scm"],
    },
    ext_package="tree_sitter_LOWER_PARSER_NAME",
    ext_modules=[
        Extension(
            name="_binding",
            sources=sources,
            extra_compile_args=cflags,
            define_macros=macros,
            include_dirs=["src"],
            py_limited_api=limited_api,
        )
    ],
    cmdclass={
        "build": Build,
        "bdist_wheel": BdistWheel,
        "egg_info": EggInfo,
    },
    zip_safe=False
)
