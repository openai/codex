# 🔢 バージョン更新スクリプト
# Usage: .\scripts\bump-version.ps1 <patch|minor|major>

param(
    [Parameter(Mandatory=$true)]
    [ValidateSet("patch", "minor", "major")]
    [string]$Type
)

# VERSION ファイル読み込み
$CurrentVersion = Get-Content -Path "VERSION" -ErrorAction Stop
$CurrentVersion = $CurrentVersion.Trim()

Write-Host "Current version: $CurrentVersion" -ForegroundColor Cyan

# バージョン解析
if ($CurrentVersion -match '^(\d+)\.(\d+)\.(\d+)(-(.+))?$') {
    $Major = [int]$Matches[1]
    $Minor = [int]$Matches[2]
    $Patch = [int]$Matches[3]
    $Suffix = $Matches[5]
} else {
    Write-Host "❌ Invalid version format: $CurrentVersion" -ForegroundColor Red
    exit 1
}

# 新しいバージョン計算
switch ($Type) {
    "patch" {
        $Patch++
    }
    "minor" {
        $Minor++
        $Patch = 0
    }
    "major" {
        $Major++
        $Minor = 0
        $Patch = 0
    }
}

# 新しいバージョン文字列
if ($Suffix) {
    $NewVersion = "${Major}.${Minor}.${Patch}-${Suffix}"
} else {
    $NewVersion = "${Major}.${Minor}.${Patch}"
}

Write-Host "New version:     $NewVersion" -ForegroundColor Green

# 確認
$Confirmation = Read-Host "Update VERSION file? (y/n)"
if ($Confirmation -ne 'y') {
    Write-Host "⚠️  Aborted" -ForegroundColor Yellow
    exit 0
}

# VERSION ファイル更新
Set-Content -Path "VERSION" -Value $NewVersion -Encoding UTF8 -NoNewline

Write-Host "✅ Version bumped: $CurrentVersion → $NewVersion" -ForegroundColor Green
Write-Host ""
Write-Host "次のステップ:" -ForegroundColor Cyan
Write-Host "  1. CHANGELOG.md を更新" -ForegroundColor Yellow
Write-Host "  2. codex-rs/Cargo.toml のバージョンを更新" -ForegroundColor Yellow
Write-Host "  3. codex-cli/package.json のバージョンを更新" -ForegroundColor Yellow
Write-Host "  4. git commit -m 'chore: bump version to $NewVersion'" -ForegroundColor Yellow

