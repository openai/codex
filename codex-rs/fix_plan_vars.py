import re

file_path = 'cli/src/plan_commands.rs'
content = open(file_path, 'r', encoding='utf-8').read()

# 単語境界を考慮して置換
content = re.sub(r'\bPlan_dir\b', 'plan_dir', content)
content = re.sub(r'\bPlan_id\b', 'plan_id', content)

open(file_path, 'w', encoding='utf-8').write(content)

# 確認
plan_dir_count = content.count('Plan_dir')
plan_id_count = content.count('Plan_id')

print(f'✅ 置換完了')
print(f'   残存 Plan_dir: {plan_dir_count}')
print(f'   残存 Plan_id: {plan_id_count}')

