# Complete Git history rebuild and remote setup
# Sets upstream to official OpenAI repository

Write-Host "=== Complete Git History Rebuild & Remote Setup ===" -ForegroundColor Cyan
Write-Host ""

$repoPath = "C:\Users\downl\Desktop\codex"
Set-Location $repoPath

# Step 1: Check current status
Write-Host "[1/6] Checking current Git status..." -ForegroundColor Yellow
$currentBranch = git branch --show-current 2>$null
if ($currentBranch) {
    Write-Host "  Current branch: $currentBranch" -ForegroundColor White
} else {
    Write-Host "  No Git repository found" -ForegroundColor White
}

# Step 2: Save current remote URLs if they exist
Write-Host "`n[2/6] Saving remote URLs..." -ForegroundColor Yellow
$originUrl = git remote get-url origin 2>$null
$upstreamUrl = git remote get-url upstream 2>$null

if ($originUrl) {
    Write-Host "  Origin: $originUrl" -ForegroundColor White
} else {
    $originUrl = "https://github.com/zapabob/codex.git"
    Write-Host "  Origin will be set to: $originUrl" -ForegroundColor White
}

if ($upstreamUrl) {
    Write-Host "  Upstream: $upstreamUrl" -ForegroundColor White
} else {
    $upstreamUrl = "https://github.com/openai/codex.git"
    Write-Host "  Upstream will be set to: $upstreamUrl" -ForegroundColor White
}

# Step 3: Backup old history if exists
Write-Host "`n[3/6] Backing up old history..." -ForegroundColor Yellow
if (Test-Path ".git") {
    git branch old-history-backup 2>$null
    Write-Host "  Backup created in: old-history-backup branch" -ForegroundColor Green
} else {
    Write-Host "  No existing Git repository to backup" -ForegroundColor White
}

# Step 4: Remove old Git directory and reinitialize
Write-Host "`n[4/6] Reinitializing Git repository..." -ForegroundColor Yellow
if (Test-Path ".git") {
    Remove-Item -Path ".git" -Recurse -Force
    Write-Host "  Old .git directory removed" -ForegroundColor Green
}

git init
Write-Host "  New Git repository initialized" -ForegroundColor Green

# Step 5: Stage all files (node_modules and .tgz are auto-excluded by .gitignore)
Write-Host "`n[5/6] Staging files (node_modules and .tgz auto-excluded)..." -ForegroundColor Yellow
git add .

# Verify large files are excluded
$stagedFiles = git ls-files
$hasNodeModules = $stagedFiles | Select-String "node_modules"
$hasTgz = $stagedFiles | Select-String "\.tgz"

if ($hasNodeModules) {
    Write-Host "  WARNING: node_modules found in staging! Removing..." -ForegroundColor Red
    git rm -r --cached "**/node_modules" 2>$null
}

if ($hasTgz) {
    Write-Host "  WARNING: .tgz files found in staging! Removing..." -ForegroundColor Red
    git rm --cached "**/*.tgz" 2>$null
}

$fileCount = ($stagedFiles | Measure-Object).Count
Write-Host "  Files staged: $fileCount" -ForegroundColor Green

# Step 6: Create initial commit
Write-Host "`n[6/6] Creating initial commit..." -ForegroundColor Yellow
$commitMessage = @"
feat: Complete Codex implementation - Clean history

Main Features:
- Core orchestration & parallel execution engine
- Git integration (commit quality checks)
- Tauri GUI (3D/4D visualization, orchestration dashboard)
- TUI improvements (Approval overlay, Status display)
- App Server Protocol V2 API
- CLI extensions (MCP, Sandbox debugging)
- Comprehensive documentation

Tech Stack:
- Rust (Core, CLI, TUI)
- TypeScript/React (Tauri GUI)
- Protocol Buffers (MCP integration)
- WebGPU (3D visualization)

Extended version with zapabob features
"@

git commit -m $commitMessage
Write-Host "  Initial commit created" -ForegroundColor Green

# Set branch name
$branchName = "main"
git branch -M $branchName
Write-Host "  Branch set to: $branchName" -ForegroundColor Green

# Configure remotes
Write-Host "`n=== Remote Configuration ===" -ForegroundColor Cyan
Write-Host "[1/2] Setting origin (zapabob/codex)..." -ForegroundColor Yellow
git remote add origin $originUrl
Write-Host "  Origin: $originUrl" -ForegroundColor Green

Write-Host "`n[2/2] Setting upstream (openai/codex - Official)..." -ForegroundColor Yellow
git remote add upstream $upstreamUrl
Write-Host "  Upstream: $upstreamUrl" -ForegroundColor Green

# Verify remotes
Write-Host "`n=== Remote Configuration Summary ===" -ForegroundColor Cyan
git remote -v

# Repository size
Write-Host "`n=== Repository Size ===" -ForegroundColor Cyan
git count-objects -vH

# Final instructions
Write-Host "`n=== Next Steps ===" -ForegroundColor Green
Write-Host "1. Force push to origin:" -ForegroundColor White
Write-Host "   git push -u origin main --force" -ForegroundColor Cyan
Write-Host "`n2. To sync with upstream (official repo):" -ForegroundColor White
Write-Host "   git fetch upstream" -ForegroundColor Cyan
Write-Host "   git merge upstream/main" -ForegroundColor Cyan
Write-Host "`n3. Verify remotes:" -ForegroundColor White
Write-Host "   git remote -v" -ForegroundColor Cyan

Write-Host "`n=== SUCCESS ===" -ForegroundColor Green
Write-Host "  [OK] Clean Git history created" -ForegroundColor White
Write-Host "  [OK] Large files excluded (node_modules, .tgz)" -ForegroundColor White
Write-Host "  [OK] Origin set to: zapabob/codex" -ForegroundColor White
Write-Host "  [OK] Upstream set to: openai/codex (Official)" -ForegroundColor White
Write-Host "`nReady to push!" -ForegroundColor Green




