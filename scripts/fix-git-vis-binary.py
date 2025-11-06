#!/usr/bin/env python3
"""git_visualizer.rs バイナリレベル修正"""

from pathlib import Path

file = Path(r"C:\Users\downl\.cursor\worktrees\codex\tBA5Q\codex-rs\tui\src\git_visualizer.rs")

print(f"📝 バイナリレベル修正: {file.name}")

# バイト列で読み込み
data = file.read_bytes()

# 306-316行目付近の問題のある format! を検索
problem_start = data.find(b'let status_text = format!(')

if problem_start != -1:
    # format! の終わりまでを探す
    problem_end = data.find(b');', problem_start + 100)
    
    if problem_end != -1:
        print(f"✓ 問題箇所発見: byte {problem_start} - {problem_end}")
        
        # 新しいコードブロック（完全に新規作成）
        new_code = b'''let status_text = format!(
            "Commits: {} | CUDA: {} | FPS: {:.1} | Camera: ({:.1}, {:.1}, {:.1}) | Rotation: {:.2}",
            self.commits.len(),
            cuda_status,
            fps,
            self.camera_pos.0,
            self.camera_pos.1,
            self.camera_pos.2,
            self.rotation.to_degrees()
        )'''
        
        # 置換
        new_data = data[:problem_start] + new_code + data[problem_end:]
        
        file.write_bytes(new_data)
        print(f"✓ {problem_end - problem_start} バイト置換完了")
    else:
        print("✗ format終了が見つかりません")
else:
    print("✗ 問題箇所が見つかりません")




