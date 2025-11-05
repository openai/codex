# tqdm風ビルド進捗モニター
# Rustのビルド進捗をリアルタイムで表示

param(
    [int]$TotalCrates = 500  # 概算の総クレート数（実際は動的に調整）
)

$logFile = "codex-rs/target/release/.fingerprint"
$buildDir = "codex-rs/target/release"

Write-Host ""
Write-Host ">> Codex Build Progress Monitor (tqdm style)" -ForegroundColor Cyan
Write-Host ("=" * 60) -ForegroundColor Gray
Write-Host ""

$startTime = Get-Date
$lastCount = 0
$iteration = 0

function Draw-ProgressBar {
    param(
        [int]$Current,
        [int]$Total,
        [double]$ElapsedSeconds,
        [string]$CurrentCrate
    )
    
    $percent = if ($Total -gt 0) { [math]::Round(($Current / $Total) * 100, 1) } else { 0 }
    $barWidth = 40
    $filledWidth = [math]::Round(($Current / $Total) * $barWidth)
    
    # プログレスバー作成
    $bar = ""
    for ($i = 0; $i -lt $barWidth; $i++) {
        if ($i -lt $filledWidth) {
            $bar += "#"
        } else {
            $bar += "-"
        }
    }
    
    # 速度計算
    $rate = if ($ElapsedSeconds -gt 0) { [math]::Round($Current / $ElapsedSeconds, 2) } else { 0 }
    $eta = if ($rate -gt 0) { [math]::Round(($Total - $Current) / $rate, 0) } else { 0 }
    
    # 時間フォーマット
    $elapsedMin = [math]::Floor($ElapsedSeconds / 60)
    $elapsedSec = [math]::Floor($ElapsedSeconds % 60)
    $etaMin = [math]::Floor($eta / 60)
    $etaSec = [math]::Floor($eta % 60)
    
    # 進捗表示（tqdm風）
    $progressLine = "{0,5}% |{1}| {2}/{3} [{4}m{5}s<{6}m{7}s, {8} crates/s]" -f `
        $percent, $bar, $Current, $Total, $elapsedMin, $elapsedSec, $etaMin, $etaSec, $rate
    
    Write-Host "`r$progressLine" -NoNewline -ForegroundColor Cyan
    
    # 現在コンパイル中のクレート
    if ($CurrentCrate) {
        $crateDisplay = if ($CurrentCrate.Length -gt 30) { 
            $CurrentCrate.Substring(0, 27) + "..." 
        } else { 
            $CurrentCrate.PadRight(30) 
        }
        Write-Host "$crateDisplay" -NoNewline -ForegroundColor White
    }
}

# メインループ
while ($true) {
    $iteration++
    
    # ビルドディレクトリが存在するかチェック
    if (Test-Path $buildDir) {
        # コンパイル済みクレート数をカウント
        $compiledCount = 0
        
        # .rmeta ファイル（コンパイル完了）をカウント
        if (Test-Path $buildDir) {
            $compiledCount = (Get-ChildItem -Path $buildDir -Filter "*.rlib" -Recurse -ErrorAction SilentlyContinue).Count
            $compiledCount += (Get-ChildItem -Path $buildDir -Filter "*.rmeta" -Recurse -ErrorAction SilentlyContinue).Count
        }
        
        # 現在コンパイル中のクレートを取得
        $currentCrate = ""
        $buildLog = "codex-rs/target/release/build"
        if (Test-Path $buildLog) {
            $recentDirs = Get-ChildItem -Path $buildLog -Directory -ErrorAction SilentlyContinue | 
                          Sort-Object LastWriteTime -Descending | 
                          Select-Object -First 1
            if ($recentDirs) {
                $currentCrate = $recentDirs.Name -replace '-[a-f0-9]+$', ''
            }
        }
        
        # 総クレート数の動的調整
        if ($compiledCount -gt $TotalCrates * 0.9) {
            $TotalCrates = [math]::Max($TotalCrates, $compiledCount + 50)
        }
        
        # 進捗表示
        $elapsed = (Get-Date) - $startTime
        Draw-ProgressBar -Current $compiledCount -Total $TotalCrates -ElapsedSeconds $elapsed.TotalSeconds -CurrentCrate $currentCrate
        
        $lastCount = $compiledCount
        
        # ビルド完了チェック
        if (Test-Path "codex-rs/target/release/codex.exe") {
            Write-Host ""
            Write-Host ""
            Write-Host "[SUCCESS] Build completed!" -ForegroundColor Green
            Write-Host "   Total time: $([math]::Round($elapsed.TotalMinutes, 2)) minutes" -ForegroundColor Cyan
            Write-Host "   Binary: codex-rs/target/release/codex.exe" -ForegroundColor Gray
            break
        }
    } else {
        Write-Host "`rWaiting for build to start..." -NoNewline -ForegroundColor Yellow
    }
    
    # 更新間隔
    Start-Sleep -Milliseconds 500
    
    # 60分でタイムアウト
    if ($elapsed.TotalMinutes -gt 60) {
        Write-Host ""
        Write-Host ""
        Write-Host "[WARNING] Build timeout (60 minutes)" -ForegroundColor Red
        break
    }
}

Write-Host ""

