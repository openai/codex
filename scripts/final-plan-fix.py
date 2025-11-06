#!/usr/bin/env python3
"""最終Plan変数修正スクリプト"""

import re
from pathlib import Path

def fix_all_plan_vars(file_path: Path) -> int:
    """ファイル内の全てのPlan変数をplanに修正"""
    content = file_path.read_text(encoding='utf-8')
    original = content
    
    # パラメータ名: Plan: & → plan: &
    content = re.sub(r'\bPlan:\s*&', 'plan: &', content)
    
    # 変数参照: Plan. → plan.
    content = re.sub(r'(?<!struct )(?<!enum )(?<!impl )(?<!use )(?<!pub )\bPlan\.', 'plan.', content)
    
    # 関数名: execute_Plan → execute_plan
    content = re.sub(r'\bexecute_Plan\b', 'execute_plan', content)
    
    # テスト関数: test_Plan_ → test_plan_
    content = re.sub(r'\btest_Plan_', 'test_plan_', content)
    
    # create_approved_Plan → create_approved_plan
    content = re.sub(r'\bcreate_approved_Plan\b', 'create_approved_plan', content)
    
    # let Plan = → let plan =
    content = re.sub(r'\blet Plan =', 'let plan =', content)
    
    # let mut Plan = → let mut plan =
    content = re.sub(r'\blet mut Plan =', 'let mut plan =', content)
    
    changes = sum(1 for a, b in zip(original.split('\n'), content.split('\n')) if a != b)
    
    if content != original:
        file_path.write_text(content, encoding='utf-8')
    
    return changes

def main():
    base = Path(r"C:\Users\downl\.cursor\worktrees\codex\tBA5Q\codex-rs\core\src")
    
    target_files = [
        base / "plan/executor.rs",
        base / "orchestration/plan_orchestrator.rs",
        base / "execution/engine.rs",
    ]
    
    print("🔧 最終Plan変数修正スクリプト")
    print("=" * 60)
    
    total_changes = 0
    for file_path in target_files:
        if file_path.exists():
            changes = fix_all_plan_vars(file_path)
            if changes > 0:
                print(f"✓ {file_path.relative_to(base.parent.parent)} ({changes} 行変更)")
                total_changes += changes
            else:
                print(f"  {file_path.relative_to(base.parent.parent)} (変更なし)")
        else:
            print(f"✗ Not found: {file_path}")
    
    print("=" * 60)
    print(f"🎉 合計 {total_changes} 行修正完了！")

if __name__ == "__main__":
    main()




