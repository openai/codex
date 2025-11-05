#!/bin/bash
# 🔧 .envファイル読み込みスクリプト
# Bash/Zsh用
# 作成日: 2025-11-02
# バージョン: v0.56.0-zapabob

# 使用方法:
#   source zapabob/scripts/load-env.sh
#   または
#   . zapabob/scripts/load-env.sh
#
# 注意: このスクリプトは source コマンドで実行してください
#       直接実行（./load-env.sh）しても環境変数は設定されません

set -e  # エラー時に即座に終了

# カラー定義
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
GRAY='\033[0;37m'
NC='\033[0m' # No Color

# ロゴ表示
show_logo() {
    echo -e "${CYAN}"
    cat << "EOF"

╔═══════════════════════════════════════════════════════════╗
║                                                           ║
║   🔧 .env環境変数読み込みスクリプト                       ║
║                                                           ║
║   バージョン: v0.56.0-zapabob                            ║
║   作成日: 2025-11-02                                      ║
║                                                           ║
╚═══════════════════════════════════════════════════════════╝

EOF
    echo -e "${NC}"
}

# .envファイルのパス
ENV_FILE="${1:-.env}"

# source で実行されているか確認
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo -e "${RED}❌ エラー: このスクリプトは source コマンドで実行してください${NC}"
    echo -e "${YELLOW}正しい使用方法:${NC}"
    echo -e "  ${CYAN}source ${BASH_SOURCE[0]}${NC}"
    echo -e "  ${CYAN}. ${BASH_SOURCE[0]}${NC}"
    exit 1
fi

show_logo

# .envファイルの存在確認
if [[ ! -f "$ENV_FILE" ]]; then
    echo -e "${RED}❌ エラー: .envファイルが見つかりません: $ENV_FILE${NC}"
    echo ""
    echo -e "${YELLOW}📝 .envファイルの作成方法:${NC}"
    echo -e "  ${GRAY}1. テンプレートをコピー:${NC}"
    echo -e "     ${CYAN}cp zapabob/templates/env.template .env${NC}"
    echo -e "  ${GRAY}2. .envファイルを編集してAPIキーを設定${NC}"
    echo -e "  ${GRAY}3. このスクリプトを再実行${NC}"
    return 1
fi

echo -e "${GRAY}📂 .envファイル: $ENV_FILE${NC}"
echo -e "${YELLOW}📌 設定モード: 現在のシェルセッション（一時的）${NC}"
echo -e "${GRAY}   ※ シェル終了時に環境変数は消去されます${NC}"
echo ""
echo -e "${CYAN}🔍 .envファイルを解析中...${NC}"
echo ""

# 環境変数をカウント
VAR_COUNT=0
SUCCESS_COUNT=0
SKIP_COUNT=0

# .envファイルを読み込み
while IFS= read -r line || [[ -n "$line" ]]; do
    # 空行とコメント行をスキップ
    if [[ -z "$line" ]] || [[ "$line" =~ ^[[:space:]]*# ]]; then
        continue
    fi
    
    # KEY=VALUE 形式をパース
    if [[ "$line" =~ ^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*)=(.*)$ ]]; then
        KEY="${BASH_REMATCH[1]}"
        VALUE="${BASH_REMATCH[2]}"
        
        # 値が空の場合はスキップ
        if [[ -z "$VALUE" ]]; then
            SKIP_COUNT=$((SKIP_COUNT + 1))
            continue
        fi
        
        # 環境変数を設定
        export "$KEY=$VALUE"
        
        VAR_COUNT=$((VAR_COUNT + 1))
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
        
        # マスク処理
        if [[ ${#VALUE} -gt 10 ]]; then
            MASKED="${VALUE:0:10}...(${#VALUE} chars)"
        else
            MASKED="***"
        fi
        
        echo -e "  ${GREEN}✅ $KEY = $MASKED${NC}"
    else
        echo -e "  ${YELLOW}⚠️  無効な形式をスキップ: $line${NC}"
    fi
done < "$ENV_FILE"

echo ""

# 結果表示
if [[ $VAR_COUNT -eq 0 ]]; then
    echo -e "${YELLOW}⚠️  警告: 有効な環境変数が見つかりませんでした${NC}"
    echo -e "${GRAY}   .envファイルにKEY=VALUE形式で記述してください${NC}"
    return 1
fi

echo -e "${GREEN}✅ 環境変数の設定が完了しました！${NC}"
echo -e "   ${GREEN}成功: $SUCCESS_COUNT 個${NC}"
if [[ $SKIP_COUNT -gt 0 ]]; then
    echo -e "   ${YELLOW}スキップ: $SKIP_COUNT 個（値が空）${NC}"
fi

# 設定確認
echo ""
echo -e "${YELLOW}📋 設定された環境変数:${NC}"
echo -e "${GRAY}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# 主要な環境変数を確認表示
IMPORTANT_VARS=(
    "CODEX_API_KEY"
    "OPENAI_API_KEY"
    "GITHUB_TOKEN"
    "GEMINI_API_KEY"
    "BRAVE_API_KEY"
    "SLACK_WEBHOOK_URL"
)

for VAR in "${IMPORTANT_VARS[@]}"; do
    if [[ -n "${!VAR}" ]]; then
        VALUE="${!VAR}"
        if [[ ${#VALUE} -gt 10 ]]; then
            MASKED="${VALUE:0:10}...(${#VALUE} chars)"
        else
            MASKED="***"
        fi
        echo -e "  ${GREEN}✅ $VAR = $MASKED${NC}"
    fi
done

echo -e "${GRAY}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# 次のステップ
echo -e "${GRAY}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${YELLOW}📌 次のステップ:${NC}"
echo -e "  ${GRAY}1. 環境変数を確認:${NC}"
echo -e "     ${CYAN}env | grep -E \"(CODEX|OPENAI|GITHUB|GEMINI|BRAVE|SLACK)\"${NC}"
echo ""
echo -e "  ${GRAY}2. Codexを起動:${NC}"
echo -e "     ${CYAN}codex exec \"echo test\"${NC}"
echo ""
echo -e "  ${YELLOW}⚠️  注意: 現在のシェルセッションのみ有効です${NC}"
echo -e "     ${GRAY}永続化する場合: ~/.bashrc または ~/.zshrc に以下を追加${NC}"
echo -e "     ${CYAN}set -a; source $(pwd)/.env; set +a${NC}"
echo -e "${GRAY}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

