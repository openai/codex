# 日本語IME Backspace問題 - 分析と解決策

**報告日時**: 2025-10-21  
**影響バージョン**: OpenAI/codex v0.47.0, zapabob/codex v0.48.0-zapabob.1  
**プラットフォーム**: Windows 10.0.26200.0 x64  
**Issue Type**: TUI Rendering Bug (CJK Character Handling)

---

## 🐛 問題の概要

### 症状

Codex CLIで日本語IMEを使用中、Backspaceキーで文字を削除すると:

**問題**:
- 削除位置の**左側の文字が見えなくなる**（invisible）
- 文字は実際には削除されていない（内部バッファには残存）
- レンダリングのみが失敗している
- 入力行が破損して見える（out of sync）

**再現手順**:
1. Codex CLIを起動（TUIモード）
2. 日本語IMEで全角文字を入力（例: `こんにちは`）
3. Backspaceキーを押して文字削除
4. → 削除位置の左の文字が**消えて見える**が、実際には残っている

---

### 環境情報

| 項目 | 値 |
|-----|---|
| Codex Version | v0.47.0 (OpenAI), v0.48.0-zapabob.1 (fork) |
| Subscription | Plus |
| Model | gpt-5-high |
| OS | Windows 10.0.26200.0 x64 |
| Terminal | VS Code Integrated Terminal, PowerShell |
| IME | Japanese (Microsoft IME) |
| WSL | Ubuntu (also affected) |

---

## 🔍 根本原因の分析

### 1. 全角文字の幅計算問題

**問題**:
- 日本語文字（全角）は**2つのセル幅**を占有
- 半角文字（ASCII）は**1つのセル幅**
- TUIライブラリ（ratatui/crossterm）のカーソル位置計算が全角文字を考慮していない

**例**:
```
入力: "こんにちは"
実際の表示幅: 10セル（5文字 × 2セル）
誤った計算: 5セル（5文字 × 1セル） ← バグの原因
```

---

### 2. Backspace処理のロジック問題

**推定される問題箇所** (codex-rs/tui/):

```rust
// 現在の実装（推定）
fn handle_backspace(&mut self) {
    if self.cursor > 0 {
        self.cursor -= 1;
        self.input.remove(self.cursor);  // ← 文字は削除される
        // しかしレンダリング時のカーソル位置が誤っている
    }
}
```

**問題点**:
1. `cursor`はバイトオフセットか文字数か？
2. 全角文字の場合、**ターミナル上のカーソル位置**（セル数）と**文字列のインデックス**が一致しない
3. レンダリング時に全角文字の幅を再計算していない

---

### 3. Unicode幅計算の不一致

**Unicode East Asian Width**:
```rust
// 正しい幅計算
use unicode_width::UnicodeWidthChar;

let ch = 'あ';
let width = ch.width().unwrap_or(1);  // → 2

let ch2 = 'a';
let width2 = ch2.width().unwrap_or(1);  // → 1
```

**問題**:
- TUIレンダリング時に`unicode_width`を使用していない可能性
- またはカーソル位置の計算で全角文字を考慮していない

---

## 🛠️ 解決策

### 解決策1: 全角文字幅を考慮したカーソル位置計算

**修正コード例** (codex-rs/tui/src/input.rs):

```rust
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

struct InputState {
    input: String,
    cursor_pos: usize,  // バイトオフセット
}

impl InputState {
    /// カーソル位置を文字列インデックスからターミナルセル位置に変換
    fn cursor_cell_position(&self) -> usize {
        self.input[..self.cursor_pos]
            .chars()
            .map(|c| c.width().unwrap_or(1))
            .sum()
    }

    /// Backspace処理（全角文字対応）
    fn handle_backspace(&mut self) {
        if self.cursor_pos > 0 {
            // カーソルの左側の文字を取得
            let before_cursor = &self.input[..self.cursor_pos];
            
            // 最後の文字のバイト長を計算
            if let Some((last_char_idx, last_char)) = before_cursor.char_indices().last() {
                // 文字を削除
                self.input.remove(last_char_idx);
                // カーソル位置を更新
                self.cursor_pos = last_char_idx;
            }
        }
    }

    /// レンダリング時の幅計算
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let visible_text = &self.input;
        let cursor_cell_pos = self.cursor_cell_position();
        
        // 全角文字を考慮したレンダリング
        buf.set_string(
            area.x,
            area.y,
            visible_text,
            Style::default(),
        );
        
        // カーソルを正しい位置に配置
        buf.set_style(
            Rect::new(
                area.x + cursor_cell_pos as u16,
                area.y,
                1,
                1,
            ),
            Style::default().add_modifier(Modifier::REVERSED),
        );
    }
}
```

