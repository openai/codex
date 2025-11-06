#!/usr/bin/env python3
"""Fix merge conflict in git_visualizer.rs"""

with open('codex-rs/tui/src/git_visualizer.rs', 'r', encoding='utf-8', errors='ignore') as f:
    lines = f.readlines()

output = []
in_conflict = False
skip_until_equal = False

for line in lines:
    if '<<<<<<< HEAD' in line:
        in_conflict = True
        skip_until_equal = True
        continue
    elif '=======' in line and in_conflict:
        skip_until_equal = False
        continue
    elif '>>>>>>> origin/main' in line and in_conflict:
        in_conflict = False
        continue
    
    if not skip_until_equal:
        # Fix encoding issue
        line = line.replace('\uff82\uff70', 'deg')
        line = line.replace('ﾂｰ', 'deg')
        output.append(line)

with open('codex-rs/tui/src/git_visualizer.rs', 'w', encoding='utf-8') as f:
    f.writelines(output)

print("✅ Conflict resolved successfully")

