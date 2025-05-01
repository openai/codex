#!/usr/bin/env pwsh
# PowerShell 7.5
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Get the script's directory and navigate to the parent directory.
$scriptDir = Split-Path $MyInvocation.MyCommand.Definition -Parent
Push-Location (Join-Path $scriptDir '..')
try {
    & npm install
    & npm run build
    Remove-Item ./dist/openai-codex-*.tgz -Recurse -Force -ErrorAction Ignore
    & npm pack --pack-destination ./dist
    Move-Item ./dist/openai-codex-*.tgz ./dist/codex.tgz -Force

    # Dockerfile Patch to Resolve Permission Issues on Windows
    Copy-Item -Path ./Dockerfile -Destination ./Dockerfile.windows -Force
    echo 'RUN git config --global --add safe.directory /app' >> ./Dockerfile.windows

    & docker build -t codex -f './Dockerfile.windows' .
}
finally {
    Pop-Location
}
