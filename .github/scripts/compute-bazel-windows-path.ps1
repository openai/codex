<#
BuildBuddy cache keys include the action and test environment, so Bazel should
not inherit the full hosted-runner PATH on Windows. That PATH includes volatile
tool entries, such as Maven, that can change independently of this repo and
cause avoidable cache misses.

This script derives a smaller, cache-stable PATH that keeps the Windows
toolchain entries Bazel-backed CI tasks need: MSVC and Windows SDK paths, Git,
PowerShell, Node, Python, DotSlash, and the standard Windows system
directories.
`setup-bazel-ci` runs this after exporting the MSVC environment, and the script
publishes the result via `GITHUB_ENV` as `CODEX_BAZEL_WINDOWS_PATH` so later
steps can pass that explicit PATH to Bazel.
#>

$stablePathEntries = New-Object System.Collections.Generic.List[string]
$seenEntries = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
$windowsAppsPath = if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
  $null
} else {
  "$($env:LOCALAPPDATA)\Microsoft\WindowsApps"
}
$windowsDir = if ($env:WINDIR) {
  $env:WINDIR
} elseif ($env:SystemRoot) {
  $env:SystemRoot
} else {
  $null
}

function Add-StablePathEntry {
  param([string]$PathEntry)

  if ([string]::IsNullOrWhiteSpace($PathEntry)) {
    return
  }

  if ($seenEntries.Add($PathEntry)) {
    [void]$stablePathEntries.Add($PathEntry)
  }
}

foreach ($pathEntry in ($env:PATH -split ';')) {
  if ([string]::IsNullOrWhiteSpace($pathEntry)) {
    continue
  }

  if (
    $pathEntry -like '*Microsoft Visual Studio*' -or
    $pathEntry -like '*Windows Kits*' -or
    $pathEntry -like '*Microsoft SDKs*' -or
    $pathEntry -like 'C:\Program Files\Git\*' -or
    $pathEntry -like 'C:\Program Files\PowerShell\*' -or
    $pathEntry -like 'C:\hostedtoolcache\windows\node\*' -or
    $pathEntry -like 'C:\hostedtoolcache\windows\Python\*' -or
    $pathEntry -eq 'D:\a\_temp\install-dotslash\bin' -or
    ($windowsDir -and ($pathEntry -eq $windowsDir -or $pathEntry -like "${windowsDir}\*"))
  ) {
    Add-StablePathEntry $pathEntry
  }
}

$gitCommand = Get-Command git -ErrorAction SilentlyContinue
if ($gitCommand) {
  Add-StablePathEntry (Split-Path $gitCommand.Source -Parent)
}

$nodeCommand = Get-Command node -ErrorAction SilentlyContinue
if ($nodeCommand) {
  Add-StablePathEntry (Split-Path $nodeCommand.Source -Parent)
}

$python3Command = Get-Command python3 -ErrorAction SilentlyContinue
if ($python3Command) {
  Add-StablePathEntry (Split-Path $python3Command.Source -Parent)
}

$pythonCommand = Get-Command python -ErrorAction SilentlyContinue
if ($pythonCommand) {
  Add-StablePathEntry (Split-Path $pythonCommand.Source -Parent)
}

$pwshCommand = Get-Command pwsh -ErrorAction SilentlyContinue
if ($pwshCommand) {
  Add-StablePathEntry (Split-Path $pwshCommand.Source -Parent)
}

if ($windowsAppsPath) {
  Add-StablePathEntry $windowsAppsPath
}

if ($stablePathEntries.Count -eq 0) {
  throw 'Failed to derive cache-stable Windows PATH.'
}

if ([string]::IsNullOrWhiteSpace($env:GITHUB_ENV)) {
  throw 'GITHUB_ENV must be set.'
}

$stablePath = $stablePathEntries -join ';'
Write-Host "Derived CODEX_BAZEL_WINDOWS_PATH: $stablePath"
"CODEX_BAZEL_WINDOWS_PATH=$stablePath" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
