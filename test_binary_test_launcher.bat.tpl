@echo off
setlocal EnableExtensions EnableDelayedExpansion

call :resolve_runfile test_bin "__TEST_BIN__"
if errorlevel 1 exit /b 1

"%test_bin%" %*
exit /b %ERRORLEVEL%

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
