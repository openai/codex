param(
    [string]$Release = "latest"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

function Write-Step {
    param(
        [string]$Message
    )

    Write-Host "==> $Message"
}

function Write-WarningStep {
    param(
        [string]$Message
    )

    Write-Warning $Message
}

function Normalize-Version {
    param(
        [string]$RawVersion
    )

    if ([string]::IsNullOrWhiteSpace($RawVersion) -or $RawVersion -eq "latest") {
        return "latest"
    }

    if ($RawVersion.StartsWith("rust-v")) {
        return $RawVersion.Substring(6)
    }

    if ($RawVersion.StartsWith("v")) {
        return $RawVersion.Substring(1)
    }

    return $RawVersion
}

function Get-ReleaseUrl {
    param(
        [string]$AssetName,
        [string]$ResolvedVersion
    )

    return "https://github.com/openai/codex/releases/download/rust-v$ResolvedVersion/$AssetName"
}

function Path-Contains {
    param(
        [string]$PathValue,
        [string]$Entry
    )

    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        return $false
    }

    $needle = $Entry.TrimEnd("\")
    foreach ($segment in $PathValue.Split(";", [System.StringSplitOptions]::RemoveEmptyEntries)) {
        if ($segment.TrimEnd("\") -ieq $needle) {
            return $true
        }
    }

    return $false
}

function Resolve-Version {
    $normalizedVersion = Normalize-Version -RawVersion $Release
    if ($normalizedVersion -ne "latest") {
        return $normalizedVersion
    }

    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/openai/codex/releases/latest"
    if (-not $release.tag_name) {
        Write-Error "Failed to resolve the latest Codex release version."
        exit 1
    }

    return (Normalize-Version -RawVersion $release.tag_name)
}

function Read-MetadataValue {
    param(
        [string]$MetadataPath,
        [string]$Key
    )

    if (-not (Test-Path $MetadataPath)) {
        return $null
    }

    foreach ($line in Get-Content $MetadataPath) {
        if ($line -match "^\s*$Key\s*=\s*""([^""]+)""") {
            return $matches[1]
        }
    }

    return $null
}

function Ensure-Junction {
    param(
        [string]$LinkPath,
        [string]$TargetPath
    )

    if (Test-Path $LinkPath) {
        $item = Get-Item -LiteralPath $LinkPath -Force
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            Remove-Item -LiteralPath $LinkPath -Force
        } else {
            Remove-Item -LiteralPath $LinkPath -Recurse -Force
        }
    }

    New-Item -ItemType Junction -Path $LinkPath -Target $TargetPath | Out-Null
}

function Get-ExistingCodexCommand {
    $existing = Get-Command codex -ErrorAction SilentlyContinue
    if ($null -eq $existing) {
        return $null
    }

    return $existing.Source
}

function Get-ExistingCodexManager {
    param(
        [string]$ExistingPath,
        [string]$VisibleBinDir
    )

    if ([string]::IsNullOrWhiteSpace($ExistingPath)) {
        return $null
    }

    if ($ExistingPath.StartsWith($VisibleBinDir, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $null
    }

    if ($ExistingPath -match "\\.bun\\") {
        return "bun"
    }

    if ($ExistingPath -match "node_modules" -or $ExistingPath -match "\\npm\\") {
        return "npm"
    }

    return $null
}

function Maybe-HandleConflictingInstall {
    param(
        [string]$VisibleBinDir
    )

    $existingPath = Get-ExistingCodexCommand
    $manager = Get-ExistingCodexManager -ExistingPath $existingPath -VisibleBinDir $VisibleBinDir
    if ($null -eq $manager) {
        return
    }

    Write-Step "Detected existing $manager-managed Codex at $existingPath"
    Write-WarningStep "Multiple managed Codex installs can be ambiguous because PATH order decides which one runs."

    $uninstallArgs = if ($manager -eq "bun") {
        @("remove", "-g", "@openai/codex")
    } else {
        @("uninstall", "-g", "@openai/codex")
    }
    $uninstallCommand = if ($manager -eq "bun") { "bun" } else { "npm" }

    $choice = Read-Host "Uninstall the existing $manager-managed Codex now? [y/N]"
    if ($choice -match "^(?i:y(?:es)?)$") {
        Write-Step "Running: $uninstallCommand $($uninstallArgs -join ' ')"
        try {
            & $uninstallCommand @uninstallArgs
        } catch {
            Write-WarningStep "Failed to uninstall the existing $manager-managed Codex. Continuing with the native install."
        }
    } else {
        Write-WarningStep "Leaving the existing $manager-managed Codex installed. PATH order will determine which codex runs."
    }
}

if ($env:OS -ne "Windows_NT") {
    Write-Error "install.ps1 supports Windows only. Use install.sh on macOS or Linux."
    exit 1
}

if (-not [Environment]::Is64BitOperatingSystem) {
    Write-Error "Codex requires a 64-bit version of Windows."
    exit 1
}

$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
$target = $null
$platformLabel = $null
$npmTag = $null
switch ($architecture) {
    "Arm64" {
        $target = "aarch64-pc-windows-msvc"
        $platformLabel = "Windows (ARM64)"
        $npmTag = "win32-arm64"
    }
    "X64" {
        $target = "x86_64-pc-windows-msvc"
        $platformLabel = "Windows (x64)"
        $npmTag = "win32-x64"
    }
    default {
        Write-Error "Unsupported architecture: $architecture"
        exit 1
    }
}

$codexHome = if ([string]::IsNullOrWhiteSpace($env:CODEX_HOME)) {
    Join-Path $env:USERPROFILE ".codex"
} else {
    $env:CODEX_HOME
}
$nativeRoot = Join-Path $codexHome "packages\native"
$releasesDir = Join-Path $nativeRoot "releases"
$currentDir = Join-Path $nativeRoot "current"

if ([string]::IsNullOrWhiteSpace($env:CODEX_INSTALL_DIR)) {
    $visibleBinDir = Join-Path $env:LOCALAPPDATA "Programs\OpenAI\Codex\bin"
} else {
    $visibleBinDir = $env:CODEX_INSTALL_DIR
}

$currentVersion = Read-MetadataValue -MetadataPath (Join-Path $currentDir "metadata.toml") -Key "version"
$resolvedVersion = Resolve-Version
$releaseName = "$resolvedVersion-$target"
$releaseDir = Join-Path $releasesDir $releaseName

if (-not [string]::IsNullOrWhiteSpace($currentVersion) -and $currentVersion -ne $resolvedVersion) {
    Write-Step "Updating Codex CLI from $currentVersion to $resolvedVersion"
} elseif (-not [string]::IsNullOrWhiteSpace($currentVersion)) {
    Write-Step "Updating Codex CLI"
} else {
    Write-Step "Installing Codex CLI"
}
Write-Step "Detected platform: $platformLabel"
Write-Step "Resolved version: $resolvedVersion"

Maybe-HandleConflictingInstall -VisibleBinDir $visibleBinDir

$packageAsset = "codex-npm-$npmTag-$resolvedVersion.tgz"
$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("codex-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tempDir | Out-Null

try {
    if (-not (Test-Path $releaseDir)) {
        $archivePath = Join-Path $tempDir $packageAsset
        $extractDir = Join-Path $tempDir "extract"
        $stagingDir = Join-Path $tempDir "release"
        $url = Get-ReleaseUrl -AssetName $packageAsset -ResolvedVersion $resolvedVersion

        Write-Step "Downloading Codex CLI"
        Invoke-WebRequest -Uri $url -OutFile $archivePath

        New-Item -ItemType Directory -Force -Path $extractDir | Out-Null
        New-Item -ItemType Directory -Force -Path $stagingDir | Out-Null
        tar -xzf $archivePath -C $extractDir

        $vendorRoot = Join-Path $extractDir "package/vendor/$target"
        $copyMap = @{
            "codex/codex.exe" = "codex.exe"
            "codex/codex-command-runner.exe" = "codex-command-runner.exe"
            "codex/codex-windows-sandbox-setup.exe" = "codex-windows-sandbox-setup.exe"
            "path/rg.exe" = "rg.exe"
        }

        foreach ($relativeSource in $copyMap.Keys) {
            Copy-Item -LiteralPath (Join-Path $vendorRoot $relativeSource) -Destination (Join-Path $stagingDir $copyMap[$relativeSource])
        }

        @"
install_method = "native"
version = "$resolvedVersion"
target = "$target"
"@ | Set-Content -LiteralPath (Join-Path $stagingDir "metadata.toml") -NoNewline

        New-Item -ItemType Directory -Force -Path $releasesDir | Out-Null
        Move-Item -LiteralPath $stagingDir -Destination $releaseDir
    }

    New-Item -ItemType Directory -Force -Path $nativeRoot | Out-Null
    Ensure-Junction -LinkPath $currentDir -TargetPath $releaseDir

    $visibleParent = Split-Path -Parent $visibleBinDir
    New-Item -ItemType Directory -Force -Path $visibleParent | Out-Null
    Ensure-Junction -LinkPath $visibleBinDir -TargetPath $currentDir
} finally {
    Remove-Item -Recurse -Force $tempDir -ErrorAction SilentlyContinue
}

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$pathNeedsNewShell = $false
if (-not (Path-Contains -PathValue $userPath -Entry $visibleBinDir)) {
    if ([string]::IsNullOrWhiteSpace($userPath)) {
        $newUserPath = $visibleBinDir
    } else {
        $newUserPath = "$visibleBinDir;$userPath"
    }

    [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
    if (-not (Path-Contains -PathValue $env:Path -Entry $visibleBinDir)) {
        if ([string]::IsNullOrWhiteSpace($env:Path)) {
            $env:Path = $visibleBinDir
        } else {
            $env:Path = "$visibleBinDir;$env:Path"
        }
    }
    Write-Step "PATH updated for future PowerShell sessions."
    $pathNeedsNewShell = $true
} elseif (Path-Contains -PathValue $env:Path -Entry $visibleBinDir) {
    Write-Step "$visibleBinDir is already on PATH."
} else {
    Write-Step "PATH is already configured for future PowerShell sessions."
    $pathNeedsNewShell = $true
}

if ($pathNeedsNewShell) {
    Write-Step ('Run now: $env:Path = "{0};$env:Path"; codex' -f $visibleBinDir)
    Write-Step "Or open a new PowerShell window and run: codex"
} else {
    Write-Step "Run: codex"
}

Write-Host "Codex CLI $resolvedVersion installed successfully."
