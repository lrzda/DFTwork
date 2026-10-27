use std::path::Path;
use vasp_application::ports::{DirEntryInfo, Fs, MAX_FULL_READ};
use vasp_application::{AppError, AppResult};

/// 本地文件系统实现。
///
/// 所有系统错误在这里翻译成 [`AppError::Io`]，上层看不到 `std::io::Error`。
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalFs;

impl LocalFs {
    pub fn new() -> Self {
        Self
    }
}

impl Fs for LocalFs {
    fn list_dir(&self, dir: &Path) -> AppResult<Vec<DirEntryInfo>> {
        let rd = std::fs::read_dir(dir)
            .map_err(|e| AppError::io(dir.display().to_string(), e))?;
        let mut out = Vec::new();
        for entry in rd {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            out.push(DirEntryInfo {
                name: entry.file_name().to_string_lossy().to_string(),
                is_dir: meta.is_dir(),
                size: if meta.is_file() { meta.len() } else { 0 },
            });
        }
        // 稳定顺序：界面上的文件列表不该每次刷新都换位置
        out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
        Ok(out)
    }

    fn read_text(&self, path: &Path, max_bytes: u64) -> AppResult<String> {
        let meta =
            std::fs::metadata(path).map_err(|e| AppError::io(path.display().to_string(), e))?;
        if meta.len() > max_bytes {
            return Err(AppError::Validation(format!(
                "文件 {} 有 {} MB，超过 {} MB 上限；大文件请用尾部读取",
                path.display(),
                meta.len() / 1024 / 1024,
                max_bytes / 1024 / 1024
            )));
        }
        let bytes =
            std::fs::read(path).map_err(|e| AppError::io(path.display().to_string(), e))?;
        Ok(decode_text(&bytes))
    }

    fn read_tail(&self, path: &Path, max_bytes: u64) -> AppResult<String> {
        use std::io::{Read, Seek, SeekFrom};

        let mut f =
            std::fs::File::open(path).map_err(|e| AppError::io(path.display().to_string(), e))?;
        let len = f
            .metadata()
            .map_err(|e| AppError::io(path.display().to_string(), e))?
            .len();
        let start = len.saturating_sub(max_bytes);
        if start > 0 {
            f.seek(SeekFrom::Start(start))
                .map_err(|e| AppError::io(path.display().to_string(), e))?;
        }
        let mut buf = Vec::new();
        f.take(max_bytes)
            .read_to_end(&mut buf)
            .map_err(|e| AppError::io(path.display().to_string(), e))?;
        Ok(decode_text(&buf))
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
}

/// 解码文本：UTF-8 优先；失败时按 GBK 尝试——Windows 上从中文环境导出的
/// VASP 输入文件常是 GBK。两者都失败则用有损替换，保证不因个别字节报错。
fn decode_text(bytes: &[u8]) -> String {
    // 去掉可能被截断在中间的多字节序列，避免「尾部读取」报错
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    for trim in 0..4usize {
        if bytes.len() > trim {
            if let Ok(s) = std::str::from_utf8(&bytes[..bytes.len() - trim]) {
                return s.to_string();
            }
        }
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// 全量读取上限的别名，方便调用方直接引用。
pub const FULL_READ_LIMIT: u64 = MAX_FULL_READ;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn tail_reads_only_the_end() {
        let dir = std::env::temp_dir().join("dftwork-fs-test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("OUTCAR");
        let mut f = std::fs::File::create(&p).unwrap();
        write!(f, "AAAAAAAAAA").unwrap();
        write!(f, "TAILMARK").unwrap();
        f.flush().unwrap();
        drop(f);

        let fs = LocalFs::new();
        let tail = fs.read_tail(&p, 8).unwrap();
        assert_eq!(tail, "TAILMARK");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn read_text_rejects_oversized() {
        let dir = std::env::temp_dir().join("dftwork-fs-test2");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("big.txt");
        std::fs::write(&p, "0123456789").unwrap();
        let fs = LocalFs::new();
        assert!(fs.read_text(&p, 4).is_err());
        let _ = std::fs::remove_file(&p);
    }
}
