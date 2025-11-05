# Codex Tauri - 完了音声再生スクリプト
# 魔理沙「終わったぜ！」

$soundPath = "C:\Users\downl\Desktop\SO8T\.cursor\marisa_owattaze.wav"

Write-Host "🔊 完了音声を再生するで..." -ForegroundColor Green

if (Test-Path $soundPath) {
    # Windows の SoundPlayer を使用して再生
    Add-Type -AssemblyName System.Windows.Forms
    $player = New-Object System.Media.SoundPlayer $soundPath
    $player.PlaySync()
    
    Write-Host "✅ 音声再生完了やで！" -ForegroundColor Green
} else {
    Write-Host "❌ 音声ファイルが見つかりません: $soundPath" -ForegroundColor Red
    Write-Host "   ファイルの存在を確認してください" -ForegroundColor Yellow
}

