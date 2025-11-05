# 🔒 Codex環境変数セットアップスクリプト
# Windows PowerShell用
# 作成日: 2025-11-02
# バージョン: v0.56.0-zapabob

<#
.SYNOPSIS
    Codexの環境変数を対話的に設定するスクリプト

.DESCRIPTION
    このスクリプトは、Codexとその関連MCPサーバーで使用する環境変数を
    ユーザーフレンドリーに設定します。

.EXAMPLE
    .\setup-env-vars.ps1
    対話的に環境変数を設定

.EXAMPLE
    .\setup-env-vars.ps1 -Permanent
    システム環境変数として永続的に設定（管理者権限推奨）

.NOTES
    セキュリティ: このスクリプトはAPIキーを平文で扱いません。
    入力されたAPIキーは環境変数として設定され、ファイルには保存されません。
#>

param(
    [switch]$Permanent,  # システム環境変数として永続化
    [switch]$Profile,    # PowerShell Profileに追加
    [switch]$ShowCurrent # 現在の環境変数を表示
)

# カラー出力関数
function Write-ColorOutput {
    param(
        [string]$Message,
        [string]$Color = "White"
    )
    Write-Host $Message -ForegroundColor $Color
}

# ロゴ表示
function Show-Logo {
    Write-ColorOutput @"

╔═══════════════════════════════════════════════════════════╗
║                                                           ║
║   🔒 Codex環境変数セットアップスクリプト                  ║
║                                                           ║
║   バージョン: v0.56.0-zapabob                            ║
║   作成日: 2025-11-02                                      ║
║                                                           ║
╚═══════════════════════════════════════════════════════════╝

"@ -Color Cyan
}

# 現在の環境変数を表示
function Show-CurrentEnvVars {
    Write-ColorOutput "`n📋 現在の環境変数設定:" -Color Yellow
    Write-ColorOutput "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -Color Gray

    $envVars = @(
        "CODEX_API_KEY",
        "OPENAI_API_KEY",
        "GITHUB_TOKEN",
        "GEMINI_API_KEY",
        "GOOGLE_AI_STUDIO_API_KEY",
        "BRAVE_API_KEY",
        "SLACK_WEBHOOK_URL"
    )

    foreach ($var in $envVars) {
        $value = [Environment]::GetEnvironmentVariable($var, "Process")
        if ($value) {
            # セキュリティのため、最初の10文字のみ表示
            $masked = $value.Substring(0, [Math]::Min(10, $value.Length)) + "..." + 
                      "(" + $value.Length + " chars)"
            Write-ColorOutput "  ✅ $var = $masked" -Color Green
        } else {
            Write-ColorOutput "  ❌ $var = (未設定)" -Color Red
        }
    }

    Write-ColorOutput "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`n" -Color Gray
}

# APIキー検証
function Test-ApiKey {
    param(
        [string]$Key,
        [string]$Type
    )

    switch ($Type) {
        "OPENAI" {
            return $Key -match "^sk-(proj-)?[A-Za-z0-9]{20,}$"
        }
        "GITHUB" {
            return $Key -match "^ghp_[A-Za-z0-9]{36,}$"
        }
        "GEMINI" {
            return $Key -match "^AIzaSy[A-Za-z0-9_-]{33,}$"
        }
        "BRAVE" {
            return $Key -match "^BSA[A-Za-z0-9_-]{20,}$"
        }
        default {
            return $true
        }
    }
}

