@echo off
setlocal EnableExtensions EnableDelayedExpansion

rem Build codex-super npm package on Windows (cmd.exe)
rem Rough equivalent of scripts/build_local_codex_npm.sh for Windows hosts.

set "RELEASE_VERSION=0.0.0-local"
rem On Windows, default to host-only builds. You can override with --targets.
set "TARGETS=host"

set "SCRIPT_DIR=%~dp0"
pushd "%SCRIPT_DIR%.." >nul 2>&1
set "REPO_ROOT=%CD%"
popd >nul 2>&1
set "CODEX_RS_DIR=%REPO_ROOT%\codex-rs"

call :parse_args %*
if errorlevel 1 goto :error

set "PACK_OUTPUT=%REPO_ROOT%\dist\codex-super-%RELEASE_VERSION%.tgz"

call :require_cmd uv || goto :error
call :require_cmd cargo || goto :error
call :require_cmd rustc || goto :error
call :require_cmd gh || goto :error
call :require_cmd zstd || goto :error
call :require_cmd npm || goto :error

if not exist "%CODEX_RS_DIR%" (
  echo Error: Expected directory not found: "%CODEX_RS_DIR%"
  goto :error
)

rem Determine rust host triple
for /f "tokens=1,* delims=:" %%A in ('rustc -vV ^| findstr /b /c:"host: "') do set "HOST_TRIPLE=%%B"
rem Trim leading space
if defined HOST_TRIPLE if "!HOST_TRIPLE:~0,1!"==" " set "HOST_TRIPLE=!HOST_TRIPLE:~1!"
if not defined HOST_TRIPLE (
  echo Error: Failed to detect Rust host triple via "rustc -vV".
  goto :error
)

echo ==^> Installing latest native dependencies into vendor/
rem This script downloads prebuilt helpers (requires network tools as documented in codex-cli/scripts)
uv run "%REPO_ROOT%\codex-cli\scripts\install_native_deps.py" "%REPO_ROOT%\codex-cli"
if errorlevel 1 goto :error

rem Parse targets list
set "NEED_HOST=false"
set "NEED_MUSL=false"
for %%T in (%TARGETS:,= %) do (
  if /I "%%~T"=="host" set "NEED_HOST=true"
  if /I "%%~T"=="x86_64-unknown-linux-musl" set "NEED_MUSL=true"
)

if /I "%NEED_MUSL%"=="true" (
  echo Error: Target x86_64-unknown-linux-musl is not supported on Windows in this script.
  echo Use --targets host or run the Bash script on Linux/macOS for musl.
  goto :error
)

if /I not "%NEED_HOST%"=="true" (
  echo Error: No valid targets to build. Use --targets host
  goto :error
)

echo ==^> Building codex-cli (host release)
pushd "%CODEX_RS_DIR%" >nul 2>&1
cargo build --release -p codex-cli
set "BUILD_ERR=%ERRORLEVEL%"
popd >nul 2>&1
if not "%BUILD_ERR%"=="0" goto :error

rem Update vendor binary for host target
set "HOST_SRC=%REPO_ROOT%\codex-rs\target\release\codex.exe"
if not exist "%HOST_SRC%" (
  echo Error: Built binary not found: "%HOST_SRC%"
  goto :error
)

set "HOST_DEST_DIR=%REPO_ROOT%\codex-cli\vendor\%HOST_TRIPLE%\codex"
set "HOST_DEST=%HOST_DEST_DIR%\codex.exe"

if exist "%HOST_DEST_DIR%" (
  echo ==^> Updating vendor binary for host target (%HOST_TRIPLE%)
  copy /Y "%HOST_SRC%" "%HOST_DEST%" >nul
  if errorlevel 1 goto :error
  echo   updated %HOST_DEST%
) else (
  echo ==^> Skipping host vendor update (directory missing for %HOST_TRIPLE%)
)

rem Ensure dist directory exists
for %%D in ("%REPO_ROOT%\dist") do if not exist "%%~fD" mkdir "%%~fD"

echo ==^> Building npm package (version %RELEASE_VERSION%)
set "STAGING_DIR=%REPO_ROOT%\dist\stage\codex-super-%RELEASE_VERSION%"
if exist "%STAGING_DIR%" rmdir /S /Q "%STAGING_DIR%"
mkdir "%STAGING_DIR%" >nul 2>&1

uv run "%REPO_ROOT%\codex-cli\scripts\build_npm_package.py" ^
  --package codex-super ^
  --release-version "%RELEASE_VERSION%" ^
  --vendor-src "%REPO_ROOT%\codex-cli\vendor" ^
  --staging-dir "%STAGING_DIR%"
if errorlevel 1 goto :error

pushd "%STAGING_DIR%" >nul 2>&1
npm pack --json --pack-destination "%REPO_ROOT%\dist"
set "NPM_PACK_ERR=%ERRORLEVEL%"
popd >nul 2>&1
if not "%NPM_PACK_ERR%"=="0" goto :error

echo ==^> npm package ready: %PACK_OUTPUT%
exit /b 0

:parse_args
rem Usage: build_local_codex_npm.cmd [--targets <list>] [release-version]
if "%~1"=="" goto :eof
:parse_loop
  if "%~1"=="" goto :eof
  if /I "%~1"=="-h" goto :print_usage_ok
  if /I "%~1"=="--help" goto :print_usage_ok

  rem --targets=...
  echo %~1 | findstr /b /c:"--targets=" >nul
  if not errorlevel 1 (
    set "TARGETS=%~1"
    set "TARGETS=!TARGETS:--targets=.!"
    goto :shift_and_continue
  )

  if /I "%~1"=="--targets" (
    shift
    if "%~1"=="" (
      echo Error: --targets requires an argument.
      goto :print_usage_err
    )
    set "TARGETS=%~1"
    goto :shift_and_continue
  )

  rem First non-option is release version; only allow once
  if /I not "%RELEASE_VERSION%"=="0.0.0-local" (
    echo Error: Multiple release versions provided.
    goto :print_usage_err
  )
  set "RELEASE_VERSION=%~1"

:shift_and_continue
  shift
  goto :parse_loop

:require_cmd
where %~1 >nul 2>&1
if errorlevel 1 (
  echo Error: Required command '%~1' not found in PATH.
  exit /b 1
)
exit /b 0

:print_usage_ok
echo Usage: build_local_codex_npm.cmd [--targets ^<list^>] [release-version]
echo.
echo   --targets ^<list^>  Comma-separated targets to build. Supported values on Windows:
echo                        host
echo   release-version      Optional version string for the generated npm package.
echo                        Defaults to 0.0.0-local
exit /b 1

:print_usage_err
call :print_usage_ok
exit /b 1

:error
exit /b 1
