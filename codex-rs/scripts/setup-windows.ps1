<#
  Setup script for building codex-rs on Windows.

  What it does:
  - Installs Rust toolchain (via winget rustup) and required components
  - Installs Visual Studio 2022 Build Tools (MSVC + Windows SDK)
  - Installs helpful CLIs used by the repo: git, ripgrep (rg), just, cmake
  - Installs cargo-insta (for snapshot tests) via cargo
  - Ensures PATH contains Cargo bin for the current session
  - Builds the workspace (cargo build)

  Usage:
    - Right-click PowerShell and "Run as Administrator" (VS Build Tools require elevation)
    - From the repo root (codex-rs), run:
        powershell -ExecutionPolicy Bypass -File scripts/setup-windows.ps1

  Notes:
    - Requires winget (Windows Package Manager). Most modern Windows 10/11 have it preinstalled.
    - The script is re-runnable; winget/cargo will skip/reinstall as appropriate.
#>

param(
  [switch] $SkipBuild
)

$ErrorActionPreference = 'Stop'

function Ensure-Command($Name) {
  $exists = Get-Command $Name -ErrorAction SilentlyContinue
  return $null -ne $exists
}

function Add-CargoBinToPath() {
  $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
  if (Test-Path $cargoBin) {
    if (-not ($env:Path.Split(';') -contains $cargoBin)) {
      $env:Path = "$env:Path;$cargoBin"
    }
  }
}

