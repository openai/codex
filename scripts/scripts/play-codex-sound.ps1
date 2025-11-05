param([Parameter(ValueFromRemainingArguments=$true)][string[]]$args)
$wavPath = Join-Path $PSScriptRoot "reimu_owattawa.wav"
Write-Host "Codex task completion notification" -ForegroundColor Cyan
if (Test-Path $wavPath) {
    try {
        $player = New-Object System.Media.SoundPlayer $wavPath
        $player.PlaySync()
        Write-Host "Sound played successfully (Reimu)" -ForegroundColor Green
    } catch {
        Write-Error "Error: $_"
        exit 1
    }
} else {
    Write-Warning "Sound file not found: $wavPath"
    Write-Host "Please place reimu_owattawa.wav in: $PSScriptRoot"
    exit 1
}

