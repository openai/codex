# Complete script to commit and push to zapabob/codex main
# Run this in a NEW PowerShell window

Write-Host "=== Committing to zapabob/codex main ===" -ForegroundColor Green

$repoPath = "C:\Users\downl\Desktop\codex-main\codex-main"
Set-Location $repoPath

# Step 1: Clean up unwanted changes
Write-Host "`nStep 1: Cleaning up..." -ForegroundColor Yellow
git restore ".specstory/history/2025-10-07_16-28Z-request-for-coding-assistance.md" 2>$null

# Step 2: Check current branch and status
Write-Host "`nStep 2: Current status" -ForegroundColor Yellow
$currentBranch = git rev-parse --abbrev-ref HEAD 2>$null
Write-Host "Current branch: $currentBranch" -ForegroundColor Cyan

# Step 3: Switch to main
Write-Host "`nStep 3: Switching to main..." -ForegroundColor Yellow
git checkout main 2>$null

if ($LASTEXITCODE -ne 0) {
    Write-Host "Failed to checkout main. Creating from origin/main..." -ForegroundColor Yellow
    git checkout -b main origin/main 2>$null
}

Write-Host "On branch: main" -ForegroundColor Green

# Step 4: Merge feature branch
Write-Host "`nStep 4: Merging feature/openai-pr-clean..." -ForegroundColor Yellow
git merge feature/openai-pr-clean --no-edit 2>$null

if ($LASTEXITCODE -eq 0) {
    Write-Host "Merge successful!" -ForegroundColor Green
} else {
    Write-Host "Merge had conflicts or failed" -ForegroundColor Red
    Write-Host "Checking status..." -ForegroundColor Yellow
    $status = git status --porcelain 2>$null
    if ($status) {
        Write-Host "There are conflicts. You'll need to resolve them manually:" -ForegroundColor Red
        Write-Host $status -ForegroundColor Yellow
        exit 1
    }
}

# Step 5: Push to origin (zapabob/codex)
Write-Host "`nStep 5: Pushing to origin/main (zapabob/codex)..." -ForegroundColor Yellow

# First, fetch to check if we're behind
git fetch origin 2>$null

$status = git status -sb 2>$null
Write-Host "Git status: $status" -ForegroundColor Cyan

git push origin main 2>$null

if ($LASTEXITCODE -eq 0) {
    Write-Host "Push successful!" -ForegroundColor Green
} else {
    Write-Host "Push failed. This might be because:" -ForegroundColor Yellow
    Write-Host "  1. You're behind origin/main (need to pull first)" -ForegroundColor Cyan
    Write-Host "  2. Need to force push (if history diverged)" -ForegroundColor Cyan
    Write-Host "" -ForegroundColor Yellow
    Write-Host "Try one of these:" -ForegroundColor Yellow
    Write-Host "  git pull origin main --rebase" -ForegroundColor Cyan
    Write-Host "  git push origin main --force-with-lease" -ForegroundColor Cyan
    exit 1
}

# Step 6: Summary
Write-Host "`n=== SUCCESS ===" -ForegroundColor Green
Write-Host "" -ForegroundColor Green
Write-Host "Branch: main" -ForegroundColor Cyan
Write-Host "Remote: zapabob/codex" -ForegroundColor Cyan
Write-Host "Status: Pushed successfully" -ForegroundColor Green
Write-Host "" -ForegroundColor Green

Write-Host "Latest commits:" -ForegroundColor Yellow
$commits = git log --oneline -5 2>$null
$commits | ForEach-Object { Write-Host "  $_" -ForegroundColor Cyan }

Write-Host "" -ForegroundColor Green
Write-Host "View on GitHub: https://github.com/zapabob/codex" -ForegroundColor Cyan
Write-Host "" -ForegroundColor Green

Write-Host "Next step:" -ForegroundColor Yellow
Write-Host "  1. Go to https://github.com/zapabob/codex" -ForegroundColor Cyan
Write-Host "  2. Click 'Contribute' -> 'Open pull request'" -ForegroundColor Cyan
Write-Host "  3. Base: openai/codex main <- Compare: zapabob/codex main" -ForegroundColor Cyan
Write-Host "  4. Copy PULL_REQUEST.md content" -ForegroundColor Cyan
Write-Host "  5. Create PR!" -ForegroundColor Cyan
Write-Host "" -ForegroundColor Green
Write-Host "All done!" -ForegroundColor Green

