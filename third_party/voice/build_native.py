#!/usr/bin/env python3
"""Build the candidate native voice prefix with upstream build systems."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shlex
import subprocess
import sys

# Match the package builder: import this script's sibling under PYTHONSAFEPATH,
# without adding the caller's working directory to the module search path.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from prepare_sources import MANIFEST, load_sources, prepare_sources

TARGET_SYSTEMS = {
    "apple-darwin": "Darwin",
    "unknown-linux-gnu": "Linux",
    "pc-windows-msvc": "Windows",
}


def validate_target(target, system, machine, libc, deployment_target):
    architecture, separator, suffix = target.partition("-")
    host_architecture = {"arm64": "aarch64", "amd64": "x86_64"}.get(
        machine.lower(), machine.lower()
    )
    if (
        not separator
        or architecture not in ("aarch64", "x86_64")
        or suffix not in TARGET_SYSTEMS
    ):
        raise ValueError(f"Unsupported native voice target: {target}")
    if TARGET_SYSTEMS[suffix] != system or architecture != host_architecture:
        raise ValueError("Use a native build host matching the requested target")
    if system == "Linux" and libc != "glibc":
        raise ValueError("Native Linux voice builds require glibc")
    if system == "Darwin" and not re.fullmatch(
        r"[0-9]+\.[0-9]+(?:\.[0-9]+)?", deployment_target or ""
    ):
        raise ValueError(
            "Declare the macOS deployment target; do not inherit the host default"
        )


class NativeBuild:
    def __init__(self, args, inherited_environment):
        validate_target(
            args.target,
            platform.system(),
            platform.machine(),
            platform.libc_ver()[0],
            args.deployment_target,
        )
        self.args = args
        self.output = args.output.absolute()
        self.prefix = self.output / "prefix"
        self.tools = self.output / "tools"
        self.windows = args.target.endswith("windows-msvc")
        # Compiler drivers such as clang++ select link behavior from argv[0].
        self.toolchain = {
            name: getattr(args, name).absolute()
            for name in ("cc", "cxx", "cmake", "make", "pkg_config", "shell")
        }
        self.bootstrap_make = (args.bootstrap_make or args.make).absolute()
        for tool in (*self.toolchain.values(), self.bootstrap_make):
            if not tool.is_file():
                raise ValueError(f"Missing build tool: {tool}")
        if self.windows and not all(
            inherited_environment.get(key) for key in ("INCLUDE", "LIB")
        ):
            raise ValueError(
                "Initialize the standard Visual Studio build environment first"
            )
        self.environment = {
            key: value
            for key, value in inherited_environment.items()
            if key
            in (
                "HOME",
                "TMPDIR",
                "TMP",
                "TEMP",
                "SYSTEMROOT",
                "SystemRoot",
                "WINDIR",
                "COMSPEC",
                "INCLUDE",
                "LIB",
                "LIBPATH",
                "HTTPS_PROXY",
                "HTTP_PROXY",
                "NO_PROXY",
            )
            or (self.windows and key == "USERPROFILE")
        }
        paths = [
            str(self.tools / "bin"),
            *(str(p.parent) for p in self.toolchain.values()),
        ]
        paths += (
            inherited_environment.get("PATH", "").split(os.pathsep)
            if self.windows
            else ["/usr/bin", "/bin", "/usr/sbin", "/sbin"]
        )
        self.environment.update(
            {
                "PATH": os.pathsep.join(dict.fromkeys(paths)),
                "LANG": "C",
                "LC_ALL": "C",
                "PYTHONDONTWRITEBYTECODE": "1",
                "CC": str(self.toolchain["cc"]),
                "CXX": str(self.toolchain["cxx"]),
                "PKG_CONFIG": str(self.toolchain["pkg_config"]),
                "PKG_CONFIG_PATH": "",
                "PKG_CONFIG_LIBDIR": os.pathsep.join(
                    str(self.prefix / p) for p in ("lib/pkgconfig", "share/pkgconfig")
                ),
                "CMAKE_PREFIX_PATH": str(self.prefix),
                "NINJA": str(
                    self.tools / "bin" / ("ninja.exe" if self.windows else "ninja")
                ),
            }
        )
        self.cmake_platform = []
        if args.target.endswith("apple-darwin"):
            self.environment["MACOSX_DEPLOYMENT_TARGET"] = args.deployment_target
            self.cmake_platform = [
                f"-DCMAKE_OSX_DEPLOYMENT_TARGET={args.deployment_target}",
                "-DCMAKE_INSTALL_NAME_DIR=@rpath",
                "-DCMAKE_INSTALL_RPATH=@loader_path",
            ]
        elif not self.windows:
            self.cmake_platform = [
                "-DCMAKE_BUILD_RPATH_USE_ORIGIN=ON",
                "-DCMAKE_INSTALL_RPATH=$ORIGIN",
            ]
        self.record = {
            "target": args.target,
            "deployment_target": args.deployment_target,
            "steps": [],
        }

    def run(self, name, command, cwd=None, environment=None):
        command = [str(part) for part in command]
        step = {"name": name, "command": command}
        self.record["steps"].append(step)
        print(f"Building {name}", flush=True)
        with (self.output / f"{name}.log").open("w", encoding="utf-8") as log:
            result = subprocess.run(
                command,
                cwd=cwd or self.output,
                env=environment or self.environment,
                stdout=log,
                stderr=subprocess.STDOUT,
                check=False,
            )
        step["exit_code"] = result.returncode
        (self.output / "build-state.json").write_text(
            json.dumps(self.record, indent=2) + "\n", encoding="utf-8"
        )
        result.check_returncode()

    def posix_path(self, path):
        if not self.windows:
            return path.as_posix()
        return subprocess.check_output(
            [self.toolchain["shell"].with_name("cygpath.exe"), "-u", str(path)],
            env=self.environment,
            text=True,
        ).strip()

    def cmake(self, name, options, *, bootstrap=False):
        directory = self.output / "build" / name
        prefix = self.tools if bootstrap else self.prefix
        generator = (
            ("NMake Makefiles" if self.windows else "Unix Makefiles")
            if bootstrap
            else "Ninja"
        )
        make = self.bootstrap_make if bootstrap else self.environment["NINJA"]
        self.run(
            name + "-configure",
            [
                self.toolchain["cmake"],
                "-S",
                self.sources[name],
                "-B",
                directory,
                "-G",
                generator,
                f"-DCMAKE_MAKE_PROGRAM={make}",
                "-DCMAKE_BUILD_TYPE=Release",
                f"-DCMAKE_INSTALL_PREFIX={prefix}",
                "-DCMAKE_INSTALL_LIBDIR=lib",
                f"-DCMAKE_C_COMPILER={self.toolchain['cc']}",
                f"-DCMAKE_CXX_COMPILER={self.toolchain['cxx']}",
                f"-DCMAKE_PREFIX_PATH={self.prefix}",
                "-DCMAKE_FIND_USE_PACKAGE_REGISTRY=OFF",
                "-DCMAKE_FIND_USE_SYSTEM_PACKAGE_REGISTRY=OFF",
                "-DCMAKE_FIND_USE_CMAKE_ENVIRONMENT_PATH=OFF",
                "-DFETCHCONTENT_FULLY_DISCONNECTED=ON",
                *self.cmake_platform,
                *options,
            ],
        )
        self.run(
            name + "-build",
            [
                self.toolchain["cmake"],
                "--build",
                directory,
                "--parallel",
                self.args.jobs,
            ],
        )
        self.run(name + "-install", [self.toolchain["cmake"], "--install", directory])

    def meson(self, name, options):
        directory = self.output / "build" / name
        meson = [sys.executable, self.sources["meson"] / "meson.py"]
        quote = subprocess.list2cmdline if self.windows else shlex.join
        include = quote([f"{'/I' if self.windows else '-I'}{self.prefix / 'include'}"])
        link = (
            [f"/LIBPATH:{self.prefix / 'lib'}"]
            if self.windows
            else [f"-L{self.prefix / 'lib'}", f"-Wl,-rpath,{self.prefix / 'lib'}"]
        )
        if self.args.target.endswith("unknown-linux-gnu"):
            link[-1] = "-Wl,-rpath,$ORIGIN:$ORIGIN/.."
        self.environment.update(
            {"CFLAGS": include, "CXXFLAGS": include, "LDFLAGS": quote(link)}
        )
        self.run(
            name + "-configure",
            [
                *meson,
                "setup",
                directory,
                self.sources[name],
                f"--prefix={self.prefix}",
                "--libdir=lib",
                "--buildtype=release",
                "--wrap-mode=nofallback",
                "-Dauto_features=disabled",
                "-Ddefault_library=shared",
                "-Dpkgconfig.relocatable=true",
                *options,
            ],
        )
        self.run(
            name + "-build", [*meson, "compile", "-C", directory, "-j", self.args.jobs]
        )
        self.run(
            name + "-install", [*meson, "install", "-C", directory, "--no-rebuild"]
        )

    def build(self):
        self.output.mkdir()
        manifest = MANIFEST.read_bytes()
        prepare_sources(self.args.archives, self.output / "sources", manifest)
        self.sources = {
            s.name: self.output / "sources" / s.root for s in load_sources(manifest)
        }
        self.record["manifest_sha256"] = hashlib.sha256(manifest).hexdigest()
        self.record["tools"] = {
            name: str(path) for name, path in self.toolchain.items()
        }
        self.record["python"] = sys.version
        self.run("cmake-version", [self.toolchain["cmake"], "--version"])
        self.run("pkg-config-version", [self.toolchain["pkg_config"], "--version"])
        self.cmake("ninja", ["-DBUILD_TESTING=OFF"], bootstrap=True)
        self.cmake(
            "zlib",
            [
                "-DZLIB_BUILD_TESTING=OFF",
                "-DZLIB_BUILD_SHARED=ON",
                "-DZLIB_BUILD_STATIC=OFF",
            ],
        )
        self.cmake(
            "pcre2",
            [
                "-DBUILD_SHARED_LIBS=ON",
                "-DBUILD_STATIC_LIBS=OFF",
                "-DPCRE2_BUILD_TESTS=OFF",
                "-DPCRE2_BUILD_PCRE2GREP=OFF",
                "-DPCRE2_SUPPORT_LIBZ=OFF",
                "-DPCRE2_SUPPORT_LIBBZ2=OFF",
                "-DPCRE2_SUPPORT_LIBREADLINE=OFF",
                "-DPCRE2_SUPPORT_LIBEDIT=OFF",
            ],
        )
        ffi_build = self.output / "build/libffi"
        ffi_build.mkdir()
        environment = self.environment.copy()
        configure_options = []
        if self.windows:
            wrapper = shlex.quote(self.posix_path(self.sources["libffi"] / "msvcc.sh"))
            architecture = (
                "-m64" if self.args.target.startswith("x86_64-") else "-marm64"
            )
            # Match upstream's MSVC recipe, including native ARM64 outputs
            # from x64-emulated Cygwin tools. Never infer the target from uname.
            host = self.args.target.partition("-")[0] + "-w64-mingw32"
            configure_options = [f"--build={host}", f"--host={host}"]
            automake = subprocess.check_output(
                [
                    self.toolchain["shell"],
                    "--noprofile",
                    "--norc",
                    "-c",
                    "automake-1.18 --print-libdir",
                ],
                env=environment,
                text=True,
            ).strip()
            environment.update(
                {
                    "CC": f"{wrapper} {architecture}",
                    "CXX": f"{wrapper} {architecture}",
                    "AR": f"{shlex.quote(automake + '/ar-lib')} lib",
                    "RANLIB": ":",
                    "LD": "link",
                    "NM": "dumpbin -symbols",
                    "STRIP": ":",
                    "LDFLAGS": "-no-undefined",
                    # Libffi clears MAKEOVERRIDES. Use its recursion hook to
                    # name the import library expected by libtool's installer.
                    "AM_MAKEFLAGS": shlex.quote(
                        "LTLDFLAGS=-no-undefined -Wc,-link,/IMPLIB:.libs/libffi.lib"
                    ),
                    "CPP": "cl -nologo -EP",
                    "CXXCPP": "cl -nologo -EP",
                    "CPPFLAGS": "-DFFI_BUILDING_DLL",
                    "CONFIG_SHELL": self.posix_path(self.toolchain["shell"]),
                }
            )
        self.run(
            "libffi-configure",
            [
                self.toolchain["shell"],
                self.posix_path(self.sources["libffi"] / "configure"),
                f"--prefix={self.posix_path(self.prefix)}",
                "--enable-shared",
                "--disable-static",
                "--disable-docs",
                *configure_options,
            ],
            cwd=ffi_build,
            environment=environment,
        )
        self.run(
            "libffi-build",
            [self.toolchain["make"], f"-j{self.args.jobs}"],
            cwd=ffi_build,
            environment=environment,
        )
        self.run(
            "libffi-install",
            [self.toolchain["make"], "install"],
            cwd=ffi_build,
            environment=environment,
        )
        if self.windows:
            self.run(
                "libffi-pkg-config",
                [self.toolchain["pkg_config"], "--cflags", "--libs", "libffi"],
            )
            if "/cygdrive/" in (self.output / "libffi-pkg-config.log").read_text():
                raise ValueError("pkg-config did not relocate libffi to native paths")
        self.cmake(
            "opus",
            [
                "-DOPUS_BUILD_SHARED_LIBRARY=ON",
                "-DOPUS_BUILD_TESTING=OFF",
                "-DOPUS_BUILD_PROGRAMS=OFF",
                # Windows ARM64 guarantees NEON; upstream misses its ARM64 spelling.
                *(
                    ["-DOPUS_PRESUME_NEON=ON"]
                    if self.args.target == "aarch64-pc-windows-msvc"
                    else []
                ),
            ],
        )
        self.meson("proxy-libintl", [])
        self.meson(
            "glib",
            [
                "-Dtests=false",
                "-Dinstalled_tests=false",
                "-Dnls=disabled",
                "-Ddocumentation=false",
                "-Dintrospection=disabled",
                "-Dlibmount=disabled",
                "-Dselinux=disabled",
                "-Dxattr=false",
            ],
        )
        self.meson(
            "gstreamer",
            [
                "-Dregistry=false",
                "-Doption-parsing=false",
                "-Dtracer_hooks=false",
                "-Dgst_parse=false",
                "-Dtools=disabled",
                "-Dptp-helper=disabled",
            ],
        )
        self.meson(
            "gst-plugins-base",
            [
                "-Dapp=enabled",
                "-Daudioconvert=enabled",
                "-Daudioresample=enabled",
                "-Dopus=enabled",
                "-Dgl=disabled",
            ],
        )
        self.meson("gst-plugins-good", ["-Drtp=enabled", "-Drtpmanager=enabled"])
        (self.output / "built.json").write_text(
            json.dumps(self.record, indent=2) + "\n", encoding="utf-8"
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in (
        "archives",
        "output",
        "cc",
        "cxx",
        "cmake",
        "make",
        "pkg-config",
        "shell",
    ):
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument(
        "--bootstrap-make",
        type=Path,
        help="NMake on Windows; defaults to --make elsewhere",
    )
    parser.add_argument("--target", required=True)
    parser.add_argument(
        "--deployment-target",
        help="Required on macOS; use the existing supported release minimum",
    )
    parser.add_argument("--jobs", type=int, default=8)
    args = parser.parse_args()
    if sys.version_info < (3, 12) or args.jobs < 1:
        parser.error("Python 3.12+ and a positive --jobs value are required")
    NativeBuild(args, os.environ).build()


if __name__ == "__main__":
    main()
