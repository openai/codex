# 常见问题（FAQ）

> 本文是 `docs/faq.md` 的中文概览版本，覆盖部分常见问题。更完整的内容以英文原文为准。

## 安装相关

**Q：如何安装 Codex？**  
A：最简单的方式是：

- 使用 npm：

```bash
npm install -g @openai/codex
codex
```

- 或使用 Homebrew（macOS）：

```bash
brew install --cask codex
```

更多系统要求和构建方式见 `docs/install.md`。

## 升级相关

**Q：通过 Homebrew 安装后升级有问题？**  
A：请参考 `docs/faq.md` 中的 Homebrew 专门条目。一般建议：

- 使用 `brew upgrade codex` 升级。
- 遇到缓存或版本混乱时，根据 FAQ 中说明清理后重试。

## 账号与计费

**Q：Codex 使用的是哪个账户/套餐？**  
A：如果使用 ChatGPT 登录，Codex 会使用你当前的 ChatGPT 套餐（例如 Plus、Pro、Team 等）对应的能力。  
如果使用 API Key，则按照对应 provider 的计费策略计费。

## sandbox 与安全

**Q：Codex 会随便在我电脑上执行命令吗？**  
A：不会。Codex：

- 默认为网络禁用模式，并在受控目录下运行。
- 支持多种 sandbox 策略（read-only / workspace-write / danger-full-access）。
- 可以配置审批策略，在执行文件写入或命令前询问你的确认。

详情见 `docs/sandbox.md` 和 `docs/platform-sandboxing.md`。

## 调试与日志

**Q：如何查看更详细的调试信息？**  
A：设置 `RUST_LOG` 环境变量，例如：

```bash
RUST_LOG=info codex
RUST_LOG=trace codex exec "说明这个仓库"
```

更多高级调试选项见 `docs/advanced.md`。

