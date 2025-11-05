# Codex Tauri - Installation Guide

Complete installation guide for Codex AI-Native OS resident GUI client.

## Prerequisites

### Required Software

1. **Node.js 18+**
   - Download: https://nodejs.org/
   - Verify: `node --version`

2. **Rust 1.70+**
   - Download: https://rustup.rs/
   - Verify: `rustc --version`

3. **Windows 10/11**
   - Visual Studio Build Tools 2019+ (for native modules)
   - Windows SDK

### Optional (for Development)

- Git
- Visual Studio Code with Rust Analyzer extension

## Installation from MSI

### Step 1: Download Installer

Download the latest `Codex_0.1.0_x64.msi` from:
- GitHub Releases (when available)
- Or build from source (see below)

### Step 2: Run Installer

1. Double-click the MSI file
2. Follow the installation wizard
3. Choose installation directory (default: `C:\Program Files\Codex`)
4. Complete installation

### Step 3: First Launch

1. Find "Codex" in Start Menu
2. Or look for system tray icon (running in background)
3. Right-click tray icon → "Dashboard を開く"

## Building from Source

### Step 1: Clone Repository

```bash
cd C:\Users\YourName\Projects
git clone https://github.com/zapabob/codex.git
cd codex/codex-tauri
```

### Step 2: Install Dependencies

```powershell
npm install
```

### Step 3: Build Application

#### Development Build (faster)

```powershell
npm run tauri:dev
```

#### Production Build (optimized)

```powershell
.\build.ps1
```

Or manually:

```powershell
npm run tauri build
```

### Step 4: Find Installer

MSI installer will be located at:

```
codex-tauri\src-tauri\target\release\bundle\msi\Codex_0.1.0_x64.msi
```

## Post-Installation Setup

### 1. Configure Workspace

1. Open Codex Dashboard
2. Go to Dashboard page
3. Enter your workspace path (e.g., `C:\Users\YourName\Projects\myproject`)
4. Click "Start Monitoring"

### 2. Enable Auto-start (Optional)

1. Go to Settings
2. Toggle "Auto-start on Windows boot"
3. Codex will launch automatically on next boot

### 3. Configure Codex Core

Ensure Codex CLI is installed and accessible:

```powershell
codex --version
```

If not installed, build Codex CLI:

```powershell
cd ..\codex-rs
cargo build --release -p codex-cli
cargo install --path cli --force
```

## Troubleshooting

### Issue: "Codex binary not found"

**Solution**: Install Codex CLI or add to PATH

```powershell
# Option 1: Install from workspace
cd codex-rs
cargo install --path cli --force

# Option 2: Add to PATH
$env:PATH += ";C:\path\to\codex-rs\target\release"
```

### Issue: "Failed to initialize database"

**Solution**: Check permissions for AppData directory

```powershell
# Database location
%APPDATA%\codex\codex.db

# Ensure directory exists and is writable
New-Item -ItemType Directory -Force -Path "$env:APPDATA\codex"
```

### Issue: "System tray icon not appearing"

**Solution**: Restart Windows Explorer

```powershell
Stop-Process -Name explorer -Force
Start-Process explorer
```

### Issue: "File watcher not starting"

**Solution**: 
1. Check workspace path is valid
2. Ensure you have read permissions
3. Check firewall/antivirus settings

## Uninstallation

### Via Windows Settings

1. Open Settings → Apps → Installed apps
2. Find "Codex"
3. Click "Uninstall"

### Manual Cleanup

Remove application data:

```powershell
Remove-Item -Recurse -Force "$env:APPDATA\codex"
```

Remove from startup (if enabled):

```powershell
# Remove registry entry
Remove-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name "Codex"
```

## Updating

### Automatic Updates

Codex checks for updates on startup. When available:

1. Notification will appear
2. Click to download and install
3. Application will restart automatically

### Manual Update

1. Download new MSI installer
2. Run installer (will upgrade existing installation)
3. Restart Codex

## Security Notes

### Code Signing

Production releases should be signed with a valid certificate:

```powershell
# Sign MSI (requires certificate)
signtool sign /v /f MyCert.pfx /p password /t http://timestamp.digicert.com Codex_0.1.0_x64.msi
```

### Permissions

Codex requires:
- ✅ Read access to workspace directories
- ✅ Write access to AppData
- ✅ Registry write (HKCU only, for autostart)
- ❌ Administrator privileges NOT required

## Support

For issues or questions:
- GitHub Issues: https://github.com/zapabob/codex/issues
- Documentation: https://github.com/zapabob/codex/blob/main/README.md

## Next Steps

After installation:
1. Read [README.md](./README.md) for feature overview
2. Configure workspace monitoring
3. Create your first Blueprint
4. Explore Deep Research capabilities

Enjoy using Codex AI-Native OS! 🚀

