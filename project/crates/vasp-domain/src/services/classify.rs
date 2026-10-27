use crate::{OutputPresence, StatusEvidence, TaskStatus, TaskStatusRecord};

/// OUTCAR 正常结束标记。
const OUTCAR_FINISHED: &str = "General timing and accounting";

/// 明确的报错签名。**这是首版的启发式清单，须与 `reference/VASPFlow/outcar-parse.js`
/// 对拍后确认**，不要在没核对前把它当成 VASP 的完整错误集。
const ERROR_SIGNATURES: [&str; 6] = [
    "ZBRENT: fatal",
    "Error EDDDAV",
    "VERY BAD NEWS",
    "LAPACK: Routine ZPOTRF failed",
    "WARNING: Sub-Space-Matrix is not hermitian",
    "internal error in subroutine",
];

/// 由「输出事实」判定状态。入参是文本，不是路径。
///
/// 判定顺序：无输出 → 有结束标记 → 有报错 → 有推进迹象 → 无法判断。
/// 报错优先于结束标记之后的判断，因为部分任务会「跑完但中途报过错」。
pub fn classify(
    outputs: &OutputPresence,
    outcar_tail: Option<&str>,
    oszicar_tail: Option<&str>,
) -> TaskStatusRecord {
    if !outputs.any() {
        return TaskStatusRecord::new(
            TaskStatus::NoOutputEvidence,
            "目录里既没有 OUTCAR 也没有 vasprun.xml",
        );
    }

    if let Some(tail) = outcar_tail {
        if let Some(ev) = find_line(tail, "OUTCAR", &ERROR_SIGNATURES) {
            return TaskStatusRecord::new(TaskStatus::ErrorDetected, "OUTCAR 命中报错签名")
                .with_evidence(vec![ev]);
        }
    }

    if let Some(tail) = outcar_tail {
        if let Some(ev) = find_line(tail, "OUTCAR", &[OUTCAR_FINISHED]) {
            return TaskStatusRecord::new(TaskStatus::Completed, "OUTCAR 有正常结束标记")
                .with_evidence(vec![ev]);
        }
    }

    let progressing = outcar_tail
        .map(|t| t.contains("Iteration") || t.contains("FREE ENERGIE"))
        .unwrap_or(false)
        || oszicar_tail.map(|t| t.contains("DAV:") || t.contains("RMM:")).unwrap_or(false);

    if progressing {
        return TaskStatusRecord::new(
            TaskStatus::Running,
            "有输出、有推进迹象，但没有结束标记",
        );
    }

    TaskStatusRecord::new(
        TaskStatus::Unknown,
        "有输出，但没有结束标记也没有可识别的报错",
    )
}

fn find_line(haystack: &str, file: &str, needles: &[&str]) -> Option<StatusEvidence> {
    for (i, line) in haystack.lines().enumerate() {
        for n in needles {
            if line.contains(n) {
                return Some(StatusEvidence::new(
                    file,
                    Some(i + 1),
                    line.trim().chars().take(160).collect::<String>(),
                ));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outputs(outcar: bool, oszicar: bool) -> OutputPresence {
        OutputPresence {
            outcar,
            oszicar,
            ..Default::default()
        }
    }

    #[test]
    fn no_outputs() {
        let r = classify(&OutputPresence::default(), None, None);
        assert_eq!(r.status, TaskStatus::NoOutputEvidence);
    }

    #[test]
    fn finished() {
        let r = classify(
            &outputs(true, true),
            Some(" ...\n General timing and accounting informations for this job:\n"),
            None,
        );
        assert_eq!(r.status, TaskStatus::Completed);
        assert!(!r.evidence.is_empty());
    }

    #[test]
    fn error_beats_finished() {
        let r = classify(
            &outputs(true, true),
            Some(" ZBRENT: fatal error in bracketing\n General timing and accounting\n"),
            None,
        );
        assert_eq!(r.status, TaskStatus::ErrorDetected);
    }

    #[test]
    fn running_when_oszicar_shows_dav() {
        let r = classify(&outputs(true, true), Some(" Iteration    1(  60)\n"), Some("DAV:   1\n"));
        assert_eq!(r.status, TaskStatus::Running);
    }
}
