# Continuous Code Review với Local AI

Codex CLI hiện hỗ trợ continuous code review sử dụng local AI models để review và cải thiện code liên tục.

## Tổng Quan

Tính năng Continuous Code Review cho phép bạn:

- 🔍 **Tự động phân tích code** khi bạn lưu file
- 🤖 **Sử dụng AI models local** (Ollama, LM Studio, v.v.)
- 💡 **Nhận suggestions real-time** để cải thiện code
- 📊 **Theo dõi metrics** và tiến độ qua các sessions
- 🔒 **100% Privacy** - Mọi thứ chạy local, không gửi code lên cloud

## Quick Start

### 1. Cài đặt Ollama và Models

```bash
# Cài đặt Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Pull recommended models
ollama pull deepseek-coder:33b    # Model chính cho coding
ollama pull codellama:13b-instruct # Model cho review
ollama pull qwen2.5-coder:7b       # Model nhanh cho quick check
```

### 2. Khởi động Continuous Review

```bash
# Review thư mục hiện tại
codex review start

# Review thư mục cụ thể
codex review start --dir ./src

# Chạy trong 8 giờ
codex review start --duration 8

# Chạy vô thời hạn
codex review start --indefinite
```

### 3. Analyze File Cụ Thể

```bash
# Analyze một file
codex review analyze src/main.rs

# Output dạng JSON
codex review analyze src/main.rs --output json

# Sử dụng config file tùy chỉnh
codex review analyze src/main.rs --config-file my-config.toml
```

## Cấu Hình

### Tạo Configuration File

```bash
# Xem config mặc định
codex review configure --show

# Tạo file config mới
codex review configure --init
```

### Example: codex-review.toml

```toml
[continuous_review]
enabled = true

# Patterns để watch
watch_patterns = [
    "**/*.rs",
    "**/*.py",
    "**/*.js",
    "**/*.ts",
    "**/*.go"
]

# Patterns để ignore
ignore_patterns = [
    "**/target/**",
    "**/node_modules/**",
    "**/.git/**",
    "**/dist/**",
    "**/build/**",
    "**/__pycache__/**"
]

# Triggers - Khi nào review sẽ chạy
[continuous_review.triggers]
on_file_save = true           # Review khi save file
on_git_commit = true          # Review trước khi commit
periodic_check_minutes = 30   # Review định kỳ mỗi 30 phút
on_manual_request = true      # Cho phép manual trigger

# Policies - Quyết định gì được auto-apply
[continuous_review.policies]
auto_fix_style = true                # Tự động fix style issues
auto_add_tests = false               # KHÔNG tự động add tests (cần approval)
auto_refactor = false                # KHÔNG tự động refactor (cần approval)
auto_add_comments = true             # Tự động thêm comments
require_tests_pass = true            # Yêu cầu tests pass trước khi apply
create_backup_branch = true          # Tạo backup branch trước khi thay đổi
commit_each_improvement = true       # Commit từng improvement riêng
confidence_threshold = 0.85          # Chỉ apply nếu confidence >= 85%

# Local AI Configuration
[continuous_review.local_ai]
coding_model = "deepseek-coder:33b"       # Model cho coding tasks
review_model = "codellama:13b-instruct"   # Model cho review
quick_check_model = "qwen2.5-coder:7b"    # Model nhanh
ollama_base_url = "http://localhost:11434/v1"
concurrent_requests = 3                    # Số requests đồng thời
timeout_secs = 300                         # Timeout cho mỗi request

# Analysis Settings
[continuous_review.analysis]
check_style = true              # Kiểm tra code style
detect_bugs = true              # Phát hiện bugs
suggest_refactoring = true      # Suggest refactoring
check_complexity = true         # Kiểm tra complexity
generate_tests = false          # Tạo tests (experimental)
check_documentation = true      # Kiểm tra documentation
max_complexity = 10             # Ngưỡng complexity
min_doc_coverage = 0.7          # Tối thiểu 70% documentation coverage
```

## Recommended Models

### Theo Mục Đích

| Mục đích | Model | Size | RAM Cần |
|----------|-------|------|---------|
| **Code Generation** | DeepSeek-Coder V2 | 16B-236B | 32-128GB |
| **Code Review** | CodeLlama Instruct | 13B-34B | 16-64GB |
| **Quick Checks** | Qwen2.5-Coder | 7B | 8GB |
| **Lightweight** | StarCoder2 | 3B | 4GB |

### Theo Hardware

| RAM Available | Recommended Setup |
|---------------|-------------------|
| **8GB** | qwen2.5-coder:7b |
| **16GB** | codellama:13b + qwen2.5-coder:7b |
| **32GB** | deepseek-coder:33b + codellama:13b |
| **64GB+** | deepseek-coder-v2:236b (full quality) |

## Use Cases

### Use Case 1: Daily Development

```bash
# Morning: Bắt đầu review session
codex review start --duration 8

# Làm việc bình thường, mọi file save sẽ được review tự động
# Style issues sẽ được auto-fix
# Bugs và suggestions sẽ được highlight

# Evening: Xem statistics
codex review stats
```

### Use Case 2: Pre-Commit Review

```toml
# .git/hooks/pre-commit
#!/bin/bash
codex review analyze $(git diff --cached --name-only --diff-filter=ACM)
```

### Use Case 3: Large Codebase Audit

