//! 进度显示封装。
//!
//! 实现 spec 的 FR-017 与 SC-002：由于采用"边扫描边处理"，文件总数未知，
//! 因此使用 indicatif 的 spinner + 计数器，而非百分比进度条。所有进度输出走 stderr，
//! 以免污染 stdout 上的结果摘要。

use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

/// 处理进度报告器。
///
/// 当进度被禁用（`--progress 0`）时，内部不创建任何进度条，所有方法成为空操作。
pub struct Progress {
    bar: Option<ProgressBar>,
}

impl Progress {
    /// 创建进度报告器。
    ///
    /// # 参数
    /// - `enabled`：是否显示进度（对应 `--progress`）。
    pub fn new(enabled: bool) -> Self {
        if !enabled {
            return Self { bar: None };
        }

        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::with_template("{spinner} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        bar.enable_steady_tick(Duration::from_millis(100));
        Self { bar: Some(bar) }
    }

    /// 更新进度消息（如"已扫描 N，已修改 M"）。
    pub fn set_message(&self, msg: String) {
        if let Some(bar) = &self.bar {
            bar.set_message(msg);
        }
    }

    /// 结束并清除进度显示。
    pub fn finish(&self) {
        if let Some(bar) = &self.bar {
            bar.finish_and_clear();
        }
    }
}
