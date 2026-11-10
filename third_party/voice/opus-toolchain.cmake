# CMake's generator must use the Ninja declared in the Bazel action.
set(CMAKE_MAKE_PROGRAM "$ENV{CODEX_VOICE_NINJA}" CACHE FILEPATH "" FORCE)
# Archiver paths from Bazel are relative to the execution root.
get_filename_component(CMAKE_AR "$ENV{AR}" ABSOLUTE
    BASE_DIR "${CMAKE_CURRENT_LIST_DIR}/../..")
if("$ENV{TARGET}" MATCHES "apple-darwin$")
  set(CMAKE_SYSTEM_NAME Darwin)
elseif("$ENV{TARGET}" MATCHES "windows")
  set(CMAKE_SYSTEM_NAME Windows)
else()
  set(CMAKE_SYSTEM_NAME Linux)
endif()
string(REGEX REPLACE "-.*" "" CMAKE_SYSTEM_PROCESSOR "$ENV{TARGET}")
if("$ENV{TARGET}" STREQUAL "aarch64-pc-windows-msvc")
  # NEON is guaranteed on Windows ARM64. Opus's optional runtime probe emits
  # ARM32 instructions through MSVC's __emit, which clang-cl does not support.
  set(OPUS_PRESUME_NEON ON CACHE BOOL "" FORCE)
  set(OPUS_MAY_HAVE_NEON OFF CACHE BOOL "" FORCE)
endif()
# Compiler, SDK and linker inputs remain supplied by Bazel's target toolchain.
