#!/usr/bin/env bash

# Remove compiler and SDK state before any Windows Bazel command starts or
# reuses a server. Callers cannot extend the fixed policy or supply replacement
# tools; the only value written is Bazel's path to the existing workflow shell.
sanitize_bazel_windows_environment() {
  if [[ $# -ne 0 ]]; then
    echo "sanitize_bazel_windows_environment does not accept arguments." >&2
    return 2
  fi
  if [[ "${RUNNER_OS:-}" != "Windows" ]]; then
    return
  fi
  if [[ -z "${ProgramFiles:-}" ]]; then
    echo "ProgramFiles must be set for the Windows Bazel shell substrate." >&2
    return 1
  fi

  unset \
    AR ARFLAGS AS BAZEL_LLVM BAZEL_LLVM_COV BAZEL_LLVM_PROFDATA BAZEL_VC \
    BAZEL_VC_FULL_VERSION BAZEL_VS BAZEL_WINSDK_FULL_VERSION CC CFLAGS CL \
    CMAKE_GENERATOR CMAKE_GENERATOR_PLATFORM CMAKE_GENERATOR_TOOLSET \
    CMAKE_TOOLCHAIN_FILE CPPFLAGS CXX CXXFLAGS DevEnvDir DLLTOOL \
    ExtensionSdkDir FrameworkDir FrameworkDir32 FrameworkVersion \
    FrameworkVersion32 INCLUDE LD LDFLAGS LIB LIBPATH LINK NETFXSDKDir NASM \
    NM OBJCOPY OBJDUMP RANLIB RC RUSTC RUSTDOC RUSTFLAGS STRIP UCRTVersion \
    UniversalCRTSdkDir USE_CLANG_CL VCIDEInstallDir VCINSTALLDIR \
    VCToolsInstallDir VCToolsRedistDir VisualStudioVersion VSCMD_ARG_HOST_ARCH \
    VSCMD_ARG_TGT_ARCH VSCMD_START_DIR VSCMD_VER VSINSTALLDIR WINDRES \
    WindowsLibPath WindowsSdkBinPath WindowsSdkDir WindowsSDKLibVersion \
    WindowsSDKVersion WindowsSdkVerBinPath YASM _CL_ _LINK_

  # MSYS preserves the spelling of exported variable names. Clear alternate
  # casing and target-qualified compiler variables without accepting a list
  # from the caller.
  mapfile -t client_environment_names < <(compgen -e)
  for environment_name in "${client_environment_names[@]}"; do
    uppercase_environment_name="${environment_name^^}"
    case "${uppercase_environment_name}" in
      AR | ARFLAGS | AS | BAZEL_LLVM* | BAZEL_VC | BAZEL_VC_FULL_VERSION | \
        BAZEL_VS | BAZEL_WINSDK_FULL_VERSION | CC | CFLAGS | CL | CMAKE | \
        CMAKE_* | CPPFLAGS | CXX | CXXFLAGS | DLLTOOL | GO | INCLUDE | LD | \
        LDFLAGS | LIB | LIBPATH | LINK | LLVM_* | MAKE | MESON | MINGW* | \
        MSVC* | NASM | NINJA | NM | OBJCOPY | OBJDUMP | PERL | PERL5LIB | \
        PKG_CONFIG | PROTOC | RANLIB | RC | RUSTC | RUSTDOC | RUSTFLAGS | \
        STRIP | UCRTVERSION | UNIVERSALCRTSDKDIR | USE_CLANG_CL | \
        VCIDEINSTALLDIR | VCINSTALLDIR | VCTOOLS* | VISUALSTUDIOVERSION | \
        VSCMD_* | VSINSTALLDIR | WINDRES | WINDOWSLIBPATH | WINDOWSSDK* | \
        YASM | _CL_ | _LINK_ | CARGO_BUILD_TARGET | CARGO_TARGET_*_LINKER | \
        AR_* | *_AR | AS_* | *_AS | CC_* | *_CC | CFLAGS_* | *_CFLAGS | \
        CXX_* | *_CXX | CXXFLAGS_* | *_CXXFLAGS | LD_* | *_LD)
        unset "${environment_name}"
        ;;
    esac
  done

  path_separator=:
  if [[ "${PATH}" == *";"* ]]; then
    path_separator=";"
  fi
  IFS="${path_separator}" read -r -a client_path_entries <<< "${PATH}"
  sanitized_client_path=""
  for path_entry in "${client_path_entries[@]}"; do
    [[ -n "${path_entry}" ]] || continue
    normalized_path_entry="${path_entry//\\//}"
    normalized_path_entry="${normalized_path_entry,,}"
    case "${normalized_path_entry}/" in
      *"/microsoft visual studio/"* | \
        *"/windows kits/"* | \
        *"/microsoft sdks/"* | \
        *"/program files/llvm/"* | \
        *"/program files (x86)/llvm/"* | \
        *"/msys64/"* | \
        *"/mingw32/"* | \
        *"/mingw64/"*)
        continue
        ;;
    esac
    if [[ -z "${sanitized_client_path}" ]]; then
      sanitized_client_path="${path_entry}"
    else
      sanitized_client_path+="${path_separator}${path_entry}"
    fi
  done
  if [[ -z "${sanitized_client_path}" ]]; then
    echo "Windows Bazel client PATH is empty after removing compiler and SDK directories." >&2
    return 1
  fi
  export PATH="${sanitized_client_path}"
  export BAZEL_SH="${ProgramFiles//\\//}/Git/usr/bin/bash.exe"
}
