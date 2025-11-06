#!/usr/bin/env python3
"""GUI blueprint残り修正"""
from pathlib import Path

base = Path(r"C:\Users\downl\.cursor\worktrees\codex\tBA5Q")
files = [
    base / "codex-rs/tauri-gui/src/pages/Settings.tsx",
    base / "codex-rs/tauri-gui/src/pages/Dashboard.tsx"
]

for f in files:
    if f.exists():
        c = f.read_text(encoding='utf-8')
        orig = c
        c = c.replace('Blueprints', 'Plans')
        c = c.replace('/blueprints', '/plans')
        c = c.replace('blueprint', 'plan')
        if c != orig:
            f.write_text(c, encoding='utf-8')
            print(f"✓ {f.name}")

print("GUI修正完了")

