//! 调用 `hermes chat --format stream-json` 并解析事件流。
//!
//! 实测事实（见 `docs/m3-probe-findings.md`，v0.22.5）：
//! - stdout 是纯 JSONL；`Session:` 只在 stderr
//! - `text` 事件是 **token 级片段**，必须拼接
//! - 成功判据是 `result.exit_code`，不是进程退出码
//! - `--run-budget` 不被支持，超时只能由我们 kill 子进程
//! - 必须显式传一个无害 toolset：`-t ''` 会回落到默认工具集（含 terminal）

use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use vasp_application::ports::Analyzer;
use vasp_application::{AppError, AppResult};
use vasp_domain::{AnalysisReport, TokenUsage, ToolCall};

/// 默认 toolset。
///
/// 实测：`-t ''` 或省略 **会回落到默认工具集**，AI 甚至能"看到" terminal
/// （虽然调用失败）。`todo` 是实测里最干净的一档——只有 `todo_list`，
/// 不能读写文件、不能联网、不能执行命令。
pub const SAFE_TOOLSET: &str = "todo";

pub struct HermesRunner {
    bin: String,
    model: Option<String>,
    provider: Option<String>,
    max_turns: u32,
    timeout: Duration,
    toolset: String,
}

impl HermesRunner {
    pub fn new(bin: impl Into<String>) -> Self {
        Self {
            bin: bin.into(),
            model: None,
            provider: None,
            max_turns: 2,
            timeout: Duration::from_secs(180),
            toolset: SAFE_TOOLSET.to_string(),
        }
    }

    pub fn with_model(mut self, model: Option<String>) -> Self {
        self.model = model;
        self
    }

    pub fn with_provider(mut self, provider: Option<String>) -> Self {
        self.provider = provider;
        self
    }

    pub fn with_max_turns(mut self, n: u32) -> Self {
        self.max_turns = n.max(1);
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout = Duration::from_secs(secs.max(5));
        self
    }

    /// 问一次。提示词走文件（避免引号、反斜杠、命令行长度限制）。
    pub fn run(&self, prompt: &str) -> AppResult<AnalysisReport> {
        let prompt_file = self.write_prompt(prompt)?;
        let result = self.run_with(&prompt_file);
        let _ = std::fs::remove_file(&prompt_file);
        result
    }

    fn write_prompt(&self, prompt: &str) -> AppResult<PathBuf> {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "dftwork-prompt-{}-{}.txt",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        std::fs::write(&path, prompt.as_bytes())
            .map_err(|e| AppError::io(path.display().to_string(), e))?;
        Ok(path)
    }

    fn run_with(&self, prompt_file: &std::path::Path) -> AppResult<AnalysisReport> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("chat")
            .arg("--query-file")
            .arg(prompt_file)
            .arg("--format")
            .arg("stream-json")
            .arg("--source")
            .arg("tool")
            .arg("--quiet")
            .arg("--max-turns")
            .arg(self.max_turns.to_string())
            // 隔离用户的 persona / skills / memory：实测不隔离时 AI 会自称
            // 用户的 SOUL.md 人格，产物带人设语气且多耗 token
            .arg("--ignore-user-config")
            .arg("--ignore-rules")
            .arg("-t")
            .arg(&self.toolset)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8");

        // --ignore-user-config 读不到 profile 默认模型，必须显式给
        if let Some(m) = &self.model {
            cmd.arg("--model").arg(m);
        }
        if let Some(p) = &self.provider {
            cmd.arg("--provider").arg(p);
        }

        let started = Instant::now();
        let mut child = cmd.spawn().map_err(|e| {
            AppError::External(format!("无法启动 {}：{e}", self.bin))
        })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::External("拿不到子进程 stdout".to_string()))?;

        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });

        let mut text = String::new();
        let mut tool_calls = Vec::new();
        let mut session_id = String::new();
        let mut exit_code: Option<i32> = None;
        let mut tokens = TokenUsage::default();
        let mut duration_ms = 0u64;
        let mut result_text = String::new();
        let mut lines = 0usize;
        let mut timed_out = false;

        loop {
            let remaining = self.timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                let _ = child.kill();
                timed_out = true;
                break;
            }

            match rx.recv_timeout(remaining) {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    lines += 1;
                    let Ok(v) = serde_json::from_str::<Value>(line) else {
                        continue; // 非 JSON 行不该出现，但出现了也不要崩
                    };
                    match v.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            if let Some(t) = v.get("text").and_then(Value::as_str) {
                                text.push_str(t);
                            }
                        }
                        Some("tool_use") => {
                            tool_calls.push(ToolCall {
                                name: v
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .unwrap_or("?")
                                    .to_string(),
                                summary: truncate(
                                    &v.get("input").map(|i| i.to_string()).unwrap_or_default(),
                                    160,
                                ),
                                is_error: false,
                            });
                        }
                        Some("tool_result") => {
                            if v.get("is_error").and_then(Value::as_bool).unwrap_or(false) {
                                if let Some(last) = tool_calls.last_mut() {
                                    last.is_error = true;
                                }
                            }
                        }
                        Some("result") => {
                            session_id = v
                                .get("session_id")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            exit_code = v
                                .get("exit_code")
                                .and_then(Value::as_i64)
                                .map(|n| n as i32);
                            result_text = v
                                .get("text")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            duration_ms =
                                v.get("duration_ms").and_then(Value::as_u64).unwrap_or(0);
                            if let Some(t) = v.get("tokens") {
                                tokens = TokenUsage {
                                    input: t.get("input").and_then(Value::as_u64).unwrap_or(0),
                                    output: t.get("output").and_then(Value::as_u64).unwrap_or(0),
                                    total: t.get("total").and_then(Value::as_u64).unwrap_or(0),
                                    cache_read: t
                                        .get("cache_read")
                                        .and_then(Value::as_u64)
                                        .unwrap_or(0),
                                    cache_write: t
                                        .get("cache_write")
                                        .and_then(Value::as_u64)
                                        .unwrap_or(0),
                                };
                            }
                        }
                        _ => {}
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let _ = child.kill();
                    timed_out = true;
                    break;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        let _ = child.wait();

        // 拿不到 result 事件时（被 kill、崩溃、协议变更），用已拼好的 text 兜底，
        // exit_code 记 -1 让上层知道「这次不算成功」
        let exit_code = exit_code.unwrap_or(-1);
        if text.is_empty() && !result_text.is_empty() {
            text = result_text;
        }

        Ok(AnalysisReport {
            session_id,
            exit_code,
            text,
            tokens,
            duration_ms,
            event_lines: lines,
            tool_calls,
            timed_out,
        })
    }
}

impl Analyzer for HermesRunner {
    fn ask(&self, prompt: &str) -> AppResult<AnalysisReport> {
        self.run(prompt)
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let head: String = s.chars().take(n).collect();
        format!("{head}…")
    }
}
