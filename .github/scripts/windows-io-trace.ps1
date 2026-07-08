param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("mark", "transition", "stop")]
    [string]$Action,

    [string]$Label,

    [string]$From,

    [string]$To
)

$ErrorActionPreference = "Stop"

function Invoke-Wpr {
    param([string[]]$Arguments)

    & wpr @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "wpr $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

switch ($Action) {
    "mark" {
        Invoke-Wpr @("-marker", $Label)
    }
    "transition" {
        Invoke-Wpr @("-marker", "step:end:$From")
        Invoke-Wpr @("-marker", "step:start:$To")
    }
    "stop" {
        Invoke-Wpr @("-marker", "step:end:$Label")

        $traceRoot = Join-Path $env:RUNNER_TEMP "codex-io-trace"
        New-Item -ItemType Directory -Force -Path $traceRoot | Out-Null
        $tracePath = Join-Path $traceRoot "windows-io-$($env:GITHUB_JOB)-$($env:GITHUB_RUN_ID)-$($env:GITHUB_RUN_ATTEMPT).etl"
        Invoke-Wpr @("-stop", $tracePath)
        Write-Output "saved Windows I/O trace to $tracePath"
    }
}