```bash
# Review toàn bộ src directory
codex review start --dir ./src --duration 24

# Check progress
codex review stats

# View sessions
codex review sessions --verbose
```

### Use Case 4: CI/CD Integration

```yaml
# .github/workflows/code-review.yml
name: Continuous Code Review

on: [push, pull_request]

jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2

      - name: Install Ollama
        run: curl -fsSL https://ollama.com/install.sh | sh

      - name: Pull Models
        run: ollama pull qwen2.5-coder:7b

      - name: Run Code Review
        run: |
          cargo install codex-cli
          codex review analyze src/ --output json > review-results.json

      - name: Upload Results
        uses: actions/upload-artifact@v2
        with:
          name: code-review-results
          path: review-results.json
```

## Session Management

### View Sessions

```bash
# List all sessions
codex review sessions

# Output:
# 📋 Review Sessions
#    Session directory: ~/.codex/review-sessions
#    Found 3 session(s):
#    • review-1699123456
#    • review-1699209856
#    • review-1699296256

# Detailed view
codex review sessions --verbose

# Output:
#    • review-1699123456
#      Started: 2024-11-05 09:00:00 UTC
#      State: Completed
#      Files analyzed: 145
#      Issues found: 23
#      Improvements: 12
```

### Statistics

```bash
codex review stats

# Output:
# 📊 Review Statistics
#    Session: review-1699123456
#
#    Files Analyzed: 145
#    Total Issues: 23
#      Critical: 0
#      Errors: 3
#      Warnings: 15
#      Info: 5
#
#    Suggestions: 45
#      Applied: 12
#      Pending: 33
#
#    Improvements:
#      Style fixes: 8
#      Refactorings: 2
#      Comments added: 2
```

## Advanced Features

### Custom Analysis Rules

Bạn có thể extend analyzer với custom rules:

```rust
// custom-analyzer/src/lib.rs
use codex_code_review::{CodeAnalyzer, Issue};

pub fn check_custom_patterns(content: &str) -> Vec<Issue> {
    // Your custom analysis logic
}
```

### Integration với Linters

```toml
[continuous_review.integrations]
clippy = true
eslint = true
pylint = true

[continuous_review.integrations.clippy]
flags = ["-W", "clippy::all"]

[continuous_review.integrations.eslint]
config = ".eslintrc.json"
```

### Git Hooks Integration

```bash
# Tự động setup git hooks
codex review configure --setup-hooks

# Thủ công tạo pre-commit hook
cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
changed_files=$(git diff --cached --name-only --diff-filter=ACM | grep '\.rs$')
if [ -n "$changed_files" ]; then
    codex review analyze $changed_files
    if [ $? -ne 0 ]; then
        echo "Code review found issues. Fix them or use git commit --no-verify"
        exit 1
    fi
fi
EOF

chmod +x .git/hooks/pre-commit
```

## Troubleshooting

### Ollama Connection Issues

```bash
# Check Ollama is running
ollama list

# Restart Ollama
ollama serve

# Test connection
curl http://localhost:11434/api/tags
```

### Performance Issues

```bash
# Reduce concurrent requests
codex review start -c continuous_review.local_ai.concurrent_requests=1

# Use smaller models
codex review start -c continuous_review.local_ai.review_model="qwen2.5-coder:7b"

# Reduce watch patterns
codex review start -c 'continuous_review.watch_patterns=["**/*.rs"]'
```

### High Memory Usage

```bash
# Use quantized models
ollama pull deepseek-coder:33b-q4  # 4-bit quantized

# Reduce context window
# (Configure in Ollama modelfile)
```

## So Sánh với Claude Code

| Tính năng | Claude Code | Codex Local Review |
|-----------|-------------|-------------------|
| **Privacy** | Gửi code lên cloud | ✅ 100% local |
| **Chi phí** | $X/month | ✅ Miễn phí |
| **Tốc độ** | Phụ thuộc network | ⚡ Local inference |
| **Offline** | ❌ Cần internet | ✅ Hoạt động offline |
| **Customization** | Giới hạn | ✅ Hoàn toàn tùy chỉnh |
| **Models** | Fixed (GPT-4) | ✅ Bất kỳ model nào |
| **Context size** | API limits | ✅ Hardware limits |
| **Enterprise** | Cần license | ✅ Tự host |

## Roadmap

### Phase 1 ✅ (Completed)
- [x] Basic continuous review loop
- [x] File watching
- [x] Ollama integration
- [x] Session management
- [x] CLI commands

### Phase 2 🚧 (In Progress)
- [ ] Linter integrations (clippy, eslint)
- [ ] Test generation
- [ ] Git hooks automation
- [ ] TUI interface
- [ ] Performance optimizations

### Phase 3 📝 (Planned)
- [ ] Web UI for session viewing
- [ ] Multi-model ensembles
- [ ] RAG for large codebases
- [ ] Incremental analysis
- [ ] Learning from feedback

## Resources

- **Ollama**: https://ollama.com/
- **Models**: https://ollama.com/library
- **Codex Documentation**: https://docs.codex.dev/
- **Issue Tracker**: https://github.com/openai/codex/issues

## Contributing

Contributions welcome! Key areas:

1. **Model Support** - Add more local model integrations
2. **Analysis Tools** - Integrate existing linters
3. **UI/UX** - Improve output formatting
4. **Performance** - Optimize for large codebases
5. **Documentation** - Add more examples

Xem `code-review/README.md` để biết chi tiết về architecture.
