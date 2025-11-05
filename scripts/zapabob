# OpenAI PR branch creation and patch application script
# Create a clean branch from upstream/main and apply changes

Write-Host "Starting OpenAI PR branch creation..." -ForegroundColor Green

# Set repository path
$repoPath = "C:\Users\downl\Desktop\codex-main\codex-main"
Set-Location $repoPath

Write-Host "Working directory: $repoPath" -ForegroundColor Cyan

# 1. Check current status
Write-Host "`nChecking current status..." -ForegroundColor Yellow
git status --short

# 2. Check if branch already exists
$branchExists = git branch --list "feature/openai-pr-clean"
if ($branchExists) {
    Write-Host "`nBranch feature/openai-pr-clean already exists" -ForegroundColor Yellow
    Write-Host "Currently on this branch" -ForegroundColor Cyan
} else {
    Write-Host "`nBranch feature/openai-pr-clean not found" -ForegroundColor Red
    Write-Host "Creating from upstream/main..." -ForegroundColor Yellow
    
    # Fetch upstream
    Write-Host "`nFetching upstream..." -ForegroundColor Cyan
    git fetch upstream
    
    # Create new branch
    Write-Host "`nCreating new branch..." -ForegroundColor Cyan
    git checkout -b feature/openai-pr-clean upstream/main
}

# 3. Check for patch file
Write-Host "`nChecking for patch file..." -ForegroundColor Yellow
if (Test-Path "openai-pr-changes.patch") {
    $patchSize = (Get-Item "openai-pr-changes.patch").Length
    Write-Host "Found openai-pr-changes.patch (size: $patchSize bytes)" -ForegroundColor Green
    
    # 4. Apply patch
    Write-Host "`nApplying changes..." -ForegroundColor Yellow
    git apply openai-pr-changes.patch
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Patch applied successfully!" -ForegroundColor Green
    } else {
        Write-Host "Patch application failed" -ForegroundColor Red
        Write-Host "Manual application may be required" -ForegroundColor Yellow
    }
} else {
    Write-Host "openai-pr-changes.patch not found" -ForegroundColor Red
    Write-Host "Please create the patch file first" -ForegroundColor Yellow
}

# 5. Check status after applying
Write-Host "`nStatus after applying changes..." -ForegroundColor Yellow
git status --short | Select-Object -First 20

# 6. Count modified files
Write-Host "`nStatistics..." -ForegroundColor Cyan
$modifiedFiles = git status --short | Measure-Object -Line
Write-Host "Modified files: $($modifiedFiles.Lines)" -ForegroundColor Green

# 7. Next steps
Write-Host "`nNext steps:" -ForegroundColor Yellow
Write-Host "1. git add -A" -ForegroundColor Cyan
Write-Host "2. git commit -m ""feat: add Multi-Agent, Deep Research, Security, npm distribution""" -ForegroundColor Cyan
Write-Host "3. git push origin feature/openai-pr-clean" -ForegroundColor Cyan
Write-Host "4. Create PR on GitHub: openai/codex <- zapabob/codex:feature/openai-pr-clean" -ForegroundColor Cyan

Write-Host "`nScript completed!" -ForegroundColor Green
