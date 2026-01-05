## ワーカー起動例（shard指定、JSONのみ回収）

```bash
OUT_JSON="$(mktemp)"
# スキルの配置場所に応じて調整:
# - repo-local:   ./.codex/skills/history-search/...
# - CODEX_HOME:   ~/.codex/skills/history-search/...
SCHEMA_PATH="./.codex/skills/history-search/references/WORKER_OUTPUT_SCHEMA.json"

cat <<'PROMPT' | codex --ask-for-approval never exec \
  --cd . \
  --sandbox read-only \
  --output-schema "$SCHEMA_PATH" \
  --output-last-message "$OUT_JSON" \
  -
$history-search

query:
対象プロジェクトの運用（ビルド/テスト/リリース/開発手順/規約）について、再利用できる知識候補を抽出して。

scope (JSON):
{"shard_id":"2025-12-15..2025-12-21","since":"2025-12-15","until":"2025-12-21","recent_files":200,"include_archived":true,"project_root":".","kinds":["command","file","error","message","tool"],"max_hits":60,"expand_window":2}
PROMPT

cat "$OUT_JSON"
```