# 環境変数設定関数
function Set-EnvVariable {
    param(
        [string]$Name,
        [string]$Value,
        [bool]$IsPermanent = $false,
        [bool]$IsProfile = $false
    )

    if ($IsPermanent) {
        # システム環境変数として永続化
        [System.Environment]::SetEnvironmentVariable($Name, $Value, [System.EnvironmentVariableTarget]::User)
        Write-ColorOutput "  💾 システム環境変数として保存しました: $Name" -Color Green
    } 
    elseif ($IsProfile) {
        # PowerShell Profileに追加
        $profileLine = "`$env:$Name = `"$Value`""
        Add-Content -Path $PROFILE -Value $profileLine
        Write-ColorOutput "  📝 PowerShell Profileに追加しました: $Name" -Color Green
    }
    else {
        # 現在のセッションのみ
        [System.Environment]::SetEnvironmentVariable($Name, $Value, [System.EnvironmentVariableTarget]::Process)
        Write-ColorOutput "  ✅ 現在のセッションに設定しました: $Name" -Color Green
    }
}

# メイン処理
function Main {
    Show-Logo

    if ($ShowCurrent) {
        Show-CurrentEnvVars
        return
    }

    Write-ColorOutput "このスクリプトは、Codexで使用する環境変数を設定します。" -Color White
    Write-ColorOutput "APIキーを入力してください（スキップする場合はEnterキーを押してください）`n" -Color Gray

    # 設定モード選択
    if ($Permanent) {
        Write-ColorOutput "📌 設定モード: システム環境変数（永続化）" -Color Yellow
        Write-ColorOutput "   ※ PowerShell再起動後も有効" -Color Gray
    }
    elseif ($Profile) {
        Write-ColorOutput "📌 設定モード: PowerShell Profile" -Color Yellow
        Write-ColorOutput "   ※ PowerShell起動時に自動読み込み" -Color Gray
    }
    else {
        Write-ColorOutput "📌 設定モード: 現在のセッション（一時的）" -Color Yellow
        Write-ColorOutput "   ※ PowerShell再起動後は再設定が必要" -Color Gray
        Write-ColorOutput "   永続化する場合: .\setup-env-vars.ps1 -Permanent" -Color Gray
    }

    Write-ColorOutput "`n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`n" -Color Gray

    # 環境変数設定
    $envVarsToSet = @()

    # 1. CODEX_API_KEY（最優先）
    Write-ColorOutput "🔑 CODEX_API_KEY（推奨）" -Color Cyan
    Write-ColorOutput "   用途: Codex専用OpenAI APIキー（最優先）" -Color Gray
    Write-ColorOutput "   形式: sk-proj-XXXXXXXXXXXXXXXXXXXX" -Color Gray
    $codexApiKey = Read-Host "   入力"
    if ($codexApiKey -and (Test-ApiKey -Key $codexApiKey -Type "OPENAI")) {
        $envVarsToSet += @{Name = "CODEX_API_KEY"; Value = $codexApiKey}
    }
    elseif ($codexApiKey) {
        Write-ColorOutput "   ⚠️ 警告: OpenAI APIキーの形式が正しくありません" -Color Yellow
        $confirm = Read-Host "   それでも設定しますか？ (y/N)"
        if ($confirm -eq "y" -or $confirm -eq "Y") {
            $envVarsToSet += @{Name = "CODEX_API_KEY"; Value = $codexApiKey}
        }
    }

    Write-ColorOutput ""

    # 2. OPENAI_API_KEY（フォールバック）
    Write-ColorOutput "🔑 OPENAI_API_KEY（フォールバック）" -Color Cyan
    Write-ColorOutput "   用途: OpenAI APIキー（CODEX_API_KEY未設定時に使用）" -Color Gray
    Write-ColorOutput "   形式: sk-proj-XXXXXXXXXXXXXXXXXXXX" -Color Gray
    $openaiApiKey = Read-Host "   入力"
    if ($openaiApiKey -and (Test-ApiKey -Key $openaiApiKey -Type "OPENAI")) {
        $envVarsToSet += @{Name = "OPENAI_API_KEY"; Value = $openaiApiKey}
    }
    elseif ($openaiApiKey) {
        Write-ColorOutput "   ⚠️ 警告: OpenAI APIキーの形式が正しくありません" -Color Yellow
        $confirm = Read-Host "   それでも設定しますか？ (y/N)"
        if ($confirm -eq "y" -or $confirm -eq "Y") {
            $envVarsToSet += @{Name = "OPENAI_API_KEY"; Value = $openaiApiKey}
        }
    }

    Write-ColorOutput ""

    # 3. GITHUB_TOKEN（任意）
    Write-ColorOutput "🔑 GITHUB_TOKEN（任意）" -Color Cyan
    Write-ColorOutput "   用途: GitHub MCP Server（PR/Issue管理）" -Color Gray
    Write-ColorOutput "   形式: ghp_XXXXXXXXXXXXXXXXXXXX" -Color Gray
    $githubToken = Read-Host "   入力"
    if ($githubToken -and (Test-ApiKey -Key $githubToken -Type "GITHUB")) {
        $envVarsToSet += @{Name = "GITHUB_TOKEN"; Value = $githubToken}
    }
    elseif ($githubToken) {
        Write-ColorOutput "   ⚠️ 警告: GitHub Tokenの形式が正しくありません" -Color Yellow
        $confirm = Read-Host "   それでも設定しますか？ (y/N)"
        if ($confirm -eq "y" -or $confirm -eq "Y") {
            $envVarsToSet += @{Name = "GITHUB_TOKEN"; Value = $githubToken}
        }
    }

    Write-ColorOutput ""

    # 4. GEMINI_API_KEY（任意）
    Write-ColorOutput "🔑 GEMINI_API_KEY（任意）" -Color Cyan
    Write-ColorOutput "   用途: Gemini MCP Server（Google AI）" -Color Gray
    Write-ColorOutput "   形式: AIzaSyXXXXXXXXXXXXXXXXXXXXXXXXXXX" -Color Gray
    $geminiApiKey = Read-Host "   入力"
    if ($geminiApiKey -and (Test-ApiKey -Key $geminiApiKey -Type "GEMINI")) {
        $envVarsToSet += @{Name = "GEMINI_API_KEY"; Value = $geminiApiKey}
    }
    elseif ($geminiApiKey) {
        Write-ColorOutput "   ⚠️ 警告: Gemini APIキーの形式が正しくありません" -Color Yellow
        $confirm = Read-Host "   それでも設定しますか？ (y/N)"
        if ($confirm -eq "y" -or $confirm -eq "Y") {
            $envVarsToSet += @{Name = "GEMINI_API_KEY"; Value = $geminiApiKey}
        }
    }

    Write-ColorOutput ""

    # 5. BRAVE_API_KEY（任意）
    Write-ColorOutput "🔑 BRAVE_API_KEY（任意）" -Color Cyan
    Write-ColorOutput "   用途: Brave Search MCP Server（Web検索）" -Color Gray
    Write-ColorOutput "   形式: BSA_XXXXXXXXXXXXXXXXXXXX" -Color Gray
    $braveApiKey = Read-Host "   入力"
    if ($braveApiKey) {
        $envVarsToSet += @{Name = "BRAVE_API_KEY"; Value = $braveApiKey}
    }

    Write-ColorOutput ""

    # 6. SLACK_WEBHOOK_URL（任意）
    Write-ColorOutput "🔑 SLACK_WEBHOOK_URL（任意）" -Color Cyan
    Write-ColorOutput "   用途: Codex通知（Slack連携）" -Color Gray
    Write-ColorOutput "   形式: https://hooks.slack.com/services/XXX/XXX/XXX" -Color Gray
    $slackWebhookUrl = Read-Host "   入力"
    if ($slackWebhookUrl) {
        $envVarsToSet += @{Name = "SLACK_WEBHOOK_URL"; Value = $slackWebhookUrl}
    }

    Write-ColorOutput "`n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`n" -Color Gray

    # 設定確認
    if ($envVarsToSet.Count -eq 0) {
        Write-ColorOutput "❌ 設定する環境変数がありません。" -Color Red
        return
    }

    Write-ColorOutput "📋 設定する環境変数:" -Color Yellow
    foreach ($env in $envVarsToSet) {
        $masked = $env.Value.Substring(0, [Math]::Min(10, $env.Value.Length)) + "..." + 
                  "(" + $env.Value.Length + " chars)"
        Write-ColorOutput "  • $($env.Name) = $masked" -Color White
    }

    Write-ColorOutput ""
    $confirm = Read-Host "これらの環境変数を設定しますか？ (Y/n)"
    if ($confirm -eq "n" -or $confirm -eq "N") {
        Write-ColorOutput "❌ キャンセルされました。" -Color Red
        return
    }

    # 環境変数設定実行
    Write-ColorOutput "`n🚀 環境変数を設定中..." -Color Cyan
    foreach ($env in $envVarsToSet) {
        Set-EnvVariable -Name $env.Name -Value $env.Value -IsPermanent $Permanent -IsProfile $Profile
    }

    Write-ColorOutput "`n✅ 環境変数の設定が完了しました！`n" -Color Green

    # 現在の設定を表示
    Show-CurrentEnvVars

    # 次のステップ
    Write-ColorOutput "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -Color Gray
    Write-ColorOutput "📌 次のステップ:" -Color Yellow
    Write-ColorOutput "  1. Codexを起動して動作確認:" -Color White
    Write-ColorOutput "     codex exec `"echo test`"" -Color Cyan
    Write-ColorOutput ""
    Write-ColorOutput "  2. 環境変数を確認:" -Color White
    Write-ColorOutput "     .\setup-env-vars.ps1 -ShowCurrent" -Color Cyan
    Write-ColorOutput ""
    Write-ColorOutput "  3. 詳細なガイドを参照:" -Color White
    Write-ColorOutput "     _docs/2025-11-02_環境変数APIキー設定ガイド.md" -Color Cyan
    Write-ColorOutput "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`n" -Color Gray
}

# スクリプト実行
Main

