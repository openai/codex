# CI-only prerequisites. Never install these tools into a Codex package.
param(
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][string]$SnapshotArchive
)

$ErrorActionPreference = "Stop"
$PkgHashes = @{
    "x86_64-pc-windows-msvc" = @("x64", "5604cf25ef38bb6a09520cff25ae9f0ecd8c2443053b15e40df4ad1eae0e4405")
    "aarch64-pc-windows-msvc" = @("arm64", "d5752ce2ac2296c8abb91fb12c12e75ee99ba8a18b52ce5febf325847b6f062b")
}
if (-not $PkgHashes.ContainsKey($Target)) { throw "Unsupported target: $Target" }
$Root = Join-Path $env:RUNNER_TEMP "voice-windows-build-tools"
$Evidence = Join-Path $Root "evidence"
$Cygwin = Join-Path $Root "cygwin"
$Cache = Join-Path $Root "cache"
New-Item -ItemType Directory -Path $Root, $Evidence | Out-Null
$ManifestPath = Join-Path $PSScriptRoot "voice-cygwin-snapshot.json"
$Manifest = Get-Content -Raw $ManifestPath | ConvertFrom-Json
$SnapshotTool = Join-Path $PSScriptRoot "voice-cygwin-inputs.py"
& python $SnapshotTool extract --archive $SnapshotArchive --directory (Join-Path $Cache $Manifest.cacheDirectory)
if ($LASTEXITCODE -ne 0) { throw "Cygwin snapshot could not be verified and extracted" }
Copy-Item $ManifestPath (Join-Path $Evidence "cygwin-snapshot.json")

# Official https://cygwin.com/setup/sha512.sum and pkgconf release asset digests.
# The complete package closure is pinned separately in cygwin-snapshot.json.
$Inputs = @(
    @{
        url = "https://cygwin.com/setup/setup-2.937.x86_64.exe"
        file = "setup.exe"
        algorithm = "SHA512"
        digest = "6acea47c59781c9e7f544a18d53935d59df6e44d5d52ac95ee165671b8e388820455eebf30ccc7d254b00c3d0eb694269a0f8dc84b17944c1f355dac9c5aafcc"
    },
    @{
        url = "https://github.com/pkgconf/pkgconf/releases/download/pkgconf-3.0.6/pkgconf-$($PkgHashes[$Target][0])-3.0.6.msi"
        file = "pkgconf.msi"
        algorithm = "SHA256"
        digest = $PkgHashes[$Target][1]
    }
)
foreach ($InputFile in $Inputs) {
    $Destination = Join-Path $Root $InputFile.file
    Invoke-WebRequest -Uri $InputFile.url -OutFile $Destination
    $Actual = (Get-FileHash -Path $Destination -Algorithm $InputFile.algorithm).Hash.ToLowerInvariant()
    if ($Actual -ne $InputFile.digest) { throw "Build input digest mismatch: $($InputFile.file)" }
}
$Inputs | ConvertTo-Json | Set-Content (Join-Path $Evidence "bootstrap-inputs.json")
$Packages = $Manifest.requestedPackages -join ","
# Only setup.xz + setup.xz.sig are present: local mode still verifies that
# signature. Never substitute an unsigned setup.ini or enable a network fallback.
$Setup = Start-Process -FilePath (Join-Path $Root "setup.exe") -Wait -PassThru -ArgumentList @(
    "--quiet-mode", "--no-admin", "--no-shortcuts", "--only-site",
    "--local-install", "--no-version-check",
    "--root", "`"$Cygwin`"", "--local-package-dir", "`"$Cache`"",
    "--packages", $Packages
)
if ($Setup.ExitCode -ne 0) { throw "Cygwin setup failed: $($Setup.ExitCode)" }
Copy-Item (Join-Path $Cygwin "var/log/setup.log*") $Evidence
Copy-Item (Join-Path $Cygwin "etc/setup/installed.db") $Evidence
& (Join-Path $Cygwin "bin/cygcheck.exe") -cd | Set-Content (Join-Path $Evidence "packages.txt")
if ($LASTEXITCODE -ne 0) { throw "Cannot inventory Cygwin packages" }
& python $SnapshotTool check-installed --inventory (Join-Path $Evidence "packages.txt")
if ($LASTEXITCODE -ne 0) { throw "Installed Cygwin package set differs from the snapshot" }
Get-ChildItem $Cache -File -Recurse | ForEach-Object {
    @{ file = [IO.Path]::GetRelativePath($Cache, $_.FullName); bytes = $_.Length;
       sha512 = (Get-FileHash $_.FullName -Algorithm SHA512).Hash.ToLowerInvariant() }
} | ConvertTo-Json | Set-Content (Join-Path $Evidence "package-cache.json")

# Administrative extraction uses TARGETDIR, not a normal MSI installation:
# no product registration or system PATH modification. Fail rather than fall
# back to an ambient executable or ordinary installer if the payload is absent.
$Image = Join-Path $Root "pkgconf-image"
$Extract = Start-Process msiexec.exe -Wait -PassThru -ArgumentList @(
    "/a", "`"$(Join-Path $Root 'pkgconf.msi')`"", "/qn", "/norestart",
    "TARGETDIR=`"$Image`"", "/L*v", "`"$(Join-Path $Evidence 'pkgconf-extract.log')`""
)
if ($Extract.ExitCode -ne 0) { throw "pkgconf extraction failed: $($Extract.ExitCode)" }
$Candidates = @(Get-ChildItem $Image -Filter pkgconf.exe -File -Recurse)
if ($Candidates.Count -ne 1) { throw "Expected exactly one native pkgconf executable" }
# Bazel shell-quotes execpaths containing spaces; Cargo executes PKG_CONFIG
# directly. Keep the executable and its support files at a space-free execpath.
$Native = Join-Path $Image "native"
if (Test-Path $Native) { throw "Native pkgconf destination must be fresh" }
Move-Item -LiteralPath $Candidates[0].Directory.FullName -Destination $Native
$PkgConfig = Join-Path $Native "pkgconf.exe"
$Version = & $PkgConfig --version
if ($LASTEXITCODE -ne 0 -or $Version -ne "3.0.6") { throw "Unexpected native pkgconf version" }
@{ target = $Target; cygwin_root = $Cygwin; pkg_config = $PkgConfig;
   pkgconf_version = $Version; requested_packages = $Packages } |
    ConvertTo-Json | Set-Content (Join-Path $Evidence "tools.json")
# Declare complete installed support trees only after snapshot and tool checks.
& python (Join-Path $PSScriptRoot "voice_windows_tools.py") --root $Root --target $Target --pkg-config $PkgConfig
if ($LASTEXITCODE -ne 0) { throw "Cannot declare Windows Bazel tool inputs" }
"VOICE_WINDOWS_BAZEL_REPOSITORY=$Root" | Out-File $env:GITHUB_ENV -Encoding utf8 -Append
"VOICE_CYGWIN_ROOT=$Cygwin" | Out-File $env:GITHUB_ENV -Encoding utf8 -Append
"VOICE_PKG_CONFIG=$PkgConfig" | Out-File $env:GITHUB_ENV -Encoding utf8 -Append
