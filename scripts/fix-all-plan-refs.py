#!/usr/bin/env python3
"""
Blueprint → Plan 完全修正スクリプト
全ての残存参照を修正
"""

import re
from pathlib import Path

def fix_rust_file(content: str) -> str:
    """Rustファイルの内容を修正"""
    # 引数の Plan を plan に
    content = re.sub(r',\s*Plan\s*,', ', plan,', content)
    content = re.sub(r'\(\s*Plan\s*\)', '(plan)', content)
    
    # Plan. を plan. に（ただし PlanBlock, PlanState などの型は除外）
    content = re.sub(r'(?<!struct )(?<!enum )(?<!impl )(?<!use )\bPlan\.', 'plan.', content)
    
    # &Plan. を &plan. に
    content = re.sub(r'&Plan\.', '&plan.', content)
    
    # "Plan:" (変数宣言) を "plan:" に
    content = re.sub(r'\bPlan:', 'plan:', content)
    
    # format引数など
    content = re.sub(r'Plan\.id', 'plan.id', content)
    content = re.sub(r'Plan\.mode', 'plan.mode', content)
    content = re.sub(r'Plan\.goal', 'plan.goal', content)
    content = re.sub(r'Plan\.title', 'plan.title', content)
    content = re.sub(r'Plan\.state', 'plan.state', content)
    content = re.sub(r'Plan\.approach', 'plan.approach', content)
    content = re.sub(r'Plan\.work_items', 'plan.work_items', content)
    content = re.sub(r'Plan\.artifacts', 'plan.artifacts', content)
    content = re.sub(r'Plan\.created_by', 'plan.created_by', content)
    content = re.sub(r'Plan\.eval', 'plan.eval', content)
    
    # メッセージ内の "Plan xxx" を "plan xxx" に
    content = re.sub(r'"Executing Plan ', '"Executing plan ', content)
    content = re.sub(r'"Plan ', '"plan ', content)
    
    # let mut bp = を let mut plan = に
    content = re.sub(r'let mut bp\b', 'let mut plan', content)
    content = re.sub(r'\bbp\.', 'plan.', content)
    content = re.sub(r'\bbp\)', 'plan)', content)
    content = re.sub(r'\(&bp\)', '(&plan)', content)
    
    return content

def main():
    base = Path(r"C:\Users\downl\.cursor\worktrees\codex\tBA5Q")
    
    files = [
        base / "codex-rs/core/src/orchestration/plan_orchestrator.rs",
        base / "codex-rs/core/src/execution/engine.rs",
        base / "codex-rs/core/src/agents/competition.rs",
        base / "codex-rs/core/src/plan/manager.rs",
    ]
    
    print("🔧 Plan変数完全修正スクリプト")
    print("=" * 60)
    
    for file_path in files:
        if file_path.exists():
            content = file_path.read_text(encoding='utf-8')
            original = content
            fixed = fix_rust_file(content)
            
            if fixed != original:
                file_path.write_text(fixed, encoding='utf-8')
                changes = len([1 for a, b in zip(original.split('\n'), fixed.split('\n')) if a != b])
                print(f"✓ {file_path.relative_to(base)} ({changes} lines changed)")
            else:
                print(f"  {file_path.relative_to(base)} (no changes needed)")
        else:
            print(f"✗ Not found: {file_path}")
    
    print("=" * 60)
    print("🎉 修正完了！")

if __name__ == "__main__":
    main()

