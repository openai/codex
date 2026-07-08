@echo off
setlocal EnableExtensions EnableDelayedExpansion

call :resolve_runfile "__WORKSPACE_ROOT_MARKER__"
if errorlevel 1 exit /b 1
set "workspace_root_marker=!resolve_runfile_result!"

for %%I in ("%workspace_root_marker%") do set "workspace_root_marker_dir=%%~dpI"
for %%I in ("%workspace_root_marker_dir%..\..") do set "workspace_root=%%~fI"

call :resolve_runfile "__TEST_BIN__"
if errorlevel 1 exit /b 1
set "test_bin=!resolve_runfile_result!"

__RUNFILE_ENV_EXPORTS__

if not defined test_bin (
  >&2 echo resolved test binary was lost while exporting runfile environment variables
  exit /b 1
)

__WORKSPACE_ROOT_SETUP__

set "TOTAL_SHARDS=%RULES_RUST_TEST_TOTAL_SHARDS%"
if not defined TOTAL_SHARDS set "TOTAL_SHARDS=%TEST_TOTAL_SHARDS%"
if defined TESTBRIDGE_TEST_ONLY if "%~1"=="" (
  "%test_bin%" "%TESTBRIDGE_TEST_ONLY%"
  exit /b !ERRORLEVEL!
)
if defined CODEX_BAZEL_TEST_SKIP_FILTERS (
  call :run_selected_libtest %*
  exit /b !ERRORLEVEL!
)
if defined TOTAL_SHARDS if not "%TOTAL_SHARDS%"=="0" (
  call :run_selected_libtest %*
  exit /b !ERRORLEVEL!
)

"%test_bin%" %*
exit /b %ERRORLEVEL%

:run_selected_libtest
if defined TEST_SHARD_STATUS_FILE if defined TEST_TOTAL_SHARDS if not "%TEST_TOTAL_SHARDS%"=="0" (
  type nul > "%TEST_SHARD_STATUS_FILE%"
)

if not "%~1"=="" (
  "%test_bin%" %*
  exit /b !ERRORLEVEL!
)

set "SHARD_INDEX=%RULES_RUST_TEST_SHARD_INDEX%"
if not defined SHARD_INDEX set "SHARD_INDEX=%TEST_SHARD_INDEX%"
set "HAS_SHARDS="
if defined TOTAL_SHARDS if not "%TOTAL_SHARDS%"=="0" set "HAS_SHARDS=1"
if defined HAS_SHARDS if not defined SHARD_INDEX (
  >&2 echo TEST_SHARD_INDEX or RULES_RUST_TEST_SHARD_INDEX must be set when sharding is enabled
  exit /b 1
)

set "TEMP_ROOT=%TEST_TMPDIR%"
if not defined TEMP_ROOT set "TEMP_ROOT=%TEMP%"
if not defined TEMP_ROOT set "TEMP_ROOT=."
:CREATE_TEMP_DIR
set "TEMP_DIR=%TEMP_ROOT%\workspace_root_test_sharding_!RANDOM!_!RANDOM!_!RANDOM!"
mkdir "!TEMP_DIR!" 2>nul
if errorlevel 1 goto :CREATE_TEMP_DIR
set "TEMP_LIST=!TEMP_DIR!\list.txt"
set "TEMP_SHARD_LIST=!TEMP_DIR!\shard.txt"

"%test_bin%" --list --format terse > "!TEMP_LIST!"
if errorlevel 1 (
  rmdir /s /q "!TEMP_DIR!" 2>nul
  exit /b 1
)

powershell.exe -NoProfile -ExecutionPolicy Bypass -Command ^
  "$ErrorActionPreference = 'Stop';" ^
  "$tests = @(Get-Content -LiteralPath $env:TEMP_LIST | Where-Object { $_.EndsWith(': test') } | ForEach-Object { $_.Substring(0, $_.Length - 6) });" ^
  "[Array]::Sort($tests, [StringComparer]::Ordinal);" ^
  "$hasShards = -not [string]::IsNullOrEmpty($env:HAS_SHARDS);" ^
  "$skipFilters = @();" ^
  "if (-not [string]::IsNullOrEmpty($env:CODEX_BAZEL_TEST_SKIP_FILTERS)) { $skipFilters = @($env:CODEX_BAZEL_TEST_SKIP_FILTERS -split ',' | Where-Object { $_ -ne '' }) };" ^
  "if ($hasShards) { $totalShards = [uint32]$env:TOTAL_SHARDS; $shardIndex = [uint32]$env:SHARD_INDEX };" ^
  "$fnvPrime = [uint64]16777619; $u32Mask = [uint64]4294967295;" ^
  "foreach ($test in $tests) { $skip = $false; foreach ($filter in $skipFilters) { if ($test.Contains($filter)) { $skip = $true; break } }; if ($skip) { continue }; if ($hasShards) { $hash = [uint32]2166136261; foreach ($byte in [Text.Encoding]::UTF8.GetBytes($test)) { $hash = [uint32](([uint64]($hash -bxor $byte) * $fnvPrime) -band $u32Mask) }; if (($hash %% $totalShards) -eq $shardIndex) { $test } } else { $test } }" ^
  > "!TEMP_SHARD_LIST!"
