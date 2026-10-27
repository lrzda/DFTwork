use crate::{InputPresence, OutputPresence, Task};
use std::path::PathBuf;

/// 判据：目录里存在 `OUTCAR` 或 `vasprun.xml` 才算一个跑过的任务。
///
/// **待确认**（见 docs/rust-rewrite.md §11.2 `scanSingleDir` 判据）：
/// 原实现是否把「只有 INCAR+POSCAR」的目录也算任务、是否下探子目录、
/// 是否认 `OUTCAR.gz` / `OUTCAR.bak`。首版取最保守的一条，只认完整输出文件。
pub const TASK_MARKERS: [&str; 2] = ["OUTCAR", "vasprun.xml"];

/// 输入文件（大小写不敏感匹配）。
pub const INPUT_FILES: [&str; 4] = ["POSCAR", "INCAR", "KPOINTS", "POTCAR"];

/// 输出文件。
pub const OUTPUT_FILES: [&str; 5] = ["OUTCAR", "OSZICAR", "vasprun.xml", "CONTCAR", "XDATCAR"];

fn has(files: &[String], name: &str) -> bool {
    files.iter().any(|f| f.eq_ignore_ascii_case(name))
}

/// 只看文件名，不读内容——这是纯函数，可毫秒级测试。
pub fn is_task_dir(files: &[String]) -> bool {
    TASK_MARKERS.iter().any(|m| has(files, m))
}

pub fn presence(files: &[String]) -> (InputPresence, OutputPresence) {
    let inputs = InputPresence {
        poscar: has(files, "POSCAR"),
        incar: has(files, "INCAR"),
        kpoints: has(files, "KPOINTS"),
        potcar: has(files, "POTCAR"),
    };
    let outputs = OutputPresence {
        outcar: has(files, "OUTCAR"),
        oszicar: has(files, "OSZICAR"),
        vasprun: has(files, "vasprun.xml"),
        contcar: has(files, "CONTCAR"),
        xdatcar: has(files, "XDATCAR"),
    };
    (inputs, outputs)
}

/// 组装一个任务。`files` 是目录里所有文件/子目录名。
///
/// 返回 `None` 表示这不是任务目录（调用方决定是否当「未开始」展示）。
pub fn detect(rel_path: &str, abs_path: PathBuf, files: &[String]) -> Option<Task> {
    if !is_task_dir(files) {
        return None;
    }
    let (inputs, outputs) = presence(files);
    Some(Task {
        rel_path: rel_path.to_string(),
        abs_path,
        files: files.to_vec(),
        inputs,
        outputs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn outcar_marks_a_task() {
        assert!(is_task_dir(&v(&["INCAR", "OUTCAR"])));
    }

    #[test]
    fn vasprun_marks_a_task() {
        assert!(is_task_dir(&v(&["vasprun.xml"])));
    }

    #[test]
    fn inputs_only_is_not_a_task() {
        assert!(!is_task_dir(&v(&["INCAR", "POSCAR", "KPOINTS", "POTCAR"])));
    }

    #[test]
    fn presence_is_case_insensitive() {
        let (i, o) = presence(&v(&["poscar", "Incar", "outcar"]));
        assert!(i.poscar && i.incar && !i.kpoints);
        assert!(o.outcar && !o.oszicar);
    }
}
