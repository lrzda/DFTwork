//! 让 AI 分析一个 VASP 任务。
//!
//! **数据由我们收集并注入提示词，AI 不调用任何工具**。这样：
//! - 零配置：用户装好 Hermes 就能用，不用建 profile、不用改 config
//! - 安全：AI 碰不到文件系统，写权限仍然只有 `ApplyPlan` 一条通路
//! - 可控：塞多少数据、花多少 token，由这里决定

use crate::ports::{Analyzer, Fs, MAX_FULL_READ};
use crate::use_cases::GetConvergence;
use crate::{AppError, AppResult};
use serde_json::json;
use std::path::Path;
use vasp_domain::{AnalysisReport, Incar};

/// 收敛序列只保留首尾若干步。
///
/// 一个 数百步的弛豫任务，全量塞进提示词要几千 token，而这些点对「是否收敛」
/// 的判断贡献几乎为零——首步给出起点，末几步给出收敛趋势，足够了。
const HEAD_POINTS: usize = 2;
const TAIL_POINTS: usize = 6;

pub struct AnalyzeTask<'a> {
    fs: &'a dyn Fs,
    analyzer: &'a dyn Analyzer,
}

impl<'a> AnalyzeTask<'a> {
    pub fn new(fs: &'a dyn Fs, analyzer: &'a dyn Analyzer) -> Self {
        Self { fs, analyzer }
    }

    /// 收集该任务的关键数据，问 AI 一次，返回结果。
    pub fn execute(&self, task_dir: &Path) -> AppResult<AnalysisReport> {
        let ctx = self.collect(task_dir)?;
        let prompt = build_prompt(&ctx);
        self.analyzer.ask(&prompt)
    }

    fn collect(&self, dir: &Path) -> AppResult<serde_json::Value> {
        if !self.fs.is_dir(dir) {
            return Err(AppError::NotFound(format!(
                "任务目录不存在：{}",
                dir.display()
            )));
        }

        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| dir.display().to_string());

        // INCAR：体积小，给全文（AI 需要看到 ENCUT/EDIFF/ISMEAR 这些才判断得了）
        let incar_text = self
            .fs
            .read_text(&dir.join("INCAR"), MAX_FULL_READ)
            .unwrap_or_default();
        let incar = Incar::parse(&incar_text);
        let params: serde_json::Map<String, serde_json::Value> = incar
            .entries()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect();

        // 收敛序列：压到首尾
        let conv = GetConvergence::new(self.fs).execute(dir).ok();
        let conv_summary = conv.map(|c| {
            let n = c.steps.len();
            let mut picked = Vec::new();
            for (i, s) in c.steps.iter().enumerate() {
                if i < HEAD_POINTS || i + TAIL_POINTS >= n {
                    picked.push(json!({
                        "step": s.index,
                        "energy_ev": s.energy_ev,
                        "max_force_ev_per_a": s.max_force_ev_per_a,
                    }));
                }
            }
            json!({
                "total_steps": n,
                "first": picked.first(),
                "sampled": picked,
                "note": if n > HEAD_POINTS + TAIL_POINTS {
                    "中间步已省略，只保留首尾"
                } else {
                    "全部步骤"
                },
            })
        });

        Ok(json!({
            "task": name,
            "incar": params,
            "convergence": conv_summary,
        }))
    }
}

fn build_prompt(ctx: &serde_json::Value) -> String {
    format!(
        r#"你是 VASP 计算分析助手。下面是一个第一性原理计算任务的实测数据（JSON）。

## 硬性要求
1. **只基于给出的数据回答**。数据里没有的字段，不许推测或补全。
2. 如果某个判断依赖未提供的数据，明确写出「需要 X 才能判断」。
3. 不要复述输入数据，直接给结论。
4. 用中文，分点作答，总长不超过 350 字。

## 请回答
1. 这个任务在算什么？（依据 INCAR 参数判断）
2. 收敛情况如何？能量和力的变化说明什么？
3. 有没有可疑的输入参数搭配？
4. 还有什么值得注意的？

## 数据
```json
{}
```"#,
        serde_json::to_string_pretty(ctx).unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeAnalyzer {
        seen: Mutex<String>,
    }

    impl Analyzer for FakeAnalyzer {
        fn ask(&self, prompt: &str) -> AppResult<AnalysisReport> {
            *self.seen.lock().unwrap() = prompt.to_string();
            Ok(AnalysisReport {
                session_id: "fake".into(),
                exit_code: 0,
                text: "ok".into(),
                tokens: Default::default(),
                duration_ms: 0,
                event_lines: 1,
                tool_calls: vec![],
                timed_out: false,
            })
        }
    }

    #[test]
    fn prompt_contains_hard_rules_and_data() {
        let ctx = json!({"task": "t1", "incar": {"ENCUT": "400"}});
        let p = build_prompt(&ctx);
        // 硬性要求必须在，否则 AI 会把推测当观察写出来
        assert!(p.contains("只基于给出的数据回答"));
        assert!(p.contains("需要 X 才能判断"));
        assert!(p.contains("ENCUT"));
    }

    #[test]
    fn fake_analyzer_receives_prompt() {
        let a = FakeAnalyzer {
            seen: Mutex::new(String::new()),
        };
        let ctx = json!({"task": "x"});
        let _ = a.ask(&build_prompt(&ctx));
        assert!(a.seen.lock().unwrap().contains("\"task\""));
    }
}
