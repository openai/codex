# Setup proper fork and create PR branch
# This script helps create a proper fork-based PR to openai/codex

Write-Host "Setting up proper fork for OpenAI PR..." -ForegroundColor Green

$repoPath = "C:\Users\downl\Desktop\codex-main\codex-main"
Set-Location $repoPath

Write-Host "Working directory: $repoPath" -ForegroundColor Cyan

# Step 1: Check current remotes
Write-Host "`nStep 1: Current remotes" -ForegroundColor Yellow
git remote -v

# Step 2: Fetch all latest changes from upstream
Write-Host "`nStep 2: Fetching latest from upstream (openai/codex)..." -ForegroundColor Yellow
git fetch upstream

# Step 3: Create clean branch from upstream/main
Write-Host "`nStep 3: Creating clean branch from upstream/main..." -ForegroundColor Yellow

# Check if branch exists
$branchExists = git branch --list "feature/openai-pr-from-fork"
if ($branchExists) {
    Write-Host "Branch feature/openai-pr-from-fork already exists, deleting..." -ForegroundColor Yellow
    git branch -D feature/openai-pr-from-fork
}

# Create new branch
git checkout -b feature/openai-pr-from-fork upstream/main

Write-Host "Created branch: feature/openai-pr-from-fork" -ForegroundColor Green
Write-Host "This branch is based on: upstream/main (openai/codex)" -ForegroundColor Cyan

# Step 4: List files to copy from old implementation
Write-Host "`nStep 4: Files to copy from old implementation:" -ForegroundColor Yellow

$filesToCopy = @(
    "codex-rs/supervisor/*",
    "codex-rs/deep-research/*",
    "codex-rs/audit/*",
    "codex-rs/mcp-server/src/supervisor_tool.rs",
    "codex-rs/mcp-server/src/deep_research_tool.rs",
    "codex-rs/mcp-server/src/supervisor_tool_handler.rs",
    "codex-rs/mcp-server/src/deep_research_tool_handler.rs",
    "_docs/2025-10-08_*.md",
    "CURSOR_IDE_SETUP.md",
    "OPENAI_PR_GUIDE.md",
    "cursor-integration/*"
)

foreach ($file in $filesToCopy) {
    Write-Host "  - $file" -ForegroundColor Cyan
}

Write-Host "`nStep 5: Manual file copying required" -ForegroundColor Yellow
Write-Host "Please run the companion script: copy-implementation-files.ps1" -ForegroundColor Cyan

Write-Host "`nStep 6: After copying files:" -ForegroundColor Yellow
Write-Host "1. git add -A" -ForegroundColor Cyan
Write-Host "2. git commit -m ""feat: add Multi-Agent, Deep Research, Security""" -ForegroundColor Cyan
Write-Host "3. git push origin feature/openai-pr-from-fork" -ForegroundColor Cyan

Write-Host "`nCurrent branch:" -ForegroundColor Yellow
git branch --show-current

Write-Host "`nSetup completed! Ready for file copying." -ForegroundColor Green

