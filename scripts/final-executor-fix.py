#!/usr/bin/env python3
"""executor.rs 最終修正"""

import re
from pathlib import Path

file = Path(r"C:\Users\downl\.cursor\worktrees\codex\tBA5Q\codex-rs\core\src\plan\executor.rs")

content = file.read_text(encoding='utf-8')

# 188行目付近の Plan.state を plan.state に
content = re.sub(r'(\s+)Plan\.state = Plan', r'\1plan.state = plan', content)

# 他の全ての Plan. を plan. に（ただしPlanBlock等の型名は除く）
content = re.sub(r'(?<!pub struct )(?<!enum )(?<!impl )\bPlan\.', 'plan.', content)

# メッセージ内の "Plan " を "plan " に
content = re.sub(r'"Plan \{', '"plan {', content)

file.write_text(content, encoding='utf-8')
print("✓ executor.rs 修正完了")




