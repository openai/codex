<#
Bazel build actions must not inherit the hosted-runner PATH. Keep their
execution substrate fixed to Windows and the Git-for-Windows shell utilities
that Bazel genrules already use. Test actions get a separate fixed path with
the product runtimes exercised by Windows tests (Git, PowerShell, and
DotSlash). Compiler, SDK, MinGW, hosted Python, and hosted Node directories are
intentionally absent from both values.
#>

$windowsDir = if ($env:WINDIR) {
  $env:WINDIR
} elseif ($env:SystemRoot) {
  $env:SystemRoot
} else {
  throw 'WINDIR or SystemRoot must be set.'
}

if ([string]::IsNullOrWhiteSpace($env:ProgramFiles)) {
  throw 'ProgramFiles must be set.'
}
if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
  throw 'LOCALAPPDATA must be set.'
}
if ([string]::IsNullOrWhiteSpace($env:GITHUB_ENV)) {
  throw 'GITHUB_ENV must be set.'
}

$gitRoot = Join-Path $env:ProgramFiles 'Git'
$executionPathEntries = @(
  (Join-Path $gitRoot 'usr\bin'),
  (Join-Path $windowsDir 'System32'),
  $windowsDir
)
$testPathEntries = @(
  (Join-Path $env:ProgramFiles 'PowerShell\7'),
  (Join-Path $gitRoot 'bin'),
  (Join-Path $gitRoot 'usr\bin'),
  (Join-Path $env:LOCALAPPDATA 'Microsoft\WindowsApps'),
  (Join-Path $windowsDir 'System32\WindowsPowerShell\v1.0'),
  (Join-Path $windowsDir 'System32'),
  $windowsDir
)

$requiredPathEntries = ($executionPathEntries + $testPathEntries) | Select-Object -Unique
foreach ($pathEntry in $requiredPathEntries) {
  if (-not (Test-Path $pathEntry)) {
    throw "Required Windows Bazel substrate path does not exist: $pathEntry"
  }
}

$executionPath = $executionPathEntries -join ';'
$testPath = $testPathEntries -join ';'

Write-Host 'Frozen CODEX_BAZEL_WINDOWS_EXECUTION_PATH entries:'
$executionPathEntries | ForEach-Object { Write-Host "  $_" }
Write-Host 'Frozen CODEX_BAZEL_WINDOWS_TEST_PATH entries:'
$testPathEntries | ForEach-Object { Write-Host "  $_" }

"CODEX_BAZEL_WINDOWS_EXECUTION_PATH=$executionPath" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
"CODEX_BAZEL_WINDOWS_TEST_PATH=$testPath" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
