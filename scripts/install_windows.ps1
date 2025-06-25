# PowerShell script to configure Codex on Windows
param(
    [switch]$Audio
)

Write-Host "Codex CLI Windows setup" -ForegroundColor Cyan

# Verify Node.js presence and version
$node = Get-Command node -ErrorAction SilentlyContinue
if (-not $node) {
    Write-Host "Node.js 22+ is required. Please install it from https://nodejs.org and re-run this script." -ForegroundColor Red
    exit 1
}
node "$PSScriptRoot\check_node_version.js"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$nodeDir = Split-Path $node.Source
if ($env:PATH -notlike "*$nodeDir*") {
    Write-Host "Adding Node.js directory to PATH" -ForegroundColor Yellow
    [Environment]::SetEnvironmentVariable("PATH", $env:PATH + ";$nodeDir", "User")
}

# Ensure ~/.codex exists
$codexDir = Join-Path $env:USERPROFILE ".codex"
if (!(Test-Path $codexDir)) {
    New-Item -ItemType Directory -Path $codexDir | Out-Null
}

# Optional audio setup
$installAudio = $Audio
if (-not $Audio) {
    $resp = Read-Host "Install optional audio packages (pyttsx3 & speech_recognition)? [y/N]"
    if ($resp -match '^[Yy]') { $installAudio = $true }
}
if ($installAudio) {
    try {
        pip install --user pyttsx3 SpeechRecognition | Out-Null
        Write-Host "Audio packages installed" -ForegroundColor Green
    } catch {
        Write-Host "Failed to install audio packages: $_" -ForegroundColor Red
    }
}

# Choose provider
$providers = @("OpenAI","Claude","Gemini")
Write-Host "Select default provider:" -ForegroundColor Cyan
for ($i=0; $i -lt $providers.Count; $i++) { Write-Host "  [$($i+1)] $($providers[$i])" }
$choice = Read-Host "Enter choice (1-3)"; if (-not $choice) { $choice = '1' }
$provider = $providers[[int]$choice - 1]
$config = @{ provider = $provider }

switch ($provider) {
    'OpenAI' { $config.model = 'gpt-4o' }
    'Claude' { $config.model = 'claude-3-opus-20240229' }
    'Gemini' {
        $token = Read-Host "Gemini API key"
        [Environment]::SetEnvironmentVariable('GEMINI_API_KEY',$token,'User')
        $models = @("models/gemini-1.5-pro-latest")
        try {
            $resp = Invoke-RestMethod "https://generativelanguage.googleapis.com/v1/models?key=$token"
            if ($resp.models) { $models = $resp.models | ForEach-Object { $_.name } }
        } catch { Write-Host "Could not fetch Gemini models: $_" -ForegroundColor Yellow }
        for ($i=0; $i -lt $models.Count; $i++) { Write-Host "  [$($i+1)] $($models[$i])" }
        $mChoice = Read-Host "Select Gemini model"; if (-not $mChoice) { $mChoice = '1' }
        $config.model = $models[[int]$mChoice - 1]
        $config.providers = @{ gemini = @{ name='Gemini'; baseURL='https://generativelanguage.googleapis.com/v1beta/openai'; envKey='GEMINI_API_KEY' } }
    }
}

$configPath = Join-Path $codexDir "config.json"
$config | ConvertTo-Json -Depth 3 | Out-File $configPath -Encoding UTF8
Write-Host "Wrote configuration to $configPath" -ForegroundColor Green

# Create default AGENTS.md with bilingual instructions
$agentsPath = Join-Path $codexDir "AGENTS.md"
@'
# Windows instructions (English)
- Explain any command requiring administrator rights and ask for confirmation using the ask-user tool.
- Reply in English when the prompt is in English.

---

# Instructions Windows (Français)
- Explique chaque commande nécessitant des privilèges administrateur et demande une confirmation avec l'outil ask-user.
- Réponds en français lorsque la demande est en français.
'@ | Out-File $agentsPath -Encoding UTF8
Write-Host "Wrote default instructions to $agentsPath" -ForegroundColor Green

# Ensure npm global bin is in PATH
$npmBin = (npm bin -g).Trim()
if ($env:PATH -notlike "*$npmBin*") {
    Write-Host "Adding npm global bin to PATH" -ForegroundColor Yellow
    [Environment]::SetEnvironmentVariable("PATH", $env:PATH + ";$npmBin", "User")
}

# Optional CLI installation
$respCli = Read-Host "Install Codex CLI now? [Y/n]"
if ($respCli -match '^[Yy]' -or $respCli -eq '') {
    try {
        npm install -g github:damdam775/codex#codex_windows_version
        $codexCmd = Get-Command codex -ErrorAction SilentlyContinue
        if (-not $codexCmd) {
            Write-Host "CLI installed but 'codex' not found in PATH. Restart your terminal or check npm prefix." -ForegroundColor Yellow
        } else {
            Write-Host "Codex CLI installed" -ForegroundColor Green
        }
    } catch {
        Write-Host "Failed to install Codex CLI: $_" -ForegroundColor Red
    }
}

Write-Host "Installation complete. Restart your terminal for PATH changes to take effect." -ForegroundColor Cyan
