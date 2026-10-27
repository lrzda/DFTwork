use std::path::{Path, PathBuf};
use vasp_domain::ProjectStore;

/// 项目级状态的 JSON 存储。
///
/// 写入走「临时文件 + rename」：中途断电不会留下半个文件。
/// 首版不支持多实例并发写（用单实例锁在应用层规避）。
pub struct JsonProjectStore {
    path: PathBuf,
}

impl JsonProjectStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// `%APPDATA%/DFTwork/store.json`
    pub fn default_path() -> PathBuf {
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        base.join("DFTwork").join("store.json")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl ProjectStore for JsonProjectStore {
    fn load(&self) -> Result<String, String> {
        match std::fs::read_to_string(&self.path) {
            Ok(s) => Ok(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok("{}".to_string()),
            Err(e) => Err(format!("读取 {} 失败：{e}", self.path.display())),
        }
    }

    fn save(&self, json: &str) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录 {} 失败：{e}", parent.display()))?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| format!("写入 {} 失败：{e}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)
            .map_err(|e| format!("替换 {} 失败：{e}", self.path.display()))?;
        Ok(())
    }
}
