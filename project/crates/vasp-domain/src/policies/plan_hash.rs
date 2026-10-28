use crate::{BuildItem, BuildPlan, DomainError, DomainResult};
use sha2::{Digest, Sha256};

/// 计划指纹。
///
/// 只覆盖「路径 + 动作 + 内容」三项，**不含 `reason`**——
/// 说明文字改一改不该让用户已经看过的计划失效。
/// 条目按路径排序后摘要，保证顺序无关。
pub fn compute(items: &[BuildItem]) -> String {
    let mut sorted: Vec<&BuildItem> = items.iter().collect();
    sorted.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    let mut h = Sha256::new();
    for it in sorted {
        h.update(it.rel_path.as_bytes());
        h.update([0u8]);
        h.update(action_tag(it.action).as_bytes());
        h.update([0u8]);
        h.update(it.content.as_bytes());
        h.update([0u8, 0u8]);
    }
    let hex = format!("{:x}", h.finalize());
    hex[..32].to_string()
}

fn action_tag(a: crate::WriteAction) -> &'static str {
    match a {
        crate::WriteAction::Create => "create",
        crate::WriteAction::Overwrite => "overwrite",
        crate::WriteAction::Skip => "skip",
    }
}

/// 放行时校验：用户看到的计划还是不是当前这一份。
///
/// 这个判断是**领域规则**，所以界面、CLI、AI 工具走同一个用例时自动生效，
/// 不需要各自再写一遍。
pub fn assert_match(plan: &BuildPlan) -> DomainResult<()> {
    let actual = compute(&plan.items);
    if actual == plan.plan_hash {
        Ok(())
    } else {
        Err(DomainError::PlanHashMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WriteAction;

    fn item(path: &str, content: &str) -> BuildItem {
        BuildItem {
            rel_path: path.into(),
            content: content.into(),
            action: WriteAction::Create,
            reason: "test".into(),
            previous_content: None,
        }
    }

    #[test]
    fn order_does_not_matter() {
        let a = vec![item("a/INCAR", "x"), item("b/INCAR", "y")];
        let b = vec![item("b/INCAR", "y"), item("a/INCAR", "x")];
        assert_eq!(compute(&a), compute(&b));
    }

    #[test]
    fn content_change_changes_hash() {
        let a = vec![item("a/INCAR", "ENCUT=520")];
        let b = vec![item("a/INCAR", "ENCUT=600")];
        assert_ne!(compute(&a), compute(&b));
    }

    #[test]
    fn reason_change_keeps_hash() {
        let mut a = vec![item("a/INCAR", "x")];
        let mut b = vec![item("a/INCAR", "x")];
        a[0].reason = "换了说明".into();
        b[0].reason = "另一句说明".into();
        assert_eq!(compute(&a), compute(&b));
    }
}
