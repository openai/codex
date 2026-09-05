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
# Compiler, SDK and linker inputs remain supplied by Bazel's target toolchain.
