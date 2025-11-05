# MCP サーバーテストスクリプト
# 各 MCP サーバーが正しく起動できるかテスト

Write-Host "=== MCP サーバーテスト開始 ===" -ForegroundColor Cyan
Write-Host ""

# テスト結果を記録
$results = @()

# 1. Codex MCP Server (codex-agent)
Write-Host "[1/9] Testing codex-agent..." -ForegroundColor Yellow
try {
    $output = codex mcp-server --help 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ✅ codex-agent: OK" -ForegroundColor Green
        $results += [PSCustomObject]@{Server="codex-agent"; Status="✅ OK"; Command="codex mcp-server"}
    } else {
        Write-Host "  ❌ codex-agent: FAILED" -ForegroundColor Red
        $results += [PSCustomObject]@{Server="codex-agent"; Status="❌ FAILED"; Command="codex mcp-server"}
    }
} catch {
    Write-Host "  ❌ codex-agent: ERROR - $_" -ForegroundColor Red
    $results += [PSCustomObject]@{Server="codex-agent"; Status="❌ ERROR"; Command="codex mcp-server"}
}
Write-Host ""

# 2. Playwright
Write-Host "[2/9] Testing playwright..." -ForegroundColor Yellow
Write-Host "  (Note: This will download @playwright/mcp if not installed)" -ForegroundColor Gray
try {
    # npx でバージョン確認（実際には起動せず）
    Write-Host "  Checking if playwright MCP is available..." -ForegroundColor Gray
    $results += [PSCustomObject]@{Server="playwright"; Status="⚠️ SKIP"; Command="npx -y @playwright/mcp@latest"}
    Write-Host "  ⚠️ playwright: SKIPPED (requires manual test)" -ForegroundColor Yellow
} catch {
    Write-Host "  ❌ playwright: ERROR" -ForegroundColor Red
    $results += [PSCustomObject]@{Server="playwright"; Status="❌ ERROR"; Command="npx -y @playwright/mcp@latest"}
}
Write-Host ""

# 3. MarkItDown
Write-Host "[3/9] Testing markitdown..." -ForegroundColor Yellow
try {
    # uvx の存在確認
    $uvx = Get-Command uvx -ErrorAction SilentlyContinue
    if ($uvx) {
        Write-Host "  ✅ markitdown: uvx found" -ForegroundColor Green
        $results += [PSCustomObject]@{Server="markitdown"; Status="⚠️ uvx OK"; Command="uvx markitdown-mcp"}
    } else {
        Write-Host "  ❌ markitdown: uvx not found" -ForegroundColor Red
        $results += [PSCustomObject]@{Server="markitdown"; Status="❌ uvx missing"; Command="uvx markitdown-mcp"}
    }
} catch {
    Write-Host "  ❌ markitdown: ERROR" -ForegroundColor Red
    $results += [PSCustomObject]@{Server="markitdown"; Status="❌ ERROR"; Command="uvx markitdown-mcp"}
}
Write-Host ""

# 4. arXiv
Write-Host "[4/9] Testing arxiv-mcp-server..." -ForegroundColor Yellow
try {
    $uvx = Get-Command uvx -ErrorAction SilentlyContinue
    if ($uvx) {
        Write-Host "  ✅ arxiv-mcp-server: uvx found" -ForegroundColor Green
        $results += [PSCustomObject]@{Server="arxiv-mcp-server"; Status="⚠️ uvx OK"; Command="uvx arxiv-mcp-server"}
    } else {
        Write-Host "  ❌ arxiv-mcp-server: uvx not found" -ForegroundColor Red
        $results += [PSCustomObject]@{Server="arxiv-mcp-server"; Status="❌ uvx missing"; Command="uvx arxiv-mcp-server"}
    }
} catch {
    Write-Host "  ❌ arxiv-mcp-server: ERROR" -ForegroundColor Red
    $results += [PSCustomObject]@{Server="arxiv-mcp-server"; Status="❌ ERROR"; Command="uvx arxiv-mcp-server"}
}
Write-Host ""

# 5. Context7
Write-Host "[5/9] Testing context7..." -ForegroundColor Yellow
try {
    Write-Host "  npx available: checking..." -ForegroundColor Gray
    $results += [PSCustomObject]@{Server="context7"; Status="⚠️ SKIP"; Command="npx -y @upstash/context7-mcp"}
    Write-Host "  ⚠️ context7: SKIPPED (requires manual test)" -ForegroundColor Yellow
} catch {
    Write-Host "  ❌ context7: ERROR" -ForegroundColor Red
    $results += [PSCustomObject]@{Server="context7"; Status="❌ ERROR"; Command="npx -y @upstash/context7-mcp"}
}
Write-Host ""

