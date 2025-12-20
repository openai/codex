param(
    [switch]$Check
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$changelogPath = Join-Path $repoRoot "CHANGELOG.md"

function Require-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing required command: $Name"
    }
}

Require-Command git

if (-not (Test-Path $changelogPath)) {
    throw "CHANGELOG.md not found at $changelogPath"
}

$text = Get-Content -Raw -Path $changelogPath
$newline = if ($text -match "`r`n") { "`r`n" } else { "`n" }

$hasUpstream = $false
& git rev-parse --verify --quiet upstream/main | Out-Null
if ($LASTEXITCODE -eq 0) {
    $hasUpstream = $true
}

function Get-GroupForSubject([string]$Subject) {
    if ($Subject -match '^feat') { return "Features" }
    if ($Subject -match '^fix') { return "Fixes" }
    if ($Subject -match '^docs') { return "Documentation" }
    if ($Subject -match '^tui') { return "TUI" }
    if ($Subject -match '^core') { return "Core" }
    if ($Subject -match '^plan' -or $Subject -match '(?i)\bplan\b|plan mode') { return "Plan Mode" }
    if ($Subject -match '(?i)rebrand|codexel|@ixe1/codexel') { return "Branding & Packaging" }
    if ($Subject -match '^(chore|build|ci)') { return "Chores" }
    return "Other"
}

function Render-Details([string]$Range) {
    $revArgs = @("rev-list", "--reverse", $Range)
    if ($hasUpstream) {
        $revArgs += @("--not", "upstream/main")
    }
    $shas = & git @revArgs
    if ($LASTEXITCODE -ne 0) {
        throw "git rev-list failed for range $Range"
    }

    if (-not $shas -or $shas.Count -eq 0) {
        return ""
    }

    $groups = [ordered]@{
        "Features"              = @()
        "Fixes"                 = @()
        "Documentation"         = @()
        "TUI"                   = @()
        "Core"                  = @()
        "Plan Mode"             = @()
        "Branding & Packaging"  = @()
        "Chores"                = @()
        "Other"                 = @()
    }

    foreach ($sha in $shas) {
        $subject = (& git show -s --format=%s $sha).TrimEnd()
        if ([string]::IsNullOrWhiteSpace($subject)) {
            continue
        }

        $body = (& git show -s --format=%B $sha) -replace "\r\n|\r|\n", "`n"
        $body = $body.TrimEnd()
        if ([string]::IsNullOrWhiteSpace($body)) {
            continue
        }

        $group = Get-GroupForSubject $subject
        $lines = $body -split "`n", -1
        $lines[0] = "- " + $lines[0]
        $entry = ($lines -join $newline).TrimEnd()
        $groups[$group] += $entry
    }

    $out = @()
    foreach ($kvp in $groups.GetEnumerator()) {
        if ($kvp.Value.Count -eq 0) {
            continue
        }
        $out += "#### $($kvp.Key)"
        $out += $kvp.Value
        $out += ""
    }

    return ($out -join $newline).Trim()
}

$pattern = '<!-- BEGIN GENERATED DETAILS: range=(?<range>[^ ]+) -->\s*(?<content>.*?)\s*<!-- END GENERATED DETAILS -->'
$matches = [regex]::Matches($text, $pattern, [System.Text.RegularExpressions.RegexOptions]::Singleline)
if ($matches.Count -eq 0) {
    throw "No generated details blocks found in CHANGELOG.md."
}

$updated = [regex]::Replace($text, $pattern, {
    param($match)
    $range = $match.Groups["range"].Value
    $details = Render-Details $range
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