function Ensure-UserPathContains([string] $Segment) {
  try {
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($null -eq $userPath) { $userPath = '' }
    $parts = $userPath.Split(';') | Where-Object { $_ -ne '' }
    if (-not ($parts -contains $Segment)) {
      $newPath = if ($userPath) { "$userPath;$Segment" } else { $Segment }
      [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    }
  } catch {}
}

function Ensure-UserEnvVar([string] $Name, [string] $Value) {
  try { [Environment]::SetEnvironmentVariable($Name, $Value, 'User') } catch {}
}

function Ensure-VSComponents([string[]]$Components) {
  $vsInstaller = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vs_installer.exe"
  $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
  if (-not (Test-Path $vsInstaller) -or -not (Test-Path $vswhere)) { return }

  $instPath = & $vswhere -latest -products * -version "[17.0,18.0)" -requires Microsoft.VisualStudio.Workload.VCTools -property installationPath 2>$null
  if (-not $instPath) {
    # 2022 instance may be present without VC Tools; pick BuildTools 2022 and add components
    $instPath = & $vswhere -latest -products Microsoft.VisualStudio.Product.BuildTools -version "[17.0,18.0)" -property installationPath 2>$null
  }
  if (-not $instPath) {
    $instPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Workload.VCTools -property installationPath 2>$null
  }
  if (-not $instPath) {
    $default2022 = 'C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools'
    if (Test-Path $default2022) { $instPath = $default2022 }
  }
  if (-not $instPath) { return }

  $vsDevCmd = Join-Path $instPath 'Common7\Tools\VsDevCmd.bat'
  $verb = if (Test-Path $vsDevCmd) { 'modify' } else { 'install' }
  $args = @($verb, '--installPath', $instPath, '--quiet', '--norestart', '--nocache')
  if ($verb -eq 'install') { $args += @('--productId', 'Microsoft.VisualStudio.Product.BuildTools') }
  foreach ($c in $Components) { $args += @('--add', $c) }
  Write-Host "-- Ensuring VS components installed: $($Components -join ', ')" -ForegroundColor DarkCyan
  & $vsInstaller @args | Out-Host
}

function Enter-VsDevShell() {
  $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
  if (-not (Test-Path $vswhere)) { return }

  $instPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
  if (-not $instPath) {
    # Try ARM64 components
    $instPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.ARM64 -property installationPath 2>$null
  }
  if (-not $instPath) { return }

  $vsDevCmd = Join-Path $instPath 'Common7\Tools\VsDevCmd.bat'
  if (-not (Test-Path $vsDevCmd)) { return }

  # Prefer ARM64 on ARM machines, otherwise x64
  $arch = if ($env:PROCESSOR_ARCHITEW6432 -eq 'ARM64' -or $env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x64' }
  $devCmdStr = ('"{0}" -no_logo -arch={1} -host_arch={1} & set' -f $vsDevCmd, $arch)
  $envLines = & cmd.exe /c $devCmdStr
  foreach ($line in $envLines) {
    if ($line -match '^(.*?)=(.*)$') {
      $name = $matches[1]
      $value = $matches[2]
      try { [Environment]::SetEnvironmentVariable($name, $value, 'Process') } catch {}
    }
  }
}

Write-Host "==> Installing prerequisites via winget (may take a while)" -ForegroundColor Cyan

# Accept agreements up-front for non-interactive installs
$WingetArgs = @('--accept-package-agreements', '--accept-source-agreements', '-e')

if (-not (Ensure-Command 'winget')) {
  throw "winget is required. Please update to the latest Windows 10/11 or install winget."
}

# 1) Visual Studio 2022 Build Tools (MSVC toolchain + Windows SDK)
# The VC Tools workload brings the required MSVC toolchains; include recommended components to pick up a Windows SDK.
Write-Host "-- Installing Visual Studio Build Tools (VC Tools workload + ARM64 toolchains)" -ForegroundColor DarkCyan
$vsOverride = @(
  '--quiet', '--wait', '--norestart', '--nocache',
  '--add', 'Microsoft.VisualStudio.Workload.VCTools',
  '--add', 'Microsoft.VisualStudio.Component.VC.Tools.ARM64',
  '--add', 'Microsoft.VisualStudio.Component.VC.Tools.ARM64EC',
  '--add', 'Microsoft.VisualStudio.Component.Windows11SDK.22000'
) -join ' '
winget install @WingetArgs --id Microsoft.VisualStudio.2022.BuildTools --override $vsOverride | Out-Host

# Ensure required VC components even if winget doesn't modify the instance
$isArm64 = ($env:PROCESSOR_ARCHITEW6432 -eq 'ARM64' -or $env:PROCESSOR_ARCHITECTURE -eq 'ARM64')
$components = @(
  'Microsoft.VisualStudio.Workload.VCTools',
  'Microsoft.VisualStudio.Component.VC.Tools.ARM64',
  'Microsoft.VisualStudio.Component.VC.Tools.ARM64EC',
  'Microsoft.VisualStudio.Component.Windows11SDK.22000'
)
Ensure-VSComponents -Components $components

# 2) Rustup
Write-Host "-- Installing rustup" -ForegroundColor DarkCyan
winget install @WingetArgs --id Rustlang.Rustup | Out-Host

# Make cargo available in this session
Add-CargoBinToPath

# 3) Git (often present, but ensure installed)
Write-Host "-- Installing Git" -ForegroundColor DarkCyan
winget install @WingetArgs --id Git.Git | Out-Host

# 4) ripgrep (rg)
Write-Host "-- Installing ripgrep (rg)" -ForegroundColor DarkCyan
winget install @WingetArgs --id BurntSushi.ripgrep.MSVC | Out-Host

# 5) just
Write-Host "-- Installing just" -ForegroundColor DarkCyan
winget install @WingetArgs --id Casey.Just | Out-Host

# 6) cmake (commonly needed by native crates)
Write-Host "-- Installing CMake" -ForegroundColor DarkCyan
winget install @WingetArgs --id Kitware.CMake | Out-Host

# Ensure cargo is available after rustup install
Add-CargoBinToPath
if (-not (Ensure-Command 'cargo')) {
  # Some shells need a re-login; attempt to source cargo.env if present
  $cargoEnv = Join-Path $env:USERPROFILE ".cargo\env"
  if (Test-Path $cargoEnv) { . $cargoEnv }
  Add-CargoBinToPath
}
if (-not (Ensure-Command 'cargo')) {
  throw "cargo not found in PATH after rustup install. Please open a new terminal and re-run the script."
}

Write-Host "==> Configuring Rust toolchain per rust-toolchain.toml" -ForegroundColor Cyan

# Pin to the workspace toolchain and install components
$toolchain = '1.92.0'
& rustup toolchain install $toolchain --profile minimal | Out-Host
& rustup default $toolchain | Out-Host
& rustup component add clippy rustfmt rust-src --toolchain $toolchain | Out-Host

# 6.5) LLVM/Clang (some crates/bindgen require clang/libclang)
function Add-LLVMToPath() {
  $llvmBin = 'C:\\Program Files\\LLVM\\bin'
  if (Test-Path $llvmBin) {
    if (-not ($env:Path.Split(';') -contains $llvmBin)) {
      $env:Path = "$env:Path;$llvmBin"
    }
    if (-not $env:LIBCLANG_PATH) {
      $env:LIBCLANG_PATH = $llvmBin
    }
    Ensure-UserPathContains $llvmBin
    Ensure-UserEnvVar -Name 'LIBCLANG_PATH' -Value $llvmBin

    $clang = Join-Path $llvmBin 'clang.exe'
    $clangxx = Join-Path $llvmBin 'clang++.exe'
    if (Test-Path $clang) {
      $env:CC = $clang
      Ensure-UserEnvVar -Name 'CC' -Value $clang
    }
    if (Test-Path $clangxx) {
      $env:CXX = $clangxx
      Ensure-UserEnvVar -Name 'CXX' -Value $clangxx
    }
  }
}

Write-Host "-- Installing LLVM/Clang" -ForegroundColor DarkCyan
winget install @WingetArgs --id LLVM.LLVM | Out-Host
Add-LLVMToPath

# 7) cargo-insta (used by snapshot tests)
# Ensure MSVC linker is available before building/cargo-install by entering VS dev shell
Enter-VsDevShell
$hasLink = $false
try { & where.exe link | Out-Null; $hasLink = $true } catch {}
if ($hasLink) {
  Write-Host "-- Installing cargo-insta" -ForegroundColor DarkCyan
  & cargo install cargo-insta --locked | Out-Host
} else {
  Write-Host "-- Skipping cargo-insta for now (MSVC linker not found yet)" -ForegroundColor Yellow
}

if ($SkipBuild) {
  Write-Host "==> Skipping cargo build (SkipBuild specified)" -ForegroundColor Yellow
  exit 0
}

Write-Host "==> Building workspace (cargo build)" -ForegroundColor Cyan
pushd "$PSScriptRoot\.." | Out-Null
try {
  # Clear RUSTFLAGS if coming from constrained environments
  $env:RUSTFLAGS = ''
  Enter-VsDevShell
  & cargo build
}
finally {
  popd | Out-Null
}

Write-Host "==> Build complete" -ForegroundColor Green
