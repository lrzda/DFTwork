use crate::WriteAction;

/// 覆盖策略：**存在即冲突**，不静默覆盖。
///
/// 三种处置只有三种来源：
/// 1. 目标不存在 → 创建；
/// 2. 已存在且内容完全一致 → 跳过（重复构建不该刷出新文件）；
/// 3. 已存在且内容不同 → 覆盖，但必须由用户在 diff 面板逐项确认。
pub fn decide(exists: bool, same_content: bool) -> WriteAction {
    match (exists, same_content) {
        (false, _) => WriteAction::Create,
        (true, true) => WriteAction::Skip,
        (true, false) => WriteAction::Overwrite,
    }
}

/// 是否需要用户确认。
pub fn needs_confirmation(action: WriteAction) -> bool {
    action == WriteAction::Overwrite
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix() {
        assert_eq!(decide(false, false), WriteAction::Create);
        assert_eq!(decide(true, true), WriteAction::Skip);
        assert_eq!(decide(true, false), WriteAction::Overwrite);
        assert!(needs_confirmation(WriteAction::Overwrite));
        assert!(!needs_confirmation(WriteAction::Create));
    }
}
