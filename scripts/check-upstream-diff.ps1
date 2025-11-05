# Check differences between zapabob/codex and openai/codex
# 公式リポジトリとの差分を確認

Write-Host "=== zapabob/codex ↔ openai/codex 差分チェック ===" -ForegroundColor Cyan
Write-Host ""

$repoPath = "C:\Users\downl\Desktop\codex"
Set-Location $repoPath

# Step 1: Fetch latest from upstream
Write-Host "[1/5] Fetching latest from upstream (openai/codex)..." -ForegroundColor Yellow
git fetch upstream 2>&1 | Out-Null
Write-Host "  ✓ Fetched" -ForegroundColor Green

# Step 2: Show commit difference
Write-Host "`n[2/5] Commit difference..." -ForegroundColor Yellow
$aheadBehind = git rev-list --left-right --count upstream/main...main
$behindCount = ($aheadBehind -split '\s+')[0]
$aheadCount = ($aheadBehind -split '\s+')[1]

Write-Host "  Behind upstream: $behindCount commits" -ForegroundColor $(if ($behindCount -gt 0) { "Red" } else { "Green" })
Write-Host "  Ahead of upstream: $aheadCount commits" -ForegroundColor Cyan

# Step 3: Show recent commits on both sides
Write-Host "`n[3/5] Recent commits comparison..." -ForegroundColor Yellow
Write-Host "`n  公式リポジトリ (upstream/main):" -ForegroundColor Magenta
git log --oneline --max-count=5 upstream/main
Write-Host "`n  zapabobリポジトリ (origin/main):" -ForegroundColor Cyan
git log --oneline --max-count=5 main

# Step 4: Show file differences
Write-Host "`n[4/5] File differences..." -ForegroundColor Yellow
$diffFiles = git diff --name-status upstream/main...main

if ($diffFiles) {
    Write-Host "  Changed files:" -ForegroundColor White
    $diffFiles | ForEach-Object {
        $status = $_.Substring(0, 1)
        $file = $_.Substring(2)
        $color = switch ($status) {
            "A" { "Green" }  # Added
            "M" { "Yellow" } # Modified
            "D" { "Red" }    # Deleted
            default { "White" }
        }
        $symbol = switch ($status) {
            "A" { "+" }
            "M" { "~" }
            "D" { "-" }
            default { "?" }
        }
        Write-Host "    [$symbol] $file" -ForegroundColor $color
    }
    
    $fileCount = ($diffFiles | Measure-Object).Count
    Write-Host "`n  Total: $fileCount files" -ForegroundColor White
} else {
    Write-Host "  No file differences" -ForegroundColor Green
}

# Step 5: Show merge preview
Write-Host "`n[5/5] Merge preview..." -ForegroundColor Yellow

# Check if merge would create conflicts
$conflicts = git merge-tree $(git merge-base upstream/main main) upstream/main main | Select-String "^changed in both"

if ($conflicts) {
    Write-Host "  ⚠️  Potential conflicts detected:" -ForegroundColor Red
    $conflicts | ForEach-Object {
        Write-Host "    $_" -ForegroundColor Yellow
    }
} else {
    Write-Host "  ✓ No conflicts detected - safe to merge" -ForegroundColor Green
}

# Summary
Write-Host "`n=== Summary ===" -ForegroundColor Cyan
Write-Host "  Status: " -NoNewline
if ($behindCount -eq 0) {
    Write-Host "✓ Up to date with upstream" -ForegroundColor Green
} elseif ($behindCount -le 10) {
    Write-Host "⚠️  Slightly behind ($behindCount commits)" -ForegroundColor Yellow
} else {
    Write-Host "❌ Significantly behind ($behindCount commits)" -ForegroundColor Red
}

if ($aheadCount -gt 0) {
    Write-Host "  Unique commits: $aheadCount (zapabob extensions)" -ForegroundColor Cyan
}

# Recommendations
Write-Host "`n=== Recommendations ===" -ForegroundColor Cyan
if ($behindCount -gt 0) {
    Write-Host "  Consider syncing with upstream:" -ForegroundColor Yellow
    Write-Host "    git merge upstream/main" -ForegroundColor White
    Write-Host "    git push origin main" -ForegroundColor White
} else {
    Write-Host "  ✓ No action needed - repository is up to date" -ForegroundColor Green
}

Write-Host ""

