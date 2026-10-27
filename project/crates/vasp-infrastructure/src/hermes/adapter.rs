use std::process::Command;
use vasp_domain::{Hermes, HermesCapability};

/// 探测 Hermes 是否可用。
///
/// 只做「找得到 + 版本号 + 是否支持 stream-json」，不启动会话。
/// **缺席是正常状态**：`found=false` 时界面把 AI 入口置灰，其余功能照常。
pub struct HermesCli {
    bin: String,
}

impl HermesCli {
    pub fn new(bin: impl Into<String>) -> Self {
        Self { bin: bin.into() }
    }

    /// 按 PATH 里的 `hermes` 探测。
    pub fn from_path() -> Self {
        Self::new("hermes")
    }
}

impl Default for HermesCli {
    fn default() -> Self {
        Self::from_path()
    }
}

impl Hermes for HermesCli {
    fn probe(&self) -> HermesCapability {
        let output = match Command::new(&self.bin).arg("--version").output() {
            Ok(o) => o,
            Err(e) => return HermesCapability::absent(format!("未找到 {}：{e}", self.bin)),
        };

        if !output.status.success() {
            return HermesCapability::absent(format!(
                "{} --version 退出码 {:?}",
                self.bin,
                output.status.code()
            ));
        }

        let text = String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr);

        HermesCapability {
            found: true,
            bin: Some(self.bin.clone()),
            version: parse_version(&text),
            // 0.22.x 起 `chat --format stream-json` 可用；版本解析失败时按「支持」处理，
            // 真正的判据是实际调用能不能产出 JSONL（见 docs/hermes-contract.md §10）。
            supports_stream_json: true,
            reason: None,
        }
    }
}

/// 从 `hermes --version` 的多行输出里只取 Hermes 自己的版本号。
///
/// 输出长这样，**不能取「第一个像版本号的 token」**——那会抓到 `Python: 3.11.15`：
///
/// ```text
/// Hermes Agent v0.22.5+1719.gd49e2eb (2026.9.24) · upstream d49e2ebb
/// Install directory: C:\...\hermes-agent
/// Python: 3.11.15
/// ```
fn parse_version(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(rest) = line.split("Hermes Agent v").nth(1) {
            let token = rest.split_whitespace().next().unwrap_or("");
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    // 退化：任意 `X.Y.Z`
    text.split_whitespace()
        .map(|t| t.trim_start_matches('v'))
        .find(|t| {
            let parts: Vec<&str> = t.split('.').collect();
            parts.len() >= 2 && parts[0].chars().all(|c| c.is_ascii_digit())
        })
        .map(|s| s.to_string())
}
