# Semantic Versioning Guide / セマンティックバージョニングガイド

**Current Version / 現在のバージョン**: `0.47.0-alpha.1`  
**Upstream Version / 上流バージョン**: `rust-v0.46.0-alpha.4`  
**Change Type / 変更タイプ**: MINOR (new features / 新機能追加)

---

## 📋 Versioning Strategy / バージョニング戦略

### Format / 形式

```
MAJOR.MINOR.PATCH-PRERELEASE
```

**Example / 例**: `0.47.0-alpha.1`

- **MAJOR** (0): Breaking changes / 互換性のない変更
- **MINOR** (47): New features, backward compatible / 新機能、後方互換
- **PATCH** (0): Bug fixes, backward compatible / バグ修正、後方互換
- **PRERELEASE** (alpha.1): Pre-release identifier / プレリリース識別子

---

## 🎯 Why 0.47.0-alpha.1? / なぜ 0.47.0-alpha.1?

### English

**Upstream Version**: `0.46.0-alpha.4`

**Our Changes**: Major feature additions:
- Multi-Agent Supervisor System (8 agents, 3 strategies)
- Deep Research System (3 strategies)
- Enhanced Security (5 profiles, 16 tests)
- npm Distribution (6 platforms)
- Cursor IDE Integration (MCP)

**Decision**: MINOR version bump (0.46 → 0.47)
- ✅ All changes are backward compatible
- ✅ Significant new features added
- ❌ No breaking changes to existing APIs

**Prerelease**: `-alpha.1`
- First alpha release of 0.47.0
- Ready for testing and feedback
- Not yet production-ready

### 日本語

**上流バージョン**: `0.46.0-alpha.4`

**我々の変更**: 主要機能追加:
- Multi-Agent Supervisorシステム（8エージェント、3戦略）
- Deep Researchシステム（3戦略）
- 強化されたセキュリティ（5プロファイル、16テスト）
- npm配布（6プラットフォーム）
- Cursor IDE統合（MCP）

**決定**: MINORバージョンアップ（0.46 → 0.47）
- ✅ すべての変更は後方互換性あり
- ✅ 重要な新機能を追加
- ❌ 既存APIへの破壊的変更なし

**プレリリース**: `-alpha.1`
- 0.47.0の最初のアルファリリース
- テストとフィードバック用
- まだプロダクション向けではない

---

## 📊 Version History / バージョン履歴

| Version | Date | Type | Description |
|---------|------|------|-------------|
| 0.47.0-alpha.1 | 2025-10-08 | MINOR | Multi-Agent, Deep Research, Security, npm, Cursor |
| 0.46.0-alpha.4 | (upstream) | - | Upstream latest |
| 0.45.0 | (upstream) | - | Previous stable |

---

## 🔄 When to Bump Versions / いつバージョンを上げるか

### MAJOR Version (1.0.0)

**English:**
- Breaking API changes
- Removing deprecated features
- Significant architectural changes

**Examples:**
- Changing function signatures
- Removing public APIs
- Incompatible data format changes

**日本語:**
- APIの破壊的変更
- 非推奨機能の削除
- 大幅なアーキテクチャ変更

**例:**
- 関数シグネチャの変更
- パブリックAPIの削除
- 互換性のないデータフォーマット変更

### MINOR Version (0.X.0)

**English:**
- New features, backward compatible
- New APIs or tools
- Performance improvements
- New optional parameters

**Examples:**
- Adding new agent types
- New execution strategies
- Additional security profiles

**日本語:**
- 新機能、後方互換性あり
- 新しいAPIまたはツール
- パフォーマンス改善
- 新しいオプションパラメータ

**例:**
- 新しいエージェントタイプの追加
- 新しい実行戦略
- 追加のセキュリティプロファイル

### PATCH Version (0.0.X)

**English:**
- Bug fixes
- Documentation updates
- Internal refactoring
- No API changes

**Examples:**
- Fixing crashes
- Correcting error messages
- Performance bug fixes

**日本語:**
- バグ修正
- ドキュメント更新
- 内部リファクタリング
- API変更なし

**例:**
- クラッシュ修正
- エラーメッセージの訂正
- パフォーマンスバグ修正

---

## 🚀 Release Process / リリースプロセス

### For this PR / このPR用

1. **Update Version**:
   ```powershell
   .\update-version.ps1
   ```

2. **Review Changes**:
   ```powershell
   git diff
   ```

3. **Commit**:
   ```powershell
   git add -A
   git commit -m "chore: bump version to 0.47.0-alpha.1"
   ```

