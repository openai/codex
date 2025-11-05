# Windows Notification Sound Setter
# Sets Windows system notification sound to marisa_owattaze.wav

param(
    [string]$SoundFile = "C:\Users\downl\Desktop\新しいフォルダー (4)\marisa_owattaze.wav"
)

Write-Host "=== Windows Notification Sound Setter ===" -ForegroundColor Cyan
Write-Host ""

# Check if sound file exists
if (-not (Test-Path $SoundFile)) {
    Write-Host "ERROR: Sound file not found!" -ForegroundColor Red
    Write-Host "  Expected: $SoundFile" -ForegroundColor Yellow
    exit 1
}

$fileInfo = Get-Item $SoundFile
Write-Host "Sound file found:" -ForegroundColor Green
Write-Host "  Path: $SoundFile" -ForegroundColor White
Write-Host "  Size: $([math]::Round($fileInfo.Length/1KB, 2)) KB" -ForegroundColor White
Write-Host ""

# Registry paths for different notification events
$notificationEvents = @{
    "Notification" = "HKCU:\AppEvents\Schemes\Apps\.Default\Notification.Default\.Current"
    "SystemNotification" = "HKCU:\AppEvents\Schemes\Apps\.Default\SystemNotification\.Current"
    "MessageNudge" = "HKCU:\AppEvents\Schemes\Apps\.Default\MessageNudge\.Current"
}

Write-Host "Setting notification sounds..." -ForegroundColor Yellow
Write-Host ""

$successCount = 0
foreach ($event in $notificationEvents.GetEnumerator()) {
    $eventName = $event.Key
    $regPath = $event.Value
    
    # Check if registry path exists
    if (Test-Path $regPath) {
        try {
            # Backup current value
            $current = Get-ItemProperty -Path $regPath -Name "(Default)" -ErrorAction SilentlyContinue
            if ($current -and $current.'(Default)') {
                Write-Host "  [$eventName] Current: $($current.'(Default)')" -ForegroundColor Gray
            }
            
            # Set new value
            Set-ItemProperty -Path $regPath -Name "(Default)" -Value $SoundFile -ErrorAction Stop
            Write-Host "  [$eventName] Updated successfully!" -ForegroundColor Green
            $successCount++
        } catch {
            Write-Host "  [$eventName] Failed: $_" -ForegroundColor Red
        }
    } else {
        Write-Host "  [$eventName] Registry path not found (skipping)" -ForegroundColor Yellow
    }
}

Write-Host ""
if ($successCount -gt 0) {
    Write-Host "SUCCESS: $successCount notification sound(s) updated!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Testing sound..." -ForegroundColor Magenta
    try {
        $player = New-Object System.Media.SoundPlayer $SoundFile
        $player.PlaySync()
        Write-Host "Sound test complete!" -ForegroundColor Green
    } catch {
        Write-Host "Warning: Could not play test sound: $_" -ForegroundColor Yellow
    }
} else {
    Write-Host "ERROR: No notification sounds were updated" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Done! Marisa will now notify you on Windows events." -ForegroundColor Cyan

