use crate::AppError;
use std::path::Path;
use vasp_domain::AnalysisReport;

/// 目录项。只保留判定需要的信息，不返回平台相关的元数据结构。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntryInfo {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// 应用层需要的文件能力。
///
/// **为什么这个 trait 在 application 而不是 domain**：领域层完全不需要文件，
/// 而「读 VASP 目录里的文件」也不存在第二种实现，所以它不是业务端口；
/// 但 application 又不能依赖 `vasp-infrastructure`（依赖方向会反），
/// 因此在这里声明接口、由基础设施实现、组合根注入。
pub trait Fs: Send + Sync {
    fn list_dir(&self, dir: &Path) -> Result<Vec<DirEntryInfo>, AppError>;

    /// 读全文，超过 `max_bytes` 视为错误（避免把 GB 级文件读进内存）。
    fn read_text(&self, path: &Path, max_bytes: u64) -> Result<String, AppError>;

    /// 读尾部，用于 OUTCAR 这种几十 MB 的文件。
    fn read_tail(&self, path: &Path, max_bytes: u64) -> Result<String, AppError>;

    /// 按行流式扫描，用于几百 MB 的 OUTCAR / OSZICAR。
    ///
    /// `visit` 返回 `false` 表示提前结束（找到目标块就不用读到底）。
    /// 返回实际读过的行数，便于诊断「到底扫了多少」。
    ///
    /// 之所以要有这个方法：`某个大体系` 的 OUTCAR 有 几百 MB / 数百万行，
    /// `某 MD 轨迹` 有 几百 MB。这类文件既不能整读进内存（[`Read_text`] 的 64 MB 上限），
    /// 也不能只读尾部（频率块、磁矩块常在文件中部）。
    ///
    /// [`Read_text`]: Fs::read_text
    fn scan_lines(
        &self,
        path: &Path,
        visit: &mut dyn FnMut(&str) -> bool,
    ) -> Result<usize, AppError>;

    fn exists(&self, path: &Path) -> bool;

    /// 是否是目录。
    ///
    /// 调用方经常要处理「给文件也行、给目录也行」的入参（例如 `scene` 命令），
    /// 缺了它就只能靠 `list_dir` 是否报错来猜，那是拿异常当控制流。
    fn is_dir(&self, path: &Path) -> bool;

    /// 写文件，父目录不存在时自动创建。
    ///
    /// **只有 `ApplyPlan` 该调用它**——其他任何用例都不得写盘。
    /// 这个约束靠代码审查守，接口上没法强制（Rust 没有 effect 系统）。
    fn write_text(&self, path: &Path, content: &str) -> Result<(), AppError>;

    /// 删除文件。回滚时清理「本次计划新建的文件」。
    ///
    /// 文件本来就不存在时视为成功：回滚要幂等，重复执行不该报错。
    fn remove_file(&self, path: &Path) -> Result<(), AppError>;
}

/// 把一段提示词交给外部 AI，拿回一次分析结果。
///
/// 放在 application 而不是 infrastructure：用例要调用它，但不能依赖
/// 具体实现（`HermesRunner`）。测试里可以换成假实现，不需要真跑 CLI。
pub trait Analyzer: Send + Sync {
    fn ask(&self, prompt: &str) -> Result<AnalysisReport, AppError>;
}

/// 单个文件读取上限：64 MB。超过时调用方应改用 [`Fs::read_tail`]。
pub const MAX_FULL_READ: u64 = 64 * 1024 * 1024;

/// 状态判定的取样长度：OUTCAR / OSZICAR 只读尾部即可判断有没有跑完。
pub const TAIL_PROBE_BYTES: u64 = 64 * 1024;
