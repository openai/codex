# 🔧 .envファイル読み込みスクリプト
# PowerShell用
# 作成日: 2025-11-02
# バージョン: v0.56.0-zapabob

<#
.SYNOPSIS
    .envファイルから環境変数を読み込むスクリプト

.DESCRIPTION
    プロジェクトルートの.envファイルを読み込み、環境変数として設定します。
    コメント行（#で始まる行）と空行は無視されます。

.EXAMPLE
    .\zapabob\scripts\load-env.ps1
    .envファイルを現在のセッションに読み込み

.EXAMPLE
    .\zapabob\scripts\load-env.ps1 -Permanent
    システム環境変数として永続化

.EXAMPLE
    .\zapabob\scripts\load-env.ps1 -Verbose
    詳細な読み込み情報を表示

.NOTES
    セキュリティ: APIキーは環境変数として設定されますが、
    PowerShellセッション終了時に消去されます（-Permanentを除く）
#>

param(
    [switch]$Permanent,  # システム環境変数として永続化
    [switch]$Verbose,    # 詳細情報を表示
    [string]$EnvFile = ".env"  # .envファイルのパス
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
║   🔧 .env環境変数読み込みスクリプト                       ║
║                                                           ║
║   バージョン: v0.56.0-zapabob                            ║
║   作成日: 2025-11-02                                      ║
║                                                           ║
╚═══════════════════════════════════════════════════════════╝

"@ -Color Cyan
}

# .envファイルの存在確認
function Test-EnvFile {
    param([string]$Path)
    
    if (-not (Test-Path $Path)) {
        Write-ColorOutput "❌ エラー: .envファイルが見つかりません: $Path" -Color Red
        Write-ColorOutput "" -Color White
        Write-ColorOutput "📝 .envファイルの作成方法:" -Color Yellow
        Write-ColorOutput "  1. テンプレートをコピー:" -Color White
        Write-ColorOutput "     Copy-Item zapabob\templates\env.template .env" -Color Cyan
        Write-ColorOutput "  2. .envファイルを編集してAPIキーを設定" -Color White
        Write-ColorOutput "  3. このスクリプトを再実行" -Color White
        return $false
    }
    
    return $true
}

# 環境変数のパース
function Get-EnvVariables {
    param([string]$Path)
    
    $envVars = @()
    $lineNumber = 0
    
    Get-Content $Path | ForEach-Object {
        $lineNumber++
        $line = $_.Trim()
        
        # 空行とコメント行をスキップ
        if ([string]::IsNullOrWhiteSpace($line) -or $line.StartsWith("#")) {
            return
        }
        
        # KEY=VALUE 形式をパース
        if ($line -match '^([^=]+)=(.*)$') {
            $key = $matches[1].Trim()
            $value = $matches[2].Trim()
            
            # 値が空でない場合のみ追加
            if (-not [string]::IsNullOrWhiteSpace($value)) {
                $envVars += @{
                    Key = $key
                    Value = $value
                    LineNumber = $lineNumber
                }
                
                if ($Verbose) {
                    $maskedValue = if ($value.Length -gt 10) {
                        $value.Substring(0, 10) + "..." + "($($value.Length) chars)"
                    } else {
                        "***"
                    }
                    Write-ColorOutput "  📝 Line $lineNumber : $key = $maskedValue" -Color Gray
                }
            }
        }
        else {
            Write-ColorOutput "  ⚠️  Line $lineNumber : 無効な形式をスキップ: $line" -Color Yellow
        }
    }
    
    return $envVars
}

# 環境変数を設定
function Set-EnvVariables {
    param(
        [array]$Variables,
        [bool]$IsPermanent
    )
    
    $successCount = 0
    $failCount = 0
    
    foreach ($var in $Variables) {
        try {
            if ($IsPermanent) {
                # システム環境変数として永続化
                [System.Environment]::SetEnvironmentVariable(
                    $var.Key, 
                    $var.Value, 
                    [System.EnvironmentVariableTarget]::User
                )
                
                if ($Verbose) {
                    Write-ColorOutput "  💾 [永続] $($var.Key)" -Color Green
                }
            }
            else {
                # 現在のセッションのみ
                [System.Environment]::SetEnvironmentVariable(
                    $var.Key, 
                    $var.Value, 
                    [System.EnvironmentVariableTarget]::Process
                )
                
                if ($Verbose) {
                    Write-ColorOutput "  ✅ [一時] $($var.Key)" -Color Green
                }
            }
            
            $successCount++
        }
        catch {
            Write-ColorOutput "  ❌ エラー: $($var.Key) の設定に失敗しました" -Color Red
            Write-ColorOutput "     詳細: $($_.Exception.Message)" -Color Red
            $failCount++
        }
    }
    
    return @{
        Success = $successCount
        Failed = $failCount
    }
}

