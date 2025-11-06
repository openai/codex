#!/usr/bin/env python3
"""git_visualizer.rs 307行目削除"""

from pathlib import Path

file = Path(r"C:\Users\downl\.cursor\worktrees\codex\tBA5Q\codex-rs\tui\src\git_visualizer.rs")

lines = file.read_text(encoding='utf-8').splitlines(keepends=True)

print(f"📝 修正前: {len(lines)} 行")
print(f"307行目: {lines[306].strip()}")

# 307行目 (index 306) を削除
del lines[306]

file.write_text(''.join(lines), encoding='utf-8')
print(f"✓ 修正後: {len(lines)} 行")
print("✓ 307行目削除完了")




