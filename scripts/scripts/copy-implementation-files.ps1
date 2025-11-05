# Copy implementation files from old branch to new fork-based branch
# Run this after setup-fork-and-pr.ps1

Write-Host "Copying implementation files..." -ForegroundColor Green

$repoPath = "C:\Users\downl\Desktop\codex-main\codex-main"
Set-Location $repoPath

# Check current branch
$currentBranch = git branch --show-current
Write-Host "Current branch: $currentBranch" -ForegroundColor Cyan

# Accept both branch names
$validBranches = @("feature/openai-pr-from-fork", "feature/openai-pr-clean")
if ($validBranches -notcontains $currentBranch) {
    Write-Host "ERROR: Must be on one of: feature/openai-pr-from-fork or feature/openai-pr-clean" -ForegroundColor Red
    Write-Host "Current branch: $currentBranch" -ForegroundColor Yellow
    exit 1
}

Write-Host "Using branch: $currentBranch (OK)" -ForegroundColor Green

# Create temporary directory for old files
$tempDir = "temp-old-implementation"
if (Test-Path $tempDir) {
    Remove-Item -Recurse -Force $tempDir
}
New-Item -ItemType Directory -Path $tempDir | Out-Null

Write-Host "`nStep 1: Saving old implementation to temp directory..." -ForegroundColor Yellow

# Checkout old branch files to temp
git checkout feature/multi-agent-security-npm-distribution -- `
    codex-rs/supervisor `
    codex-rs/deep-research `
    codex-rs/audit `
    codex-rs/mcp-server/src/supervisor_tool.rs `
    codex-rs/mcp-server/src/deep_research_tool.rs `
    codex-rs/mcp-server/src/supervisor_tool_handler.rs `
    codex-rs/mcp-server/src/deep_research_tool_handler.rs `
    codex-rs/mcp-server/src/lib.rs `
    codex-rs/mcp-server/Cargo.toml `
    codex-rs/Cargo.toml `
    _docs `
    CURSOR_IDE_SETUP.md `
    OPENAI_PR_GUIDE.md 2>$null

if ($LASTEXITCODE -eq 0) {
    Write-Host "Files copied successfully!" -ForegroundColor Green
} else {
    Write-Host "Some files may not exist, continuing..." -ForegroundColor Yellow
}

# Show what was copied
Write-Host "`nStep 2: Files staged for commit:" -ForegroundColor Yellow
git status --short

# Count files
$fileCount = (git status --short | Measure-Object -Line).Lines
Write-Host "`nTotal files changed: $fileCount" -ForegroundColor Cyan

Write-Host "`nStep 3: Review changes and commit:" -ForegroundColor Yellow
Write-Host "1. Review: git status" -ForegroundColor Cyan
Write-Host "2. Add: git add -A" -ForegroundColor Cyan
Write-Host "3. Commit: git commit -m ""feat: add Multi-Agent Supervisor, Deep Research, Security Profiles" -ForegroundColor Cyan
Write-Host "`nMulti-Agent Supervisor System with 8 specialized agents," -ForegroundColor Cyan
Write-Host "Deep Research with 3 strategies, Enhanced Security with 5 profiles," -ForegroundColor Cyan
Write-Host "Audit logging, Performance benchmarks, Cursor IDE integration""" -ForegroundColor Cyan
Write-Host "4. Push: git push origin feature/openai-pr-from-fork" -ForegroundColor Cyan

Write-Host "`nFile copying completed!" -ForegroundColor Green

