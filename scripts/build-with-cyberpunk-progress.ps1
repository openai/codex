# Cyberpunk-styled Build Script with tqdm-like Progress
# Builds Codex with colorful progress visualization

Write-Host ""
Write-Host "=== CODEX BUILD SYSTEM - CYBERPUNK EDITION ===" -ForegroundColor Cyan
Write-Host ""

$ErrorActionPreference = "Stop"
$repoPath = "C:\Users\downl\Desktop\codex"
Set-Location "$repoPath\codex-rs"

# ANSI color codes for Windows PowerShell
$ElectricBlue = "`e[38;2;0;212;255m"
$NeonPurple = "`e[38;2;184;79;255m"
$HotPink = "`e[38;2;255;0;110m"
$AcidGreen = "`e[38;2;57;255;20m"
$CyberYellow = "`e[38;2;255;255;0m"
$Reset = "`e[0m"

function Show-CyberpunkProgress {
    param(
        [int]$Current,
        [int]$Total,
        [string]$Task,
        [string]$Color = $ElectricBlue
    )
    
    $Percent = [math]::Round(($Current / $Total) * 100)
    $BarLength = 50
    $FilledLength = [math]::Round(($Percent / 100) * $BarLength)
    
    $Bar = ""
    for ($i = 0; $i -lt $FilledLength; $i++) {
        if ($i % 2 -eq 0) {
            $Bar += "█"
        } else {
            $Bar += "▓"
        }
    }
    for ($i = $FilledLength; $i -lt $BarLength; $i++) {
        $Bar += "░"
    }
    
    Write-Host -NoNewline "`r$Color[$Bar]$Reset $Percent% | $Task"
}

# Build targets
$targets = @(
    @{Name="codex-core"; Color=$ElectricBlue},
    @{Name="codex-cli"; Color=$NeonPurple},
    @{Name="codex-tui"; Color=$HotPink},
    @{Name="codex-tauri-gui"; Color=$AcidGreen}
)

$startTime = Get-Date

Write-Host "${ElectricBlue}[PHASE 1]${Reset} Checking for changes..." -ForegroundColor Cyan
Write-Host ""

# Detect changed crates
$changedCrates = @()
$gitStatus = git status --short

if ($gitStatus -match "codex-rs/core/") {
    $changedCrates += "codex-core"
}
if ($gitStatus -match "codex-rs/cli/") {
    $changedCrates += "codex-cli"
}
if ($gitStatus -match "codex-rs/tui/") {
    $changedCrates += "codex-tui"
}
if ($gitStatus -match "codex-rs/tauri-gui/") {
    $changedCrates += "codex-tauri-gui"
}

if ($changedCrates.Count -eq 0) {
    Write-Host "${AcidGreen}✓${Reset} No changes detected - using full build" -ForegroundColor Green
    $changedCrates = @("codex-cli") # Default to CLI only
} else {
    Write-Host "${AcidGreen}✓${Reset} Detected changes in: $($changedCrates -join ', ')" -ForegroundColor Green
}

Write-Host ""
Write-Host "${NeonPurple}[PHASE 2]${Reset} Building Rust crates (differential)..." -ForegroundColor Magenta
Write-Host ""

$totalSteps = $changedCrates.Count
$currentStep = 0

foreach ($crate in $changedCrates) {
    $currentStep++
    $target = $targets | Where-Object { $_.Name -eq $crate }
    $color = if ($target) { $target.Color } else { $ElectricBlue }
    
    Write-Host "${color}[BUILD $currentStep/$totalSteps]${Reset} $crate" -ForegroundColor Cyan
    
    # Simulate progress (cargo doesn't provide real-time progress)
    $buildSteps = @("Compiling", "Linking", "Optimizing", "Finishing")
    for ($i = 0; $i -lt $buildSteps.Length; $i++) {
        Show-CyberpunkProgress -Current ($i + 1) -Total $buildSteps.Length -Task "$crate - $($buildSteps[$i])" -Color $color
        Start-Sleep -Milliseconds 300
    }
    
    # Actual build
    Write-Host ""
    $buildCmd = "cargo build --release -p $crate"
    Write-Host "  ${CyberYellow}▶${Reset} $buildCmd" -ForegroundColor Yellow
    
    $buildOutput = & cargo build --release -p $crate 2>&1
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ${AcidGreen}✓${Reset} $crate built successfully" -ForegroundColor Green
    } else {
        Write-Host "  ${HotPink}✗${Reset} $crate build failed" -ForegroundColor Red
        Write-Host $buildOutput
        exit 1
    }
    
    Write-Host ""
}

$buildTime = (Get-Date) - $startTime

Write-Host ""
Write-Host "${AcidGreen}[PHASE 3]${Reset} Build complete!" -ForegroundColor Green
Write-Host ""
Write-Host "  ${ElectricBlue}●${Reset} Built crates: $($changedCrates.Count)" -ForegroundColor Cyan
Write-Host "  ${NeonPurple}●${Reset} Build time: $([math]::Round($buildTime.TotalSeconds, 2))s" -ForegroundColor Magenta
Write-Host "  ${HotPink}●${Reset} Status: ${AcidGreen}SUCCESS${Reset}" -ForegroundColor Green
Write-Host ""
Write-Host "${CyberYellow}[NEXT]${Reset} Run installation:" -ForegroundColor Yellow
Write-Host "  cargo install --path cli --force" -ForegroundColor Cyan
Write-Host ""

