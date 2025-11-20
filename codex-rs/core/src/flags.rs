// 该文件定义核心模块使用的环境开关标志。
use env_flags::env_flags;

env_flags! {
    /// Fixture path for offline tests (see client.rs).
    /// 离线测试使用的夹具路径（见 client.rs）
    pub CODEX_RS_SSE_FIXTURE: Option<&str> = None;
}
