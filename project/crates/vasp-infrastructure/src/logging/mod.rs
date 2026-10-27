use tracing_subscriber::{fmt, EnvFilter};

/// 初始化日志：写 stderr，绝不污染 stdout。
///
/// **stdout 是 JSON 通道**——任何调试输出混进去，Electron 侧的解析就断了。
pub fn init(verbose: bool) {
    let filter = if verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))
    };
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
