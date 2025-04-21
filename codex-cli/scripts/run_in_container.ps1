#!/usr/bin/env pwsh
# PowerShell 7.5
<#
Usage:
  .\run_in_container.ps1 [-WorkDir <directory>] -- <PROMPT>
Example:
  .\run_in_container.ps1 -WorkDir G:\Projects\demo1 -- "explain this codebase to me"
  .\run_in_container.ps1
#>

param(
    [string]$WorkDir = (Get-Location).ProviderPath,
    [Parameter(ValueFromRemainingArguments)] [string[]]$Args
)

$WorkDir = (Get-Item $WorkDir).FullName
$containerName = 'codex_' + ($WorkDir -replace '[\\/]', '_' -replace '[^0-9A-Za-z_-]', '')

# Check whether OPENAI_API_KEY has been set in environment variables.
if (-not $env:OPENAI_API_KEY) {
    Write-Error 'An OPENAI_API_KEY environment variable must be provided.'
    exit 1
}

function Cleanup {
    docker rm -f $containerName 2> $null
}

function Quote-BashArg {
    param([string]$Value)
    return "'$($Value -replace "'", "''\'''")'"
}

try {
    Cleanup  # Remove old container

    docker run --name $containerName -d `
        -e OPENAI_API_KEY `
        --cap-add=NET_ADMIN --cap-add=NET_RAW `
        -v "${WorkDir}:/app" `
        codex sleep infinity | Out-Null

    # docker exec $containerName bash -c 'sudo /usr/local/bin/init_firewall.sh'

    $quoted = ($Args | ForEach-Object { Quote-BashArg $_ }) -join ' '

    docker exec -it $containerName bash -c "cd '/app' && codex --full-auto $quoted"
}
finally {
    Register-EngineEvent PowerShell.Exiting -Action { Cleanup } | Out-Null
}