4. **Include in PR**:
   - Version bump commit included in PR
   - CHANGELOG.md updated
   - VERSION file created

### Future Releases / 今後のリリース

**Alpha → Beta → RC → Stable**

```
0.47.0-alpha.1  (current)
0.47.0-alpha.2  (bug fixes)
0.47.0-beta.1   (feature complete)
0.47.0-rc.1     (release candidate)
0.47.0          (stable)
```

---

## 📝 Version Files / バージョンファイル

### Files Updated / 更新されるファイル

1. **`codex-rs/Cargo.toml`**: Workspace version
   ```toml
   [workspace]
   version = "0.47.0"
   ```

2. **`codex-cli/package.json`**: npm version
   ```json
   {
     "version": "0.47.0"
   }
   ```

3. **`VERSION`**: Version tracking
   ```
   0.47.0-alpha.1
   ```

4. **`CHANGELOG.md`**: Change history
   ```markdown
   ## [0.47.0-alpha.1] - 2025-10-08
   ### Added
   - Multi-Agent Supervisor System
   ...
   ```

5. **Individual Crate `Cargo.toml`**: Module versions
   ```toml
   [package]
   version = "0.47.0"
   ```

---

## 🔍 Version Compatibility / バージョン互換性

### Backward Compatibility / 後方互換性

**✅ Guaranteed / 保証**:
- All existing APIs work without changes
- Existing configurations remain valid
- No breaking changes to data formats

**⚠️ New Features Optional / 新機能はオプション**:
- Multi-Agent features opt-in
- Deep Research opt-in
- Security profiles configurable

### Upgrade Path / アップグレードパス

**From 0.46.x → 0.47.0**:
1. No code changes required
2. New features available immediately
3. Optional: Configure new security profiles
4. Optional: Enable Cursor integration

**From 0.45.x → 0.47.0**:
1. Review 0.46.x changelog
2. Test with new security profiles
3. No breaking changes expected

---

## 📖 Semantic Versioning Rules / セマンティックバージョニングルール

### English

Following [SemVer 2.0.0](https://semver.org/):

1. **Version format**: `MAJOR.MINOR.PATCH-PRERELEASE+BUILD`
2. **MAJOR = 0**: Initial development (pre-1.0)
3. **Public API changes**: Only on MAJOR or MINOR
4. **Bug fixes**: PATCH version only
5. **Pre-release**: Append `-alpha`, `-beta`, `-rc`
6. **Build metadata**: Append `+build.123`

### 日本語

[SemVer 2.0.0](https://semver.org/lang/ja/)に準拠：

1. **バージョン形式**: `MAJOR.MINOR.PATCH-PRERELEASE+BUILD`
2. **MAJOR = 0**: 初期開発（1.0以前）
3. **パブリックAPI変更**: MAJORまたはMINORのみ
4. **バグ修正**: PATCHバージョンのみ
5. **プレリリース**: `-alpha`、`-beta`、`-rc`を追加
6. **ビルドメタデータ**: `+build.123`を追加

---

## 🎯 Next Steps / 次のステップ

### Immediate / 即時

1. **Run version update script**:
   ```powershell
   .\update-version.ps1
   ```

2. **Commit version changes**:
   ```powershell
   git add -A
   git commit -m "chore: bump version to 0.47.0-alpha.1"
   ```

3. **Include in PR**

### After PR Merge / PRマージ後

1. **Tag release**:
   ```bash
   git tag rust-v0.47.0-alpha.1
   git push origin rust-v0.47.0-alpha.1
   ```

2. **Create GitHub Release**:
   - Title: `v0.47.0-alpha.1: Multi-Agent & Deep Research`
   - Body: Copy from CHANGELOG.md

3. **Publish npm package**:
   ```bash
   cd codex-cli
   npm publish --tag alpha
   ```

---

## 📞 Questions? / 質問?

**Version-related questions:**
- Why alpha instead of beta? → Still in active development, feedback needed
- When will 0.47.0 stable release? → After alpha/beta testing, RC approval
- Is 0.47.0 compatible with 0.46.x? → Yes, fully backward compatible

**バージョン関連の質問:**
- なぜalphaでbetaではない? → まだ活発に開発中、フィードバック必要
- 0.47.0安定版はいつリリース? → alpha/betaテスト、RC承認後
- 0.47.0は0.46.xと互換性がある? → はい、完全な後方互換性

---

**Current Version / 現在のバージョン**: `0.47.0-alpha.1`  
**Status / ステータス**: Ready for testing / テスト準備完了  
**Next Release / 次のリリース**: `0.47.0-alpha.2` (bug fixes) または `0.47.0-beta.1` (feature complete)

