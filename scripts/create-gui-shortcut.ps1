# Codex GUI Desktop Shortcut Creator
# Created: 2025-11-15 13:40:06

$ErrorActionPreference = "Stop"

# Path settings
$projectRoot = Split-Path -Parent $PSScriptRoot
$guiIconSvg = Join-Path $projectRoot "gui\public\icon-512x512.svg"
$desktopPath = [Environment]::GetFolderPath("Desktop")
$shortcutPath = Join-Path $desktopPath "Codex GUI.lnk"
$codexGuiPath = Join-Path $env:USERPROFILE ".cargo\bin\codex-gui.exe"

Write-Host "Creating Codex GUI desktop shortcut..." -ForegroundColor Cyan

# Check if codex-gui.exe exists
if (-not (Test-Path $codexGuiPath)) {
    Write-Host "Error: codex-gui.exe not found: $codexGuiPath" -ForegroundColor Red
    Write-Host "Please build and install GUI first." -ForegroundColor Yellow
    exit 1
}

# Convert SVG to ICO (temporary file)
$tempIco = Join-Path $env:TEMP "codex-gui-icon.ico"

try {
    # Try ImageMagick first, then Python fallback
    Write-Host "Converting SVG to ICO..." -ForegroundColor Yellow
    
    $magickCmd = Get-Command magick -ErrorAction SilentlyContinue
    if ($magickCmd) {
        # Use ImageMagick
        Write-Host "  Using ImageMagick..." -ForegroundColor Gray
        $svgToIcoScript = Join-Path $projectRoot "scripts\svg-to-ico.ps1"
        if (Test-Path $svgToIcoScript) {
            & $svgToIcoScript -SvgPath $guiIconSvg -OutputPath $tempIco
            if (-not (Test-Path $tempIco)) {
                Write-Host "  ImageMagick conversion failed, trying Python..." -ForegroundColor Yellow
                $magickCmd = $null  # Fall through to Python
            }
        }
    }
    
    if (-not $magickCmd -or -not (Test-Path $tempIco)) {
        # Use Python as fallback (Pillow only - no cairo dependency)
        Write-Host "  Using Python (Pillow only)..." -ForegroundColor Gray
        $svgToIcoSimpleScript = Join-Path $projectRoot "scripts\svg-to-ico-simple.ps1"
        if (Test-Path $svgToIcoSimpleScript) {
            & $svgToIcoSimpleScript -SvgPath $guiIconSvg -OutputPath $tempIco
            if (-not (Test-Path $tempIco)) {
                Write-Host "Warning: SVG to ICO conversion failed. Using default icon." -ForegroundColor Yellow
                $tempIco = $null
            }
        } else {
            Write-Host "Warning: Conversion scripts not found. Using default icon." -ForegroundColor Yellow
            $tempIco = $null
        }
    }
} catch {
    Write-Host "Warning: SVG to ICO conversion failed. Using default icon." -ForegroundColor Yellow
    $tempIco = $null
}

# Create shortcut
Write-Host "Creating desktop shortcut..." -ForegroundColor Yellow

# Use launcher script instead of direct executable
$launcherScript = Join-Path $projectRoot "scripts\launch-codex-gui.ps1"

$WshShell = New-Object -ComObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut($shortcutPath)

# Use PowerShell to run the launcher script
$Shortcut.TargetPath = "powershell.exe"
$Shortcut.Arguments = "-ExecutionPolicy Bypass -File `"$launcherScript`""
$Shortcut.WorkingDirectory = $projectRoot
$Shortcut.Description = "Codex GUI - AI-Native OS with 4D Git Visualization & VR/AR Support"
$Shortcut.IconLocation = if ($tempIco -and (Test-Path $tempIco)) { $tempIco } else { $codexGuiPath + ",0" }

$Shortcut.Save()

Write-Host "Desktop shortcut created: $shortcutPath" -ForegroundColor Green
$iconInfo = if ($tempIco -and (Test-Path $tempIco)) { $tempIco } else { "Default" }
Write-Host "  Icon: $iconInfo" -ForegroundColor Gray
Write-Host "  Executable: $codexGuiPath" -ForegroundColor Gray
