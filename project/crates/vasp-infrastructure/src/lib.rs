//! 基础设施层：实现 `vasp-domain` 的端口与 `vasp-application` 的 `Fs`。
//!
//! 只做「转换与传输」。是否收敛、是否覆盖、路径是否合法，全在领域层。

pub mod fs;
pub mod hermes;
pub mod logging;
pub mod store;

pub use fs::*;
pub use hermes::*;
pub use logging::*;
pub use store::*;
