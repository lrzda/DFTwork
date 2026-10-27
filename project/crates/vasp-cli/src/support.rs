use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use vasp_domain::{Clock, Logger, ProjectStore};

/// 系统时钟。注入进来是为了让测试能固定时间。
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_iso(&self) -> String {
        OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
    }
}

/// tracing 实现的日志端口。
#[derive(Debug, Default, Clone, Copy)]
pub struct TracingLogger;

impl Logger for TracingLogger {
    fn debug(&self, msg: &str) {
        tracing::debug!("{msg}");
    }
    fn info(&self, msg: &str) {
        tracing::info!("{msg}");
    }
    fn warn(&self, msg: &str) {
        tracing::warn!("{msg}");
    }
    fn error(&self, msg: &str) {
        tracing::error!("{msg}");
    }
}

/// 把 store 读出来给人看一眼的辅助（首版 CLI `store` 子命令用）。
pub fn dump_store(store: &dyn ProjectStore) -> Result<String, String> {
    store.load()
}