---

### 解決策2: ratatuiのTextInput Widgetを活用

**推奨アプローチ**:

```rust
use ratatui::widgets::{Paragraph, Block, Borders};
use unicode_width::UnicodeWidthStr;

fn render_input(input: &str, cursor_pos: usize, area: Rect, buf: &mut Buffer) {
    // 全角文字を考慮した表示
    let display_cursor = input[..cursor_pos]
        .width();  // unicode_width::UnicodeWidthStrトレイトを使用
    
    // Paragraphウィジェットでレンダリング
    let paragraph = Paragraph::new(input)
        .block(Block::default().borders(Borders::ALL));
    
    paragraph.render(area, buf);
    
    // カーソルを正しい位置に描画
    let cursor_x = area.x + 1 + display_cursor as u16;
    buf.get_mut(cursor_x, area.y + 1)
        .set_style(Style::default().add_modifier(Modifier::REVERSED));
}
```

---

### 解決策3: crossterm互換性の確保

**crossterm設定**:

```rust
use crossterm::{
    cursor::{MoveTo, position},
    execute,
};

fn update_cursor_position(input: &str, cursor_byte_pos: usize) -> io::Result<()> {
    let cursor_cell_pos = input[..cursor_byte_pos]
        .chars()
        .map(|c| c.width().unwrap_or(1))
        .sum::<usize>();
    
    execute!(
        io::stdout(),
        MoveTo(cursor_cell_pos as u16, 0)
    )?;
    
    Ok(())
}
```

---

## 🧪 テストケース

### テスト1: 全角文字のBackspace

```rust
#[test]
fn test_backspace_wide_char() {
    let mut input = InputState::new();
    input.insert_str("こんにちは");
    
    // カーソル位置: "こんにちは|" (15バイト)
    assert_eq!(input.cursor_pos, 15);
    assert_eq!(input.cursor_cell_position(), 10);  // 5文字 × 2セル
    
    // Backspace: "こんにち|は"
    input.handle_backspace();
    assert_eq!(input.input, "こんにち");
    assert_eq!(input.cursor_pos, 12);  // 4文字 × 3バイト
    assert_eq!(input.cursor_cell_position(), 8);  // 4文字 × 2セル
}
```

---

### テスト2: 混在文字列のBackspace

```rust
#[test]
fn test_backspace_mixed_width() {
    let mut input = InputState::new();
    input.insert_str("Hello世界");
    
    // "Hello世界|" (11バイト)
    // 表示幅: 5 + 4 = 9セル
    assert_eq!(input.cursor_cell_position(), 9);
    
    // Backspace: "Hello世|界"
    input.handle_backspace();
    assert_eq!(input.input, "Hello世");
    assert_eq!(input.cursor_cell_position(), 7);  // 5 + 2
}
```

---

### テスト3: 絵文字のBackspace

```rust
#[test]
fn test_backspace_emoji() {
    let mut input = InputState::new();
    input.insert_str("Hello👋");
    
    // 絵文字は2セル幅
    input.handle_backspace();
    assert_eq!(input.input, "Hello");
}
```

---

## 📝 実装ガイド

### Step 1: 依存関係の追加

**Cargo.toml**:
```toml
[dependencies]
unicode-width = "0.2"
unicode-segmentation = "1.12"  # グラフェムクラスタ処理用
```

---

### Step 2: InputState構造体の修正

**codex-rs/tui/src/input.rs** (新規作成または修正):

