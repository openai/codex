# Merge feature/openai-pr-clean to main and push to zapabob/codex

Write-Host "Merging to zapabob/codex main branch..." -ForegroundColor Green

$repoPath = "C:\Users\downl\Desktop\codex-main\codex-main"
Set-Location $repoPath

# Step 1: Check current status
Write-Host "`nStep 1: Current status" -ForegroundColor Yellow
$currentBranch = git rev-parse --abbrev-ref HEAD 2>$null
Write-Host "Current branch: $currentBranch" -ForegroundColor Cyan

# Step 2: Fetch latest from origin
Write-Host "`nStep 2: Fetching latest from origin (zapabob/codex)..." -ForegroundColor Yellow
git fetch origin 2>$null

# Step 3: Checkout main
Write-Host "`nStep 3: Switching to main branch..." -ForegroundColor Yellow
git checkout main 2>$null

if ($LASTEXITCODE -eq 0) {
    Write-Host "Switched to main" -ForegroundColor Green
} else {
    Write-Host "Failed to switch to main" -ForegroundColor Red
    exit 1
}

# Step 4: Pull latest main
Write-Host "`nStep 4: Pulling latest main from origin..." -ForegroundColor Yellow
git pull origin main 2>$null

# Step 5: Merge feature branch
Write-Host "`nStep 5: Merging feature/openai-pr-clean into main..." -ForegroundColor Yellow
git merge feature/openai-pr-clean --no-edit 2>$null

if ($LASTEXITCODE -eq 0) {
    Write-Host "Merge successful!" -ForegroundColor Green
} else {
    Write-Host "Merge failed - conflicts may need resolution" -ForegroundColor Red
    Write-Host "Run: git status" -ForegroundColor Yellow
    exit 1
}

# Step 6: Show merge result
Write-Host "`nStep 6: Merge result..." -ForegroundColor Yellow
$commitCount = git rev-list --count origin/main..HEAD 2>$null
Write-Host "Commits ahead of origin/main: $commitCount" -ForegroundColor Cyan

# Step 7: Push to origin
Write-Host "`nStep 7: Pushing to origin (zapabob/codex)..." -ForegroundColor Yellow
git push origin main 2>$null

if ($LASTEXITCODE -eq 0) {
    Write-Host "Push successful!" -ForegroundColor Green
} else {
    Write-Host "Push failed - you may need to resolve conflicts or force push" -ForegroundColor Red
    Write-Host "If you need to force push, run:" -ForegroundColor Yellow
    Write-Host "  git push origin main --force" -ForegroundColor Cyan
    exit 1
}

# Step 8: Summary
Write-Host "`n=== Merge Complete ===" -ForegroundColor Green
Write-Host "Branch: main" -ForegroundColor Cyan
Write-Host "Remote: origin (zapabob/codex)" -ForegroundColor Cyan
Write-Host "Status: Pushed successfully" -ForegroundColor Green

Write-Host "`nLatest commits on main:" -ForegroundColor Yellow
git log --oneline -5 2>$null | ForEach-Object { Write-Host "  $_" -ForegroundColor Cyan }

Write-Host "`nYour zapabob/codex main branch is now updated!" -ForegroundColor Green
Write-Host "View on GitHub: https://github.com/zapabob/codex" -ForegroundColor Cyan

Write-Host "`nNext step: Create PR to openai/codex from zapabob/codex:main" -ForegroundColor Yellow

