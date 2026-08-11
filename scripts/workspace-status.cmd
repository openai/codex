@echo off
setlocal

if defined STABLE_GIT_COMMIT (
  echo STABLE_GIT_COMMIT %STABLE_GIT_COMMIT%
  exit /b 0
)

for /f "delims=" %%I in ('git rev-parse --verify HEAD 2^>nul') do set "BUILD_COMMIT=%%I"
if defined BUILD_COMMIT (
  echo STABLE_GIT_COMMIT %BUILD_COMMIT%
  exit /b 0
)

if defined GITHUB_SHA (
  echo STABLE_GIT_COMMIT %GITHUB_SHA%
  exit /b 0
)

echo STABLE_GIT_COMMIT unknown