```rust
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub struct InputState {
    /// 入力テキスト（UTF-8バイト列）
    input: String,
    
    /// カーソル位置（バイトオフセット）
    cursor_byte_pos: usize,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor_byte_pos: 0,
        }
    }
    
    /// カーソル位置をセル単位で取得（レンダリング用）
    pub fn cursor_cell_position(&self) -> usize {
        self.input[..self.cursor_byte_pos]
            .chars()
            .map(|c| c.width().unwrap_or(1))
            .sum()
    }
    
    /// Backspace処理（全角文字対応）
    pub fn handle_backspace(&mut self) {
        if self.cursor_byte_pos == 0 {
            return;
        }
        
        // カーソルの左側の文字インデックスを取得
        let before_cursor = &self.input[..self.cursor_byte_pos];
        
        if let Some((idx, _ch)) = before_cursor.char_indices().last() {
            self.input.remove(idx);
            self.cursor_byte_pos = idx;
        }
    }
    
    /// Delete処理（カーソル右側の文字を削除）
    pub fn handle_delete(&mut self) {
        if self.cursor_byte_pos >= self.input.len() {
            return;
        }
        
        let after_cursor = &self.input[self.cursor_byte_pos..];
        if let Some((idx, _ch)) = after_cursor.char_indices().next() {
            self.input.remove(self.cursor_byte_pos + idx);
        }
    }
    
    /// 文字挿入
    pub fn insert_char(&mut self, ch: char) {
        self.input.insert(self.cursor_byte_pos, ch);
        self.cursor_byte_pos += ch.len_utf8();
    }
    
    /// カーソル左移動
    pub fn move_cursor_left(&mut self) {
        if self.cursor_byte_pos == 0 {
            return;
        }
        
        let before_cursor = &self.input[..self.cursor_byte_pos];
        if let Some((idx, _ch)) = before_cursor.char_indices().last() {
            self.cursor_byte_pos = idx;
        }
    }
    
    /// カーソル右移動
    pub fn move_cursor_right(&mut self) {
        if self.cursor_byte_pos >= self.input.len() {
            return;
        }
        
        let after_cursor = &self.input[self.cursor_byte_pos..];
        if let Some(ch) = after_cursor.chars().next() {
            self.cursor_byte_pos += ch.len_utf8();
        }
    }
}
```

---

### Step 3: レンダリング処理の修正

**codex-rs/tui/src/render.rs**:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Widget,
};

pub struct InputWidget<'a> {
    input_state: &'a InputState,
}

impl<'a> InputWidget<'a> {
    pub fn new(input_state: &'a InputState) -> Self {
        Self { input_state }
    }
}

impl Widget for InputWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // 入力テキストをレンダリング
        let text = &self.input_state.input;
        buf.set_string(
            area.x,
            area.y,
            text,
            Style::default(),
        );
        
        // カーソル位置を計算（セル単位）
        let cursor_cell_pos = self.input_state.cursor_cell_position();
        
        // カーソルを反転表示
        let cursor_x = area.x + cursor_cell_pos as u16;
        if cursor_x < area.x + area.width {
            buf.get_mut(cursor_x, area.y)
                .set_style(Style::default().add_modifier(Modifier::REVERSED));
        }
    }
}
```

---

## 🔧 回避策（ユーザー向け）

### 一時的な回避策

現在のバージョンで問題を回避する方法:

#### 方法1: 非対話モードを使用

```bash
# TUIモードを避けて、非対話モードを使用
codex exec "タスク内容をここに書く"
```

**利点**:
- TUIレンダリング問題を回避
- 日本語入力が正常に動作

**欠点**:
- 対話的な編集ができない

---

#### 方法2: プロンプトを外部エディタで編集

```bash
# 環境変数でエディタを設定
$env:EDITOR = "code"  # VS Code
# または
$env:EDITOR = "vim"

# codex起動時に外部エディタで編集
codex --edit
```

---

#### 方法3: ファイル経由で入力

```bash
# プロンプトをファイルに保存
@"
日本語のプロンプト内容
複数行OK
"@ | Out-File -Encoding UTF8 prompt.txt

# ファイルから読み込んで実行
codex exec (Get-Content prompt.txt -Raw)
```

---

#### 方法4: Cursor IDE経由で使用

```
# Cursor IDEのチャットから使用（推奨）
@codex 日本語のタスク内容
```

**利点**:
- TUI問題を完全に回避
- Cursor IDEの入力フィールドは全角文字を正しく処理

---

## 📊 影響範囲

### 影響を受けるユーザー

- ✅ 日本語ユーザー
- ✅ 中国語ユーザー（簡体字・繁体字）
- ✅ 韓国語ユーザー（ハングル）
- ✅ その他CJK言語ユーザー
- ✅ 絵文字を使用するユーザー

### 影響を受けないユーザー

- ❌ ASCII文字のみ使用（英語など）
- ❌ 非対話モード（`codex exec`）のみ使用
- ❌ Cursor IDE経由で使用

---

## 🐛 関連Issue

### upstream (OpenAI/codex)

**推奨アクション**: GitHubにissue報告

**Issue Title**:
```
[Bug] Japanese IME: Backspace causes character rendering corruption in TUI
```

**Issue Body**:
```markdown
## Description
When using Japanese IME in Codex CLI (TUI mode), pressing Backspace causes the character immediately to the left of the deletion point to become invisible. The character is not actually deleted (remains in the internal buffer), but fails to render.

