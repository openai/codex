@echo off
setlocal EnableExtensions EnableDelayedExpansion

call :resolve_runfile binary "__BINARY__"
if errorlevel 1 exit /b 1

__RUNFILE_ENV_EXPORTS__

"%binary%" %*
exit /b %ERRORLEVEL%

:resolve_runfile
setlocal EnableExtensions EnableDelayedExpansion
set "logical_path=%~2"
set "native_logical_path=%logical_path:/=\%"
set "workspace_name=__WORKSPACE_NAME__"

for %%R in ("%RUNFILES_DIR%" "%TEST_SRCDIR%" "%~f0.runfiles") do (
  set "runfiles_root=%%~R"
  if defined runfiles_root (
    for %%P in ("!native_logical_path!" "%TEST_WORKSPACE%\!native_logical_path!" "!workspace_name!\!native_logical_path!" "_main\!native_logical_path!") do (
      if exist "!runfiles_root!\%%~P" (
        endlocal & set "%~1=!runfiles_root!\%%~P" & exit /b 0
      )
    )
  )
)

set "manifest=%RUNFILES_MANIFEST_FILE%"
if not defined manifest if exist "%~f0.runfiles_manifest" set "manifest=%~f0.runfiles_manifest"
if not defined manifest if exist "%~f0.exe.runfiles_manifest" set "manifest=%~f0.exe.runfiles_manifest"

if defined manifest if exist "%manifest%" (
  for /f "usebackq tokens=1,* delims= " %%A in ("%manifest%") do (
    if "%%A"=="%logical_path%" (
      endlocal & set "%~1=%%B" & exit /b 0
    )
    if "%%A"=="%TEST_WORKSPACE%/%logical_path%" (
      endlocal & set "%~1=%%B" & exit /b 0
    )
    if "%%A"=="%workspace_name%/%logical_path%" (
      endlocal & set "%~1=%%B" & exit /b 0
    )
    if "%%A"=="_main/%logical_path%" (
      endlocal & set "%~1=%%B" & exit /b 0
    )
  )
)

>&2 echo failed to resolve runfile: %logical_path%
endlocal & exit /b 1
