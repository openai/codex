#!/bin/bash
# analyze_upstream_conflict.sh
# 获取当前分支和 upstream/main 的 diff，分析冲突风险
# 用法: ./scripts/analyze_upstream_conflict.sh

set -e

cd "$(dirname "$0")/.."

MERGE_BASE=$(git merge-base sync/upstream upstream/main 2>/dev/null || git merge-base HEAD upstream/main)
LOCAL_HEAD=$(git show-ref -s sync/upstream 2>/dev/null || echo "HEAD")

echo "========================================"
echo "Upstream Sync 冲突风险分析"
echo "========================================"
echo "Merge Base: $MERGE_BASE"
echo "Local: $LOCAL_HEAD"
echo "Upstream: upstream/main"
echo ""

# 获取 local 修改
git diff --numstat "$MERGE_BASE".."$LOCAL_HEAD" -- codex-rs/ 2>/dev/null | \
  while read add del file; do
    [ "$add" = "-" ] || [ "$del" = "-" ] || [ -z "$file" ] && continue
    echo "$((add + del)) $add $del $file"
  done | sort -rn > /tmp/local_changes.txt

# 获取 upstream 修改
git diff --numstat "$MERGE_BASE"..upstream/main -- codex-rs/ 2>/dev/null | \
  while read add del file; do
    [ "$add" = "-" ] || [ "$del" = "-" ] || [ -z "$file" ] && continue
    echo "$((add + del)) $add $del $file"
  done | sort -rn > /tmp/upstream_changes.txt

# 找交集
cut -d' ' -f4 /tmp/local_changes.txt | sort -u > /tmp/local_files.txt
cut -d' ' -f4 /tmp/upstream_changes.txt | sort -u > /tmp/upstream_files.txt
comm -12 /tmp/local_files.txt /tmp/upstream_files.txt > /tmp/conflict_files.txt

echo "## 统计"
echo "| 指标 | 数值 |"
echo "|------|------|"
echo "| Local 修改文件数 | $(wc -l < /tmp/local_changes.txt | tr -d ' ') |"
echo "| Upstream 修改文件数 | $(wc -l < /tmp/upstream_changes.txt | tr -d ' ') |"
echo "| 两边都修改文件数 | $(wc -l < /tmp/conflict_files.txt | tr -d ' ') |"
echo ""

echo "## 高风险文件 (按总修改行数排序)"
echo "| Total | Local | Upstream | EXT | 文件 |"
echo "|-------|-------|----------|-----|------|"

while read file; do
  local_total=$(grep " $file$" /tmp/local_changes.txt | cut -d' ' -f1)
  upstream_total=$(grep " $file$" /tmp/upstream_changes.txt | cut -d' ' -f1)
  total=$((${local_total:-0} + ${upstream_total:-0}))

  # 检查 EXT 状态
  dir=$(dirname "$file")
  base=$(basename "$file" .rs)
  ext_file="${dir}/${base}_ext.rs"

  if [ -f "$ext_file" ]; then
    ext="✅"
  elif [[ "$file" == *"_ext.rs" ]]; then
    ext="📦"
  elif [[ "$file" == *"Cargo"* ]]; then
    ext="🔧"
  elif [[ "$file" == *"/tests/"* ]]; then
    ext="🧪"
  else
    ext="❌"
  fi

  echo "$total ${local_total:-0} ${upstream_total:-0} $ext $file"
done < /tmp/conflict_files.txt | sort -rn | head -40 | \
while read total local upstream ext file; do
  echo "| $total | $local | $upstream | $ext | \`$file\` |"
done

echo ""
echo "图例: ✅已有EXT | 📦本身是EXT | 🔧配置文件 | 🧪测试 | ❌无EXT"

# 清理临时文件
rm -f /tmp/local_changes.txt /tmp/upstream_changes.txt /tmp/local_files.txt /tmp/upstream_files.txt /tmp/conflict_files.txt
