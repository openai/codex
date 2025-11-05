# Contributing to Codex

Thank you for considering contributing to Codex! This document provides guidelines and instructions for contributing.

## 📋 Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Code Standards](#code-standards)
- [Testing](#testing)
- [Documentation](#documentation)
- [Pull Request Process](#pull-request-process)

## 🤝 Code of Conduct

This project adheres to a code of conduct. By participating, you are expected to uphold this code. Please report unacceptable behavior to the maintainers.

## 🚀 Getting Started

### Prerequisites

**For Core Development (Rust)**:
- Rust 2024 edition or later
- `cargo` and `rustc`
- Linux or Windows development environment

**For Kernel Development**:
- Linux: kernel-headers, build-essential
- Windows: Windows Driver Kit (WDK), Visual Studio 2022
- Understanding of kernel programming

**For Frontend Development**:
- Node.js 18+ and npm/pnpm
- TypeScript knowledge
- React and Three.js experience

### Repository Structure

```
codex/
├── codex-rs/              # Core Rust implementation
├── kernel-extensions/     # AI-Native OS kernel modules
│   ├── linux/            # Linux kernel modules (C)
│   ├── windows/          # Windows drivers (C/C++)
│   ├── rust/             # Type-safe Rust APIs
│   ├── security/         # Security audit tools
│   ├── benchmarks/       # Performance tests
│   └── packaging/        # Distribution packages
├── extensions/           # Additional features
│   └── codex-viz-web/   # Repository visualizer
│       ├── backend/     # Rust backend (axum)
│       ├── frontend/    # React + Three.js
│       └── desktop/     # Electron app
├── docs/                # Documentation
├── _docs/               # Implementation logs
└── .github/             # GitHub configuration
```

## 💻 Development Workflow

### 1. Fork and Clone

```bash
git clone https://github.com/yourusername/codex.git
cd codex
```

### 2. Create a Branch

```bash
git checkout -b feature/your-feature-name
```

### 3. Make Changes

Follow the code standards below.

### 4. Test

```bash
# Rust
cargo test --all-features

# Frontend
npm test

# Kernel modules (Linux)
make -C kernel-extensions/linux
```

### 5. Commit

Use Conventional Commits:

```bash
git commit -m "feat: add new feature"
git commit -m "fix: resolve bug in scheduler"
git commit -m "docs: update README"
```

## 📐 Code Standards

### Rust

- **Format**: Use `cargo fmt`
- **Lint**: Pass `cargo clippy -- -D warnings`
- **Safety**: Minimize `unsafe` code, document when needed
- **Testing**: Add tests for all public APIs
- **Documentation**: Add doc comments for public items

```rust
/// Calculate the sum of two numbers
///
/// # Examples
///
/// ```
/// assert_eq!(add(2, 3), 5);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

### TypeScript/React

- **Format**: Use Prettier
- **Lint**: ESLint with strict rules
- **Types**: No `any` types
- **Hooks**: Follow React hooks rules
- **Components**: Functional components with TypeScript

```typescript
interface Props {
  count: number;
  onIncrement: () => void;
}

export const Counter: React.FC<Props> = ({ count, onIncrement }) => {
  return <button onClick={onIncrement}>Count: {count}</button>;
};
```

### C/C++ (Kernel)

- **Style**: Linux kernel coding style
- **Safety**: Check all pointers, bounds, return codes
- **Memory**: Free all allocations
- **Locking**: Use proper synchronization
- **Logging**: Use `pr_info`, `pr_err`, `KdPrint`

```c
static int my_function(void *data) {
    if (!data) {
        pr_err("Invalid parameter\n");
        return -EINVAL;
    }
    
    // ... implementation
    
    return 0;
}
```

## 🧪 Testing

### Unit Tests

```bash
# Rust
cargo test

# Each crate
cd kernel-extensions/rust/ai_scheduler_rs
cargo test --release
```

### Integration Tests

```bash
# Load kernel modules
sudo insmod kernel-extensions/linux/ai_scheduler/ai_scheduler.ko

# Run integration tests
python3 kernel-extensions/tools/ai_monitor.py
```

### Performance Tests

```bash
# 24-hour stress test
sudo python3 kernel-extensions/benchmarks/stress_test.py 24
```

## 📚 Documentation

- Update README.md for user-facing changes
- Add implementation logs to `_docs/` for major features
- Document kernel APIs in code comments
- Update architecture diagrams if structure changes

## 🔍 Pull Request Process

1. **Update Documentation**: Ensure docs reflect your changes
2. **Add Tests**: All new code must have tests
3. **Pass CI**: GitHub Actions must pass
4. **Security**: Run security audits for kernel code
5. **Code Review**: Address reviewer feedback
6. **Squash Commits**: Clean commit history

### PR Title Format

```
feat(core): add GPU scheduler optimization
fix(viz): resolve memory leak in heatmap
docs(kernel): update installation guide
```

### PR Description Template

```markdown
## Summary
Brief description of the changes

## Motivation
Why is this change needed?

## Changes
- List of changes

## Testing
- How was this tested?
- Test results

## Screenshots (if applicable)

## Checklist
- [ ] Tests passing
- [ ] Documentation updated
- [ ] No new warnings
- [ ] Security reviewed (for kernel code)
```

## 🔐 Security

For kernel-level changes:

- **NEVER** commit with `--no-verify`
- **ALWAYS** test in VM first
- **ALWAYS** run security audits (valgrind, KASAN)
- **DOCUMENT** security implications

## 💡 Tips

- Start with small, focused PRs
- Ask questions in issues before big changes
- Test on real hardware when possible
- Follow existing code patterns
- Keep commits atomic

## 📞 Contact

- **Issues**: GitHub Issues
- **Discussions**: GitHub Discussions
- **Email**: zapabob@example.com

## 🙏 Thank You!

Every contribution makes Codex better. We appreciate your time and effort!
