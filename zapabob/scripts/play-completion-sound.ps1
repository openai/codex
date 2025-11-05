# Codex Agent完了音声再生スクリプト
# 魔理沙の「終わったぜ！」を再生するで

$soundPath = "C:\Users\downl\Desktop\SO8T\.cursor\marisa_owattaze.wav"

if (Test-Path $soundPath) {
    Write-Host "🎵 完了音声を再生するで！" -ForegroundColor Green
    
    # WindowsのSoundPlayerを使用して再生
    Add-Type -AssemblyName System.Windows.Forms
    $player = New-Object System.Media.SoundPlayer $soundPath
    $player.PlaySync()
    
    Write-Host "✅ 音声再生完了や！" -ForegroundColor Cyan
} else {
    Write-Host "⚠️  音声ファイルが見つからへんで: $soundPath" -ForegroundColor Yellow
    Write-Host "代わりにビープ音を鳴らすで" -ForegroundColor Yellow
    [Console]::Beep(800, 300)
    [Console]::Beep(1000, 300)
    [Console]::Beep(1200, 500)
}

