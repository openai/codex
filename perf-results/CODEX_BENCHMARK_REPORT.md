# CODEX BENCHMARK REPORT: Custom-Built vs Homebrew
## minimax-m2.5 via cliproxyapi++

---

## Configuration

| Component | Version |
|-----------|---------|
| Custom-built | codex-cli 0.0.0 (compiled from source) |
| Homebrew | codex-cli 0.104.0 |
| Model | minimax-m2.5 |
| Proxy | cliproxyapi++ @ 127.0.0.1:8318 |
| Test | exec "Reply with OK." |

---

## Results

| Metric | Custom (0.0.0) | Homebrew (0.104.0) | Winner |
|--------|----------------|---------------------|--------|
| Latency Mean | 5.51s | 12.20s | CUSTOM ✓ |
| Latency Median | 4.84s | 6.24s | CUSTOM ✓ |
| Latency Best | 4.14s | 5.42s | CUSTOM ✓ |
| Latency Worst | 8.25s | 23.59s | CUSTOM ✓ |
| Latency StdDev | 1.69s | 8.80s | CUSTOM ✓ |
| Peak RSS Mean | 60.5MB | 48.7MB | HOMEBREW ✓ |
| Peak RSS Max | 60.7MB | 49.0MB | HOMEBREW ✓ |
| Open FDs (mean) | 25 | 31 | CUSTOM ✓ |
| Open FDs (max) | 41 | 32 | HOMEBREW ✓ |

---

## Analysis

### ⚡ PERFORMANCE
- Custom is **1.29x faster** (median)
- Custom is **2.21x faster** (mean)
- Custom has **5.2x lower variance** (more predictable)

### 📦 MEMORY
- Homebrew uses **24.2% less memory**
- Difference: ~11.8MB

### 📁 FILE DESCRIPTORS
- Custom: ~25 FDs average
- Homebrew: ~31 FDs average

---

## Conclusion

| Use Case | Recommendation |
|----------|----------------|
| Performance-critical workloads | **Custom-built** |
| Memory-constrained environments | **Homebrew** |

**Custom-built codex is FASTER and more CONSISTENT**
**Homebrew uses LESS MEMORY**

---

*Benchmark run: 2026-02-23*