# 設定確認
function Show-SetVariables {
    param([array]$Variables)
    
    Write-ColorOutput "`n📋 設定された環境変数:" -Color Yellow
    Write-ColorOutput "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -Color Gray
    
    foreach ($var in $Variables) {
        $currentValue = [Environment]::GetEnvironmentVariable($var.Key, "Process")
        if ($currentValue) {
            $masked = if ($currentValue.Length -gt 10) {
                $currentValue.Substring(0, 10) + "..." + "($($currentValue.Length) chars)"
            } else {
                "***"
            }
            Write-ColorOutput "  ✅ $($var.Key) = $masked" -Color Green
        }
        else {
            Write-ColorOutput "  ❌ $($var.Key) = (設定失敗)" -Color Red
        }
    }
    
    Write-ColorOutput "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`n" -Color Gray
}

# メイン処理
function Main {
    Show-Logo
    
    # .envファイルの存在確認
    if (-not (Test-EnvFile -Path $EnvFile)) {
        exit 1
    }
    
    Write-ColorOutput "📂 .envファイル: $EnvFile" -Color White
    
    if ($Permanent) {
        Write-ColorOutput "📌 設定モード: システム環境変数（永続化）" -Color Yellow
        Write-ColorOutput "   ※ PowerShell再起動後も有効" -Color Gray
    }
    else {
        Write-ColorOutput "📌 設定モード: 現在のセッション（一時的）" -Color Yellow
        Write-ColorOutput "   ※ PowerShell終了時に消去されます" -Color Gray
    }
    
    Write-ColorOutput "`n🔍 .envファイルを解析中..." -Color Cyan
    
    # 環境変数をパース
    $envVars = Get-EnvVariables -Path $EnvFile
    
    if ($envVars.Count -eq 0) {
        Write-ColorOutput "`n⚠️  警告: 有効な環境変数が見つかりませんでした" -Color Yellow
        Write-ColorOutput "   .envファイルにKEY=VALUE形式で記述してください" -Color Gray
        exit 1
    }
    
    Write-ColorOutput "`n✅ $($envVars.Count) 個の環境変数を検出しました`n" -Color Green
    
    # 確認プロンプト
    if (-not $Verbose) {
        Write-ColorOutput "📋 読み込む環境変数:" -Color Yellow
        foreach ($var in $envVars) {
            Write-ColorOutput "  • $($var.Key)" -Color White
        }
        Write-ColorOutput ""
    }
    
    $confirm = Read-Host "これらの環境変数を設定しますか？ (Y/n)"
    if ($confirm -eq "n" -or $confirm -eq "N") {
        Write-ColorOutput "❌ キャンセルされました。" -Color Red
        exit 0
    }
    
    # 環境変数を設定
    Write-ColorOutput "`n🚀 環境変数を設定中..." -Color Cyan
    $result = Set-EnvVariables -Variables $envVars -IsPermanent $Permanent
    
    # 結果表示
    Write-ColorOutput "`n✅ 環境変数の設定が完了しました！" -Color Green
    Write-ColorOutput "   成功: $($result.Success) 個" -Color Green
    if ($result.Failed -gt 0) {
        Write-ColorOutput "   失敗: $($result.Failed) 個" -Color Red
    }
    
    # 設定確認
    Show-SetVariables -Variables $envVars
    
    # 次のステップ
    Write-ColorOutput "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -Color Gray
    Write-ColorOutput "📌 次のステップ:" -Color Yellow
    Write-ColorOutput "  1. 環境変数を確認:" -Color White
    Write-ColorOutput "     .\zapabob\scripts\setup-env-vars.ps1 -ShowCurrent" -Color Cyan
    Write-ColorOutput ""
    Write-ColorOutput "  2. Codexを起動:" -Color White
    Write-ColorOutput "     codex exec `"echo test`"" -Color Cyan
    Write-ColorOutput ""
    
    if (-not $Permanent) {
        Write-ColorOutput "  ⚠️  注意: 現在のセッションのみ有効です" -Color Yellow
        Write-ColorOutput "     永続化する場合: .\zapabob\scripts\load-env.ps1 -Permanent" -Color Gray
    }
    
    Write-ColorOutput "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`n" -Color Gray
}

# スクリプト実行
try {
    Main
}
catch {
    Write-ColorOutput "`n❌ エラーが発生しました:" -Color Red
    Write-ColorOutput "   $($_.Exception.Message)" -Color Red
    Write-ColorOutput "`n詳細:" -Color Yellow
    Write-ColorOutput $_.Exception.StackTrace -Color Gray
    exit 1
}

