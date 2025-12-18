param(
    [switch]$Check
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$changelogPath = Join-Path $repoRoot "CHANGELOG.md"
$configPath = Join-Path $repoRoot "cliff.toml"

function Require-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing required command: $Name"
    }
}

Require-Command git
Require-Command git-cliff

if (-not (Test-Path $changelogPath)) {
    throw "CHANGELOG.md not found at $changelogPath"
}

$text = Get-Content -Raw -Path $changelogPath
$newline = if ($text -match "`r`n") { "`r`n" } else { "`n" }

$pattern = '<!-- BEGIN GENERATED DETAILS: range=(?<range>[^ ]+) -->\s*(?<content>.*?)\s*<!-- END GENERATED DETAILS -->'
$matches = [regex]::Matches($text, $pattern, [System.Text.RegularExpressions.RegexOptions]::Singleline)
if ($matches.Count -eq 0) {
    throw "No generated details blocks found in CHANGELOG.md."
}

$updated = [regex]::Replace($text, $pattern, {
    param($match)
    $range = $match.Groups["range"].Value
    $details = & git-cliff -c $configPath -- $range | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "git-cliff failed for range $range"
    }
    $details = $details -replace "\r\n|\r|\n", $newline
    $details = $details.Trim()
    if ([string]::IsNullOrWhiteSpace($details)) {
        $details = "_No fork-only changes yet._"
    }
    return "<!-- BEGIN GENERATED DETAILS: range=$range -->$newline$details$newline<!-- END GENERATED DETAILS -->"
}, [System.Text.RegularExpressions.RegexOptions]::Singleline)

if ($updated -eq $text) {
    if ($Check) {
        Write-Host "CHANGELOG.md is up to date."
    } else {
        Write-Host "No changelog updates needed."
    }
    exit 0
}

if ($Check) {
    Write-Host "CHANGELOG.md is out of date. Run scripts/gen-changelog.ps1."
    exit 1
}

Set-Content -Path $changelogPath -Value $updated -NoNewline
Write-Host "Updated CHANGELOG.md"
