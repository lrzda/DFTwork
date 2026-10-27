use crate::AppError;
use std::path::Path;

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

    fn exists(&self, path: &Path) -> bool;

    /// 是否是目录。
    ///
    /// 调用方经常要处理「给文件也行、给目录也行」的入参（例如 `scene` 命令），
    /// 缺了它就只能靠 `list_dir` 是否报错来猜，那是拿异常当控制流。
    fn is_dir(&self, path: &Path) -> bool;
}

/// 单个文件读取上限：64 MB。超过时调用方应改用 [`Fs::read_tail`]。
pub const MAX_FULL_READ: u64 = 64 * 1024 * 1024;

/// 状态判定的取样长度：OUTCAR / OSZICAR 只读尾部即可判断有没有跑完。
pub const TAIL_PROBE_BYTES: u64 = 64 * 1024;
