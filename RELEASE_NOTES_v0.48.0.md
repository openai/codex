# Codex v0.48.0 - ThreeWayMerge & Enhanced Sub-Agent System

🎉 **Release Date**: October 16, 2025  
📦 **Binary Size**: 39.34 MB  
🔖 **Tag**: `v0.48.0`

---

## 🚀 What's New

### 1. ThreeWayMerge Implementation 🔀

Implemented Git-style 3-way merge algorithm for advanced conflict resolution.

**Features**:

- Automatic merge for non-conflicting changes
- Conflict marker generation (`<<<<<<<`, `=======`, `>>>>>>>`)
- Line-by-line merge logic using `similar` crate
- Comprehensive test coverage

**Usage**:

```rust
use codex_core::orchestration::conflict_resolver::resolve_three_way;

let result = resolve_three_way(base, ours, theirs);
if result.has_conflicts {
    println!("Conflicts detected:\n{}", result.merged);
} else {
    println!("Auto-merged successfully!");
}
```

**Implementation**: `codex-rs/core/src/orchestration/conflict_resolver.rs`

---

### 2. Natural Language Agent Invocation 🤖

Invoke agents using natural language commands.

```bash
# Security-focused code review
codex agent "Review this code for security vulnerabilities"

# Test generation
codex agent "Generate comprehensive tests for the auth module"

# Refactoring suggestions
codex agent "Suggest refactoring opportunities in this codebase"
```

**Benefits**:

- Intuitive task description
- Context-aware agent selection
- Flexible workflow integration

---

### 3. Enhanced Sub-Agent System 🎯

**Available Agents**:

- `code-reviewer`: Code quality, best practices, security
- `sec-audit`: Security vulnerability scanning
- `test-gen`: Automated test generation
- `researcher`: Deep research with citations

**Quick Commands**:

```bash
codex review          # Quick code review
codex audit           # Security audit
codex test            # Test generation
```

**Parallel Execution**:

```bash
codex delegate-parallel code-reviewer,test-gen \
  --scopes ./src,./tests
```

---

### 4. Agent Creation API 🛠️

Create custom agents on-the-fly:

```bash
codex agent-create "Find all TODO comments and create a summary"
```

**Features**:

- Dynamic agent configuration
- Task-specific behavior
- Reusable agent definitions

---

## 🔧 Improvements

### Build & Performance

- ✅ Clean release build (16m 29s)
- ✅ Optimized binary (39.34 MB)
- ✅ LTO enabled for performance
- ✅ 710 crates updated to latest compatible versions

### Stability

- ✅ Fixed `just` Cargo.toml conflict
- ✅ Cargo cache cleanup for reliable builds
- ✅ 8/8 integration tests passing

### Developer Experience

- ✅ Comprehensive test suite (`test-codex-v048.ps1`)
- ✅ Improved error messages
- ✅ Better CLI help documentation

---

## 📦 Installation

### From Cargo

```bash
cargo install --git https://github.com/zapabob/codex-main --tag v0.48.0 codex-cli
```

### From Binary (Windows)

1. Download `codex.exe` from [Releases](https://github.com/zapabob/codex-main/releases/tag/v0.48.0)
2. Place in `C:\Users\<USERNAME>\.cargo\bin\`
3. Verify: `codex --version` → `codex-cli 0.48.0`

### From Source

```bash
git clone https://github.com/zapabob/codex-main
cd codex-main/codex-rs
cargo build --release -p codex-cli
cargo install --path cli --force
```

---

## 🧪 Verification

Run the test suite to verify installation:

```bash
# Download test script
curl -O https://raw.githubusercontent.com/zapabob/codex-main/v0.48.0/test-codex-v048.ps1

# Run tests
powershell -ExecutionPolicy Bypass -File test-codex-v048.ps1
```

**Expected Output**:

```
Codex v0.48.0 Real Device Test
================================
Test Summary
  PASS: 8 / 8
  FAIL: 0 / 8

All tests passed!
```

---

## 📚 Documentation

- **Implementation Log**: [`_docs/2025-10-16_ThreeWayMerge_and_v048_Release.md`](_docs/2025-10-16_ThreeWayMerge_and_v048_Release.md)
- **Project Rules**: [`.cursorrules`](.cursorrules)
- **README**: [`README.md`](README.md)

---

## 🔄 Migration Guide

### From v0.47.0-alpha.1 to v0.48.0

**Breaking Changes**: None

**New Commands**:

```bash
# Old (still works)
codex delegate code-reviewer --scope ./src

# New (natural language)
codex agent "Review code in src/ for security issues"

# New (parallel)
codex delegate-parallel code-reviewer,test-gen --scopes ./src,./tests
```

**Config Updates**:
No configuration changes required. All existing config files are compatible.

---

## 🐛 Bug Fixes

- Fixed `just` Cargo.toml conflict causing build failures
- Resolved PowerShell output encoding issues in test scripts
- Improved error handling for missing dependencies

---

## 🙏 Acknowledgments

This release is based on [OpenAI/codex](https://github.com/openai/codex) with additional features and improvements.

Special thanks to:

- OpenAI team for the original Codex project
- Rust community for excellent tooling
- Contributors to the `similar` crate

---

## 📊 Technical Details

### Dependencies

- Rust: 1.80.0+
- similar: 2.7.0 (3-way merge)
- clap: 4.5.49 (CLI parsing)
- tokio: 1.48.0 (async runtime)

### Build Configuration

```toml
[profile.release]
lto = true
codegen-units = 1
```

### Tested Platforms

- ✅ Windows 11 (x86_64)
- ⏳ Linux (pending)
- ⏳ macOS (pending)

---

## 🔮 Roadmap

### v0.49.0 (Planned)

- [ ] LLM Intent Classifier
- [ ] Enhanced webhook integration
- [ ] Multi-platform binary releases
- [ ] Performance optimizations

### v0.50.0 (Future)

- [ ] GUI interface
- [ ] Plugin system
- [ ] Cloud integration
- [ ] Team collaboration features

---

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/zapabob/codex-main/issues)
- **Discussions**: [GitHub Discussions](https://github.com/zapabob/codex-main/discussions)
- **Documentation**: [Project README](README.md)

---

## 📄 License

This project maintains the same license as the original OpenAI Codex project.

---

**Full Changelog**: [`v0.47.0-alpha.1...v0.48.0`](https://github.com/zapabob/codex-main/compare/v0.47.0-alpha.1...v0.48.0)

🎉 **Enjoy Codex v0.48.0!**
