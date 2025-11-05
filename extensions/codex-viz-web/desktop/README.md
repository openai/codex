# Codex Viz Desktop

OS常駐型デスクトップクライアント（Electron）

## 🎯 Features

### デスクトップアプリ
- **システムトレイ常駐**: バックグラウンド実行
- **自動起動**: OS起動時に自動スタート
- **ネイティブ通知**: デスクトップ通知
- **最近開いたリポジトリ**: クイックアクセス

### Phase 3 コラボレーション
- **コメント機能**: コミットへのアノテーション
- **共有リンク**: ビューのURL共有
- **マルチユーザー**: リアルタイム協調閲覧（将来実装）

## 🚀 Development

```bash
cd extensions/codex-viz-web/desktop

# Install dependencies
npm install

# Run in development mode
npm run dev

# Build
npm run build

# Package for current platform
npm run package

# Package for specific platform
npm run package:win   # Windows
npm run package:mac   # macOS
npm run package:linux # Linux
```

## 📦 Installation

### Windows

```
codex-viz-setup-0.2.0.exe  # Installer
codex-viz-0.2.0-portable.exe  # Portable
```

### macOS

```
codex-viz-0.2.0.dmg  # Disk image
codex-viz-0.2.0-mac.zip  # Archive
```

### Linux

```
codex-viz-0.2.0.AppImage  # AppImage
codex-viz_0.2.0_amd64.deb  # Debian package
```

## 🎨 System Tray

右クリックメニュー:
- Show Codex Viz
- Recent Repositories (最大5件)
- Settings
  - Auto-start on login
  - Minimize to tray
  - Enable notifications
- Quit

ダブルクリック: ウィンドウ表示

## 🔔 Notifications

以下の場合にデスクトップ通知:
- 新規コミット検出
- ブランチ作成/削除
- ファイル変更（大量）
- 共有リンク作成完了

## 💾 Data Persistence

**electron-store** でデータ永続化:
- ウィンドウサイズ/位置
- 最近開いたリポジトリ（10件）
- ユーザー設定
- ブックマーク

保存先:
- Windows: `%APPDATA%\codex-viz\config.json`
- macOS: `~/Library/Application Support/codex-viz/config.json`
- Linux: `~/.config/codex-viz/config.json`

## 🔄 Auto-update

electron-updater で自動更新:
1. 起動時に更新確認
2. 新バージョンあり → 通知
3. ダウンロード完了 → インストール確認
4. 再起動でアップデート

## 🛠️ Architecture

```
Desktop App (Electron)
  ↓
Main Process
  ├── Backend Server (Rust) 起動/停止管理
  ├── System Tray 管理
  ├── IPC ハンドラ
  └── Auto-updater
  ↓
Renderer Process (React + Three.js)
  ├── フロントエンド (既存)
  └── Electron API 連携
```

## 📖 API

### Preload API

```typescript
window.electronAPI.getStore(key)
window.electronAPI.setStore(key, value)
window.electronAPI.addRecentRepo(path)
window.electronAPI.showNotification({ title, body })
window.electronAPI.minimizeToTray()
window.electronAPI.onOpenRepo(callback)
```

## 🎯 Keyboard Shortcuts (Desktop)

| Shortcut | Action |
|----------|--------|
| Ctrl+O | Open Repository |
| Ctrl+W | Close Window |
| Ctrl+M | Minimize to Tray |
| Ctrl+, | Settings |
| Ctrl+Q | Quit Application |

## 🔐 Security

- **contextIsolation**: true
- **nodeIntegration**: false
- **sandbox**: enabled
- **webSecurity**: true

## 📝 License

Same as parent project (Apache 2.0)

