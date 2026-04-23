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
    $pathEntry -eq 'D:\a\_temp\install-dotslash\bin' -or
    ($windowsDir -and ($pathEntry -eq $windowsDir -or $pathEntry -like "${windowsDir}\*")) -or
    ($windowsAppsPath -and $pathEntry -eq $windowsAppsPath)
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
"CODEX_BAZEL_WINDOWS_PATH=$stablePath" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
