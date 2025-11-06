#!/usr/bin/env python3
"""Clean merge conflict markers from git_visualizer.rs"""

import re

file_path = 'codex-rs/tui/src/git_visualizer.rs'

with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
    content = f.read()

# Remove conflict markers pattern by pattern
# Pattern 1: <<<<<<< HEAD ... ======= ... >>>>>>> origin/main
pattern1 = r'<<<<<<< HEAD.*?=======\s*'
content = re.sub(pattern1, '', content, flags=re.DOTALL)

# Pattern 2: Remove >>>>>>> lines
content = re.sub(r'>>>>>>> origin/main\s*\n', '', content)

# Pattern 3: Fix encoding issue
content = content.replace('ﾂｰ', 'deg')

with open(file_path, 'w', encoding='utf-8') as f:
    f.write(content)

print("✅ Conflict markers cleaned successfully")

