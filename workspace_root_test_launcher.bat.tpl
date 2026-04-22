@echo off
setlocal EnableExtensions EnableDelayedExpansion

call :resolve_runfile workspace_root_marker "__WORKSPACE_ROOT_MARKER__"
if errorlevel 1 exit /b 1

for %%I in ("%workspace_root_marker%") do set "workspace_root_marker_dir=%%~dpI"
for %%I in ("%workspace_root_marker_dir%..\..") do set "workspace_root=%%~fI"

call :resolve_runfile test_bin "__TEST_BIN__"
if errorlevel 1 exit /b 1

set "INSTA_WORKSPACE_ROOT=%workspace_root%"
cd /d "%workspace_root%" || exit /b 1

set "TOTAL_SHARDS=%RULES_RUST_TEST_TOTAL_SHARDS%"
if not defined TOTAL_SHARDS set "TOTAL_SHARDS=%TEST_TOTAL_SHARDS%"
if defined TOTAL_SHARDS if not "%TOTAL_SHARDS%"=="0" (
  call :run_sharded_libtest %*
  exit /b !ERRORLEVEL!
)

"%test_bin%" %*
exit /b %ERRORLEVEL%

:run_sharded_libtest
if defined TEST_SHARD_STATUS_FILE if defined TEST_TOTAL_SHARDS if not "%TEST_TOTAL_SHARDS%"=="0" (
  type nul > "%TEST_SHARD_STATUS_FILE%"
)

if not "%~1"=="" (
  "%test_bin%" %*
  exit /b !ERRORLEVEL!
)

set "SHARD_INDEX=%RULES_RUST_TEST_SHARD_INDEX%"
if not defined SHARD_INDEX set "SHARD_INDEX=%TEST_SHARD_INDEX%"
if not defined SHARD_INDEX (
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
  "$totalShards = [uint32]$env:TOTAL_SHARDS; $shardIndex = [uint32]$env:SHARD_INDEX;" ^
  "$fnvPrime = [uint64]16777619; $u32Mask = [uint64]4294967295;" ^
  "foreach ($test in $tests) { $hash = [uint32]2166136261; foreach ($byte in [Text.Encoding]::UTF8.GetBytes($test)) { $hash = [uint32](([uint64]($hash -bxor $byte) * $fnvPrime) -band $u32Mask) }; if (($hash %% $totalShards) -eq $shardIndex) { $test } }" ^
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
setlocal EnableExtensions EnableDelayedExpansion
set "logical_path=%~2"
set "workspace_logical_path=%logical_path%"
if defined TEST_WORKSPACE set "workspace_logical_path=%TEST_WORKSPACE%/%logical_path%"
set "native_logical_path=%logical_path:/=\%"
set "native_workspace_logical_path=%workspace_logical_path:/=\%"

for %%R in ("%RUNFILES_DIR%" "%TEST_SRCDIR%") do (
  set "runfiles_root=%%~R"
  if defined runfiles_root (
    if exist "!runfiles_root!\!native_logical_path!" (
      endlocal & set "%~1=!runfiles_root!\!native_logical_path!" & exit /b 0
    )
    if exist "!runfiles_root!\!native_workspace_logical_path!" (
      endlocal & set "%~1=!runfiles_root!\!native_workspace_logical_path!" & exit /b 0
    )
  )
)

set "manifest=%RUNFILES_MANIFEST_FILE%"
if not defined manifest if exist "%~f0.runfiles_manifest" set "manifest=%~f0.runfiles_manifest"
if not defined manifest if exist "%~dpn0.runfiles_manifest" set "manifest=%~dpn0.runfiles_manifest"
if not defined manifest if exist "%~f0.exe.runfiles_manifest" set "manifest=%~f0.exe.runfiles_manifest"

if defined manifest if exist "%manifest%" (
  rem Read the manifest directly instead of shelling out to findstr. In the
  rem GitHub Windows runner, the nested `findstr` path produced
  rem `FINDSTR: Cannot open D:MANIFEST`, which then broke runfile resolution for
  rem Bazel tests even though the manifest file was present.
  for /f "usebackq tokens=1,* delims= " %%A in ("%manifest%") do (
    if "%%A"=="%logical_path%" (
      endlocal & set "%~1=%%B" & exit /b 0
    )
    if "%%A"=="%workspace_logical_path%" (
      endlocal & set "%~1=%%B" & exit /b 0
    )
  )
)

>&2 echo failed to resolve runfile: %logical_path%
endlocal & exit /b 1
