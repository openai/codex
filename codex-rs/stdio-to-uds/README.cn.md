# codex-stdio-to-uds（中文说明）

传统上，MCP server 有两种传输机制：stdio 和 HTTP。

这个 crate 帮助引入第三种：UNIX 域套接字（UDS）。它有几个优势：

- UDS 可以附着在一个长期运行的进程上，比如 HTTP 服务器；
- UDS 可以利用 UNIX 文件权限来限制访问。

因此，本 crate 提供了一个 UDS 与 stdio 之间的适配器。典型用法是先启动一个通过 `/tmp/mcp.sock` 通信的 MCP server，然后在 Codex 中这样配置：

```shell
codex --config mcp_servers.example={command="codex-stdio-to-uds",args=["/tmp/mcp.sock"]}
```

遗憾的是，尽管 Windows 在 2018 年 10 月的版本中已经引入了 UNIX 域套接字，Rust 标准库目前仍未在 Windows 平台暴露对 UDS 的支持：

<https://github.com/rust-lang/rust/issues/56533>

作为替代方案，本 crate 在 Windows 上依赖 <https://crates.io/crates/uds_windows> 来提供 UDS 支持。

