#!/usr/bin/env python3
"""git_visualizer.rs の文字列エラー修正"""

from pathlib import Path

file = Path(r"C:\Users\downl\.cursor\worktrees\codex\tBA5Q\codex-rs\tui\src\git_visualizer.rs")

print(f"📝 修正: {file.name}")

lines = file.read_text(encoding='utf-8').split('\n')

# 307行目を完全に書き直し（0-indexed なので306）
if len(lines) > 306:
    # 新しい行に置き換え
    lines[306] = '            "Commits: {} | CUDA: {} | FPS: {:.1} | Camera: ({:.1}, {:.1}, {:.1}) | Rotation: {:.2}",'
    
    # ファイルに書き戻し
    file.write_text('\n'.join(lines), encoding='utf-8')
    print(f"✓ 307行目を書き直しました")
else:
    print(f"✗ ファイルが短すぎます ({len(lines)} 行)")




