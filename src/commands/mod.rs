//! 子命令实现模块聚合。

pub mod trt;

/// 图形界面子命令（仅在启用 `gui` feature 时编译）。
#[cfg(feature = "gui")]
pub mod gui;
