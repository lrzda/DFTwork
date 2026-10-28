//! 备份路径规则（纯函数）。
//!
//! 内容寻址：同一个文件内容不管在多少次写入里被覆盖，备份只存一份。
//! 目录名带时间戳与计划指纹短码，便于人工翻查「这次写入是什么时候、哪份计划」。

use sha2::{Digest, Sha256};

/// blob 名：内容 sha256 的前 16 位。
///
/// 16 位十六进制 = 64 bit，对单机备份量（万级文件）碰撞概率可忽略；
/// 取全 64 位会让路径难读，收益也不大。
pub fn blob_name(content: &str) -> String {
    let mut h = Sha256::new();
    h.update(content.as_bytes());
    let hex = format!("{:x}", h.finalize());
    hex[..16].to_string()
}

/// 备份目录名：`<时间戳>-<planHash 前 8 位>`。
///
/// 时间戳里的 `:` 在 Windows 路径中非法，必须去掉。这里把
/// `2026-10-28T11:30:00+08:00` 变成 `20261028T113000+0800`。
pub fn backup_dir_name(timestamp: &str, plan_hash: &str) -> String {
    let stamp: String = timestamp
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == 'T')
        .collect();
    let short: String = plan_hash.chars().take(8).collect();
    format!("{stamp}-{short}")
}

/// blob 在项目内的相对路径。
pub fn blob_rel_path(dir: &str, blob: &str) -> String {
    format!("{dir}/blobs/{blob}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_content_same_blob() {
        assert_eq!(blob_name("ENCUT = 400\n"), blob_name("ENCUT = 400\n"));
        assert_ne!(blob_name("ENCUT = 400\n"), blob_name("ENCUT = 520\n"));
    }

    #[test]
    fn blob_is_16_hex_chars() {
        let b = blob_name("x");
        assert_eq!(b.len(), 16);
        assert!(b.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn timestamp_colons_removed() {
        // Windows 路径里冒号非法，这一条是硬要求
        let d = backup_dir_name("2026-10-28T11:30:00+08:00", "abcdef0123456789");
        assert!(!d.contains(':'), "实际: {d}");
        assert_eq!(d, "20261028T113000+0800-abcdef01");
    }

    #[test]
    fn short_hash_survives_short_input() {
        // 计划指纹异常短时不能 panic
        let d = backup_dir_name("2026-10-28T11:30:00", "ab");
        assert!(d.ends_with("-ab"));
    }
}