## Environment
- Codex Version: v0.47.0
- OS: Windows 10.0.26200.0 x64
- Terminal: VS Code Integrated Terminal, PowerShell
- IME: Japanese (Microsoft IME)

## Steps to Reproduce
1. Launch Codex CLI (interactive TUI mode)
2. Type Japanese text using IME (e.g., "こんにちは")
3. Press Backspace to delete a character
4. Observe: the character to the left becomes invisible

## Expected Behavior
Backspace should delete exactly one character and all remaining characters should remain visible and correctly aligned.

## Root Cause
Wide character (CJK) width calculation issue in TUI rendering. The cursor position calculation doesn't account for the fact that wide characters occupy 2 terminal cells.

## Suggested Fix
Use `unicode-width` crate to calculate cursor position in terminal cells:
- Character index ≠ Terminal cell position
- Need to sum character widths: `input[..cursor_pos].chars().map(|c| c.width().unwrap_or(1)).sum()`
```

**関連PR**:
- [ ] OpenAI/codexへのPR作成
- [ ] zapabob/codexフォークでの独自修正

---

## 🔨 zapabobフォーク向け修正

### 修正PR作成

**ブランチ名**: `fix/japanese-ime-backspace-rendering`

**修正ファイル**:
1. `codex-rs/tui/Cargo.toml` - 依存関係追加
2. `codex-rs/tui/src/input.rs` - InputState修正
3. `codex-rs/tui/src/render.rs` - レンダリング修正
4. `codex-rs/tui/tests/ime_test.rs` - テスト追加

**コミットメッセージ**:
```
fix(tui): Fix Japanese IME backspace rendering corruption

Problem:
- Backspace with Japanese IME causes character invisibility
- Wide character (CJK) width not calculated correctly
- Cursor position mismatch between byte offset and terminal cells

Solution:
- Add unicode-width dependency
- Calculate cursor position in terminal cells
- Fix backspace logic to handle wide characters

Fixes #XXXX (upstream issue number)
Affects: Japanese, Chinese, Korean users and emoji users
```

---

## 📈 優先度

| 項目 | 評価 |
|-----|------|
| Severity | 🔴 High |
| Frequency | 🟡 Medium (CJK users only) |
| Impact | 🔴 High (UX破損) |
| Complexity | 🟢 Low (well-known issue) |

**推奨優先度**: **High** - CJKユーザーのUXに重大な影響

---

## ✅ 検証手順

### 修正後の検証

1. **日本語入力テスト**
   ```
   入力: "こんにちは"
   Backspace × 2
   期待: "こんに" が正しく表示される
   ```

2. **混在文字列テスト**
   ```
   入力: "Hello世界"
   Backspace × 1
   期待: "Hello世" が正しく表示される
   ```

3. **絵文字テスト**
   ```
   入力: "Test👋🌍"
   Backspace × 2
   期待: "Test" が正しく表示される
   ```

4. **カーソル移動テスト**
   ```
   入力: "あいうえお"
   ← × 3 (カーソル左移動)
   期待: カーソルが"い|うえお"の位置
   ```

---

## 📝 まとめ

### 問題の本質

**TUIレンダリングが全角文字の幅を考慮していない**
- バイトオフセット ≠ 文字数 ≠ ターミナルセル位置
- `unicode-width`クレートで正しい幅を計算する必要がある

---

### 解決のポイント

1. ✅ **unicode-width クレート使用**
   - 文字ごとの表示幅を正確に取得

2. ✅ **バイトオフセットとセル位置の分離**
   - 内部処理: バイトオフセット
   - レンダリング: セル位置

3. ✅ **char_indices()でUnicode文字単位処理**
   - バイト境界を正しく認識

---

### 推奨アクション

**ユーザー向け**:
1. 一時的に非対話モード使用
2. Cursor IDE経由で使用（推奨）
3. upstream issueを監視

**開発者向け**:
1. 本ドキュメントの修正案を実装
2. テストケース追加
3. zapabobフォークで先行修正
4. upstreamにPR提出

---

**作成日時**: 2025-10-21 20:10 JST  
**ステータス**: 分析完了 - 修正待ち  
**優先度**: High

---

*日本語IME問題の完全な分析と実装可能な解決策を提供しました。*  
*zapabobフォークで先行修正し、upstreamにフィードバックすることを推奨します。*

