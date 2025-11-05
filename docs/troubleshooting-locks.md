# Repository Lock Troubleshooting Guide

## Overview

Codex uses a repository-level lock mechanism (`.codex/lock.json`) to serialize write operations and prevent data races when multiple Codex instances or agents are working in the same repository.

## Lock File Format

The lock file contains metadata about the current lock holder:

```json
{
  "version": "1.0",
  "pid": 12345,
  "ppid": 12344,
  "uid": 1000,
  "hostname": "mycomputer",
  "repo_path": "/path/to/repository",
  "started_at": 1698765432,
  "expires_at": null
}
```

## Common Issues

### 1. "Lock is held by PID X" Error

**Cause**: Another Codex process or agent is currently holding the lock.

**Solutions**:
- Wait for the other process to complete
- Check if the process is still running: `ps aux | grep <PID>`
- If the process has crashed, remove the stale lock

### 2. Stale Locks

**Cause**: A Codex process crashed or was forcibly terminated without releasing the lock.

**Detection**: Run `codex lock status` to see if the lock holder process is still alive.

**Solution**:
```bash
# Check lock status
codex lock status

# Force remove stale lock
codex lock remove --force
```

### 3. Permission Denied

**Cause**: Insufficient permissions to read/write `.codex/lock.json`.

**Solution**:
```bash
# Check permissions
ls -la .codex/lock.json

# Fix permissions (Unix/Linux/macOS)
chmod 600 .codex/lock.json

# Ensure .codex directory is accessible
chmod 700 .codex
```

## Lock Management Commands

### Check Lock Status

```bash
codex lock status
```

Shows:
- Whether a lock is currently held
- Lock holder PID, hostname, and other metadata
- Whether the lock appears stale

### Release Lock

```bash
# Release lock (only works if current process owns it)
codex lock remove

# Force remove lock (use with caution)
codex lock remove --force
```

## Best Practices

1. **Don't manually delete lock files** unless you're certain no Codex processes are running.

2. **Use `codex lock status`** before forcing lock removal to verify the lock is truly stale.

3. **Check running processes** before removing locks:
   ```bash
   # On Unix/Linux/macOS
   ps aux | grep codex
   
   # On Windows
   tasklist | findstr codex
   ```

4. **Avoid running multiple Codex instances** in the same repository simultaneously unless using the orchestrator.

## Lock Lifecycle

1. **Acquisition**: When a Codex instance starts a write operation, it attempts to acquire the lock
2. **Active**: Lock is held while write operations are in progress
3. **Release**: Lock is released automatically when operations complete
4. **Stale Detection**: If a process crashes, the lock becomes stale and can be cleaned up

## Advanced: TTL-based Locks

Some operations support time-to-live (TTL) locks that automatically expire:

```rust
// In code, not user-facing
lock.acquire(Some(300)); // 5-minute TTL
```

Expired locks are automatically cleaned up on the next acquisition attempt.

## Debugging

Enable debug logging to see lock operations:

```bash
RUST_LOG=codex_core::lock=debug codex <command>
```

This will show:
- Lock acquisition attempts
- Stale lock detection
- Lock release operations

## Integration with Orchestrator

When using the orchestrator (planned feature), write operations are automatically serialized through a single-writer queue, reducing lock contention.

The orchestrator:
- Manages lock acquisition/release automatically
- Queues concurrent write requests
- Returns 429 (backpressure) when queue is full
- Implements idempotency for safe retries

---

**日本語版 / Japanese Version**

## 概要

Codexはリポジトリレベルのロック機構（`.codex/lock.json`）を使用して、書き込み操作を直列化し、同じリポジトリで複数のCodexインスタンスやエージェントが動作する際のデータ競合を防ぎます。

## よくある問題

### 1. "Lock is held by PID X" エラー

**原因**: 別のCodexプロセスまたはエージェントが現在ロックを保持している。

**解決方法**:
- 他のプロセスが完了するのを待つ
- プロセスがまだ実行中か確認: `ps aux | grep <PID>`
- プロセスがクラッシュしている場合、古いロックを削除

### 2. 古いロック（Stale Lock）

**原因**: Codexプロセスがクラッシュしたか、強制終了されてロックを解放できなかった。

**検出**: `codex lock status` を実行して、ロック保持者のプロセスがまだ生きているか確認。

**解決方法**:
```bash
# ロック状態を確認
codex lock status

# 古いロックを強制削除
codex lock remove --force
```

## ロック管理コマンド

### ロック状態の確認

```bash
codex lock status
```

### ロックの解放

```bash
# ロックを解放（現在のプロセスが所有している場合のみ）
codex lock remove

# ロックを強制削除（注意して使用）
codex lock remove --force
```

## ベストプラクティス

1. **手動でロックファイルを削除しない** - Codexプロセスが実行されていないことを確認してから
2. **`codex lock status` を使用** - 強制削除前にロックが本当に古いことを確認
3. **同じリポジトリで複数のCodexインスタンスを同時に実行しない** - オーケストレータを使用する場合を除く
