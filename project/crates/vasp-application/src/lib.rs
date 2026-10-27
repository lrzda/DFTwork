//! 应用层：把领域能力编排成用例。
//!
//! 这一层是**唯一**知道「一次操作从头到尾怎么走」的地方：
//! 读文件、组织数据、决定事务边界、把系统错误翻译成错误码。
//!
//! 它不依赖 `vasp-infrastructure`——所需的文件能力由 [`ports::Fs`] 抽象，
//! 具体实现由 `vasp-cli`（组合根）注入。

pub mod dto;
pub mod error;
pub mod ports;
pub mod use_cases;

pub use error::*;
pub use ports::*;
pub use use_cases::*;
