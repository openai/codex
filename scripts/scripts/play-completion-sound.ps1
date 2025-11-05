param([Parameter(ValueFromRemainingArguments=$true)][string[]]$args)

# Get current script directory and resolve to project root
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent (Split-Path -Parent $scriptDir)
$wavPath = Join-Path $projectRoot ".codex\marisa_owattaze.wav"

Write-Host "🎉 Agent/Plan completion notification (Marisa)" -ForegroundColor Magenta
Write-Host "Looking for sound file: $wavPath" -ForegroundColor Gray

if (Test-Path $wavPath) {
    try {
        $player = New-Object System.Media.SoundPlayer $wavPath
        $player.PlaySync()
        Write-Host "✅ Sound played: 終わったぜ！ (Marisa)" -ForegroundColor Green
    } catch {
        Write-Error "❌ Error playing sound: $_"
        exit 1
    }
} else {
    Write-Warning "⚠️  Sound file not found: $wavPath"
    Write-Host "Expected location: $wavPath" -ForegroundColor Yellow
    exit 1
}