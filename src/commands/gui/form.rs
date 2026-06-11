//! 图形界面参数表单：字段状态及其到 trt 参数的映射与校验。
//!
//! 对应 data-model.md 实体 4 与 FR-008/FR-011。把界面字段（[`GuiFormState`]）转换为
//! trt 既有的 [`TrtArgs`]，从而复用其全部校验逻辑（目录存在、旧文本非空、规则来源齐备等）。

use std::path::PathBuf;

use crate::commands::trt::args::{TrtArgs, TrtOptions};
use crate::error::CctResult;

/// 图形界面中各表单字段的当前值，与 trt 参数一一对应。
///
/// 不持久化：每次打开窗口以默认值初始化（FR-018）。
#[derive(Debug, Clone)]
pub struct GuiFormState {
    /// 目标目录（-d）。
    pub directory: String,
    /// 旧文本（-o）。
    pub old_text: String,
    /// 新文本（-n）。
    pub new_text: String,
    /// 规则文件（--rules），空表示不使用。
    pub rules_file: String,
    /// 启用备份（-b）。
    pub backup: bool,
    /// 撤销模式（-u）。
    pub undo: bool,
    /// 大小写敏感（-c）。
    pub case_sensitive: bool,
    /// 使用正则（-r）。
    pub use_regex: bool,
}

impl Default for GuiFormState {
    /// 默认值与 trt 命令行默认一致：备份开、大小写敏感、字面、非撤销。
    fn default() -> Self {
        Self {
            directory: String::new(),
            old_text: String::new(),
            new_text: String::new(),
            rules_file: String::new(),
            backup: true,
            undo: false,
            case_sensitive: true,
            use_regex: false,
        }
    }
}

impl GuiFormState {
    /// 将表单字段映射为 [`TrtArgs`]。空字符串字段按"未提供"处理。
    fn to_args(&self) -> TrtArgs {
        let trimmed = |s: &str| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        };
        TrtArgs {
            directory: PathBuf::from(self.directory.trim()),
            old_text: trimmed(&self.old_text),
            new_text: trimmed(&self.new_text),
            rules: trimmed(&self.rules_file).map(PathBuf::from),
            backup: u8::from(self.backup),
            undo: u8::from(self.undo),
            case_sensitive: u8::from(self.case_sensitive),
            regex: u8::from(self.use_regex),
            // 图形界面通过通道接收进度，不使用命令行进度条。
            progress: 0,
        }
    }

    /// 校验并归一化为 [`TrtOptions`]，复用 trt 既有校验逻辑（FR-011）。
    ///
    /// 校验失败时返回错误，供界面展示提示，不执行任何替换。
    pub fn to_options(&self) -> CctResult<TrtOptions> {
        self.to_args().into_options()
    }
}

#[cfg(test)]
mod tests {
    //! 表单逻辑层测试（不渲染窗口）：验证 GuiFormState → TrtOptions 的映射与校验，
    //! 与命令行选项语义一致（US2 验收场景 2/3，FR-008/FR-011）。

    use super::*;
    use crate::commands::trt::args::TrtMode;

    /// 构造一个指向真实临时目录、含有效单组规则的基础表单。
    fn base_form(dir: &std::path::Path) -> GuiFormState {
        GuiFormState {
            directory: dir.display().to_string(),
            old_text: "foo".to_string(),
            new_text: "bar".to_string(),
            ..GuiFormState::default()
        }
    }

    #[test]
    fn 默认开关与命令行默认一致() {
        let f = GuiFormState::default();
        assert!(f.backup, "默认启用备份");
        assert!(f.case_sensitive, "默认大小写敏感");
        assert!(!f.use_regex, "默认字面匹配");
        assert!(!f.undo, "默认非撤销");
    }

    #[test]
    fn 表单映射为选项各开关语义正确() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut f = base_form(dir.path());
        f.backup = false;
        f.case_sensitive = false;
        f.use_regex = true;

        let opts = f.to_options().expect("应校验通过");
        assert_eq!(opts.mode, TrtMode::Replace);
        assert!(!opts.backup_enabled);
        assert!(!opts.case_sensitive);
        assert!(opts.use_regex);
        assert_eq!(opts.old_text.as_deref(), Some("foo"));
        assert_eq!(opts.new_text.as_deref(), Some("bar"));
    }

    #[test]
    fn 撤销开关映射为撤销模式() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut f = base_form(dir.path());
        f.undo = true;
        let opts = f.to_options().expect("撤销模式无需旧/新文本即可校验通过");
        assert_eq!(opts.mode, TrtMode::Undo);
    }

    #[test]
    fn 目录不存在被校验拦截() {
        let f = GuiFormState {
            directory: "/path/does/not/exist/cct_gui_xyz".to_string(),
            old_text: "a".to_string(),
            new_text: "b".to_string(),
            ..GuiFormState::default()
        };
        assert!(f.to_options().is_err(), "不存在的目录应校验失败");
    }

    #[test]
    fn 空旧文本被校验拦截() {
        let dir = tempfile::TempDir::new().unwrap();
        let f = GuiFormState {
            directory: dir.path().display().to_string(),
            old_text: String::new(),
            new_text: "b".to_string(),
            ..GuiFormState::default()
        };
        // 既无 -o 也无规则文件，非撤销模式应校验失败。
        assert!(f.to_options().is_err(), "空旧文本且无规则文件应校验失败");
    }
}