if errorlevel 1 (
  rmdir /s /q "!TEMP_DIR!" 2>nul
  exit /b 1
)

powershell.exe -NoProfile -ExecutionPolicy Bypass -Command ^
  "$ErrorActionPreference = 'Stop';" ^
  "$testBin = $env:test_bin;" ^
  "$tests = @(Get-Content -LiteralPath $env:TEMP_SHARD_LIST);" ^
  "$failed = $false; $limit = 7000; $batch = @(); $batchChars = $testBin.Length + 8;" ^
  "function Invoke-TestBatch { if ($script:batch.Count -eq 0) { return }; & $script:testBin @script:batch '--exact'; if ($LASTEXITCODE -ne 0) { $script:failed = $true }; $script:batch = @(); $script:batchChars = $script:testBin.Length + 8 }" ^
  "foreach ($test in $tests) { $argChars = $test.Length + 3; if (($batch.Count -gt 0) -and ($batchChars + $argChars -gt $limit)) { Invoke-TestBatch }; $batch += $test; $batchChars += $argChars }" ^
  "Invoke-TestBatch; if ($failed) { exit 1 }"
set "TEST_EXIT=%ERRORLEVEL%"

rmdir /s /q "!TEMP_DIR!" 2>nul
exit /b !TEST_EXIT!

:resolve_runfile
set "resolve_runfile_result="
set "resolve_runfile_logical_path=%~1"
set "resolve_runfile_workspace_logical_path=!resolve_runfile_logical_path!"
if defined TEST_WORKSPACE set "resolve_runfile_workspace_logical_path=%TEST_WORKSPACE%/!resolve_runfile_logical_path!"
set "resolve_runfile_native_logical_path=!resolve_runfile_logical_path:/=\!"
set "resolve_runfile_native_workspace_logical_path=!resolve_runfile_workspace_logical_path:/=\!"

for %%R in ("%RUNFILES_DIR%" "%TEST_SRCDIR%") do (
  set "resolve_runfile_root=%%~R"
  if defined resolve_runfile_root (
    if exist "!resolve_runfile_root!\!resolve_runfile_native_logical_path!" (
      set "resolve_runfile_result=!resolve_runfile_root!\!resolve_runfile_native_logical_path!"
      goto :resolve_runfile_success
    )
    if exist "!resolve_runfile_root!\!resolve_runfile_native_workspace_logical_path!" (
      set "resolve_runfile_result=!resolve_runfile_root!\!resolve_runfile_native_workspace_logical_path!"
      goto :resolve_runfile_success
    )
  )
)

set "resolve_runfile_manifest=%RUNFILES_MANIFEST_FILE%"
if not defined resolve_runfile_manifest if exist "%~f0.runfiles_manifest" set "resolve_runfile_manifest=%~f0.runfiles_manifest"
if not defined resolve_runfile_manifest if exist "%~dpn0.runfiles_manifest" set "resolve_runfile_manifest=%~dpn0.runfiles_manifest"
if not defined resolve_runfile_manifest if exist "%~f0.exe.runfiles_manifest" set "resolve_runfile_manifest=%~f0.exe.runfiles_manifest"

if defined resolve_runfile_manifest if exist "!resolve_runfile_manifest!" (
  rem Read the manifest directly instead of shelling out to findstr. In the
  rem GitHub Windows runner, the nested `findstr` path produced
  rem `FINDSTR: Cannot open D:MANIFEST`, which then broke runfile resolution for
  rem Bazel tests even though the manifest file was present.
  rem A one-field manifest entry maps to itself, so fall back to %%A when the
  rem optional mapped path in %%B is empty.
  for /f "usebackq tokens=1,* delims= " %%A in ("!resolve_runfile_manifest!") do (
    if "%%A"=="!resolve_runfile_logical_path!" (
      set "resolve_runfile_manifest_path=%%B"
      if not defined resolve_runfile_manifest_path set "resolve_runfile_manifest_path=%%A"
      set "resolve_runfile_result=!resolve_runfile_manifest_path!"
      goto :resolve_runfile_success
    )
    if "%%A"=="!resolve_runfile_workspace_logical_path!" (
      set "resolve_runfile_manifest_path=%%B"
      if not defined resolve_runfile_manifest_path set "resolve_runfile_manifest_path=%%A"
      set "resolve_runfile_result=!resolve_runfile_manifest_path!"
      goto :resolve_runfile_success
    )
  )
)

>&2 echo failed to resolve runfile: !resolve_runfile_logical_path!
call :clear_resolve_runfile_state
exit /b 1

:resolve_runfile_success
if not defined resolve_runfile_result (
  >&2 echo resolved runfile has an empty path: !resolve_runfile_logical_path!
  call :clear_resolve_runfile_state
  exit /b 1
)
call :clear_resolve_runfile_state
exit /b 0

:clear_resolve_runfile_state
set "resolve_runfile_logical_path="
set "resolve_runfile_workspace_logical_path="
set "resolve_runfile_native_logical_path="
set "resolve_runfile_native_workspace_logical_path="
set "resolve_runfile_root="
set "resolve_runfile_manifest="
set "resolve_runfile_manifest_path="
exit /b 0