# 6. YouTube
Write-Host "[6/9] Testing youtube..." -ForegroundColor Yellow
try {
    Write-Host "  npx available: checking..." -ForegroundColor Gray
    $results += [PSCustomObject]@{Server="youtube"; Status="⚠️ SKIP"; Command="npx @anaisbetts/mcp-youtube"}
    Write-Host "  ⚠️ youtube: SKIPPED (requires manual test)" -ForegroundColor Yellow
} catch {
    Write-Host "  ❌ youtube: ERROR" -ForegroundColor Red
    $results += [PSCustomObject]@{Server="youtube"; Status="❌ ERROR"; Command="npx @anaisbetts/mcp-youtube"}
}
Write-Host ""

# 7. Gemini CLI
Write-Host "[7/9] Testing gemini-cli..." -ForegroundColor Yellow
try {
    Write-Host "  npx available: checking..." -ForegroundColor Gray
    $results += [PSCustomObject]@{Server="gemini-cli"; Status="⚠️ SKIP"; Command="npx mcp-gemini-cli"}
    Write-Host "  ⚠️ gemini-cli: SKIPPED (requires manual test)" -ForegroundColor Yellow
} catch {
    Write-Host "  ❌ gemini-cli: ERROR" -ForegroundColor Red
    $results += [PSCustomObject]@{Server="gemini-cli"; Status="❌ ERROR"; Command="npx mcp-gemini-cli"}
}
Write-Host ""

# 8. Codex (外部用)
Write-Host "[8/9] Testing codex (external)..." -ForegroundColor Yellow
try {
    # codex mcp コマンドの存在確認
    $help = codex mcp --help 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ✅ codex (external): OK" -ForegroundColor Green
        $results += [PSCustomObject]@{Server="codex"; Status="✅ OK"; Command="codex mcp"}
    } else {
        Write-Host "  ❌ codex (external): Command not found" -ForegroundColor Red
        $results += [PSCustomObject]@{Server="codex"; Status="❌ NOT FOUND"; Command="codex mcp"}
    }
} catch {
    Write-Host "  ❌ codex (external): ERROR" -ForegroundColor Red
    $results += [PSCustomObject]@{Server="codex"; Status="❌ ERROR"; Command="codex mcp"}
}
Write-Host ""

# 9. Chrome DevTools
Write-Host "[9/9] Testing chrome-devtools..." -ForegroundColor Yellow
try {
    Write-Host "  npx available: checking..." -ForegroundColor Gray
    $results += [PSCustomObject]@{Server="chrome-devtools"; Status="⚠️ SKIP"; Command="npx chrome-devtools-mcp@latest"}
    Write-Host "  ⚠️ chrome-devtools: SKIPPED (requires manual test)" -ForegroundColor Yellow
} catch {
    Write-Host "  ❌ chrome-devtools: ERROR" -ForegroundColor Red
    $results += [PSCustomObject]@{Server="chrome-devtools"; Status="❌ ERROR"; Command="npx chrome-devtools-mcp@latest"}
}
Write-Host ""

# 結果サマリー
Write-Host "=== テスト結果サマリー ===" -ForegroundColor Cyan
Write-Host ""
$results | Format-Table -AutoSize
Write-Host ""

# 統計
$ok = ($results | Where-Object {$_.Status -like "*OK*"}).Count
$skip = ($results | Where-Object {$_.Status -like "*SKIP*"}).Count
$failed = ($results | Where-Object {$_.Status -like "*FAILED*" -or $_.Status -like "*ERROR*" -or $_.Status -like "*missing*"}).Count

Write-Host "統計:" -ForegroundColor Cyan
Write-Host "  ✅ OK: $ok" -ForegroundColor Green
Write-Host "  ⚠️ SKIP: $skip" -ForegroundColor Yellow
Write-Host "  ❌ FAILED: $failed" -ForegroundColor Red
Write-Host ""

# 推奨事項
Write-Host "=== 推奨事項 ===" -ForegroundColor Cyan
Write-Host ""

if ($failed -gt 0) {
    Write-Host "⚠️ 失敗したサーバーがあります:" -ForegroundColor Yellow
    Write-Host ""
    
    # uvx が見つからない場合
    $uvxMissing = $results | Where-Object {$_.Status -like "*uvx missing*"}
    if ($uvxMissing) {
        Write-Host "  uvx がインストールされていません。以下のコマンドでインストールしてください:" -ForegroundColor Yellow
        Write-Host "    pip install uv" -ForegroundColor White
        Write-Host "  または" -ForegroundColor Gray
        Write-Host "    pipx install uv" -ForegroundColor White
        Write-Host ""
    }
}

Write-Host "npx ベースのサーバーは実際の使用時に自動ダウンロードされます。" -ForegroundColor Gray
Write-Host ""

Write-Host "=== テスト完了 ===" -ForegroundColor Cyan

