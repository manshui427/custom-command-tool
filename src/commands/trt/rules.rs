//! 替换规则集的构建与规则文件解析。
//!
//! 对应 spec 的 FR-007a/FR-007b 与 data-model.md 实体 2/3：
//! - 单条规则由命令行 `-o/-n` 给出；
//! - 多条规则由 `--rules` 文件给出（每行一组，旧/新以制表符分隔，`#` 注释行与空行忽略）；
//! - 同时提供时，命令行单组规则**追加**到规则集末尾。
//!
//! 规则在集合中的顺序（`order`）决定重叠命中时的优先级（小者优先，见 FR-008a）。

use std::fs;
use std::path::Path;

use crate::error::{CctError, CctResult};

/// 规则文件中分隔旧/新文本的字符（制表符）。
const RULE_SEPARATOR: char = '\t';

/// 单条替换规则（参见 data-model.md 实体 2）。
#[derive(Debug, Clone)]
pub struct ReplacementRule {
    /// 被替换文本（字面串或正则模式串），非空。
    pub old_text: String,
    /// 替换文本；正则模式下可含 `$1`/`${name}` 捕获组引用，可为空。
    pub new_text: String,
}

/// 替换规则集（参见 data-model.md 实体 3）。
///
/// 包含一条或多条有序规则，以及对所有规则统一生效的全局开关。
#[derive(Debug)]
pub struct RuleSet {
    /// 有序规则列表，至少一条。
    pub rules: Vec<ReplacementRule>,
    /// 是否大小写敏感（对应 -c）。
    pub case_sensitive: bool,
    /// 是否使用正则（对应 -r）。
    pub use_regex: bool,
}

impl RuleSet {
    /// 由命令行单组规则与可选规则文件构建规则集。
    ///
    /// 规则顺序：规则文件中的规则在前（按文件行序），命令行单组规则追加在末尾（FR-007b）。
    ///
    /// # 错误
    /// - 规则文件无法读取；
    /// - 规则文件中存在格式非法的行（缺少分隔符）；
    /// - 最终规则集为空（无任何有效规则）。
    pub fn build(
        old_text: Option<String>,
        new_text: Option<String>,
        rules_file: Option<&Path>,
        case_sensitive: bool,
        use_regex: bool,
    ) -> CctResult<Self> {
        let mut rules = Vec::new();

        // 1. 先解析规则文件（若提供）。
        if let Some(path) = rules_file {
            rules.extend(parse_rules_file(path)?);
        }

        // 2. 命令行单组规则追加在末尾。
        if let Some(o) = old_text {
            // 调用方已保证 old_text 非空；new_text 缺省视为空串（表示删除匹配内容）。
            rules.push(ReplacementRule {
                old_text: o,
                new_text: new_text.unwrap_or_default(),
            });
        }

        if rules.is_empty() {
            // 走到这里说明仅提供了规则文件但其中无有效规则。
            let p = rules_file.map(Path::to_path_buf).unwrap_or_default();
            return Err(CctError::NoValidRules(p));
        }

        Ok(RuleSet {
            rules,
            case_sensitive,
            use_regex,
        })
    }
}

/// 解析规则文件，返回有序规则列表。
///
/// 每行格式：`<旧文本><制表符><新文本>`。空行与以 `#` 起始的行被忽略。
/// 缺少分隔符的行被视为非法（返回 [`CctError::InvalidRuleLine`]）。
fn parse_rules_file(path: &Path) -> CctResult<Vec<ReplacementRule>> {
    let content = fs::read_to_string(path).map_err(|source| CctError::RulesFileUnreadable {
        path: path.to_path_buf(),
        source,
    })?;

    let mut rules = Vec::new();
    for (idx, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim_end_matches('\r');

        // 忽略空行与注释行。
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }

        // 按首个制表符分割旧/新文本。
        match line.split_once(RULE_SEPARATOR) {
            Some((old, new)) if !old.is_empty() => {
                rules.push(ReplacementRule {
                    old_text: old.to_string(),
                    new_text: new.to_string(),
                });
            }
            // 缺少分隔符，或旧文本为空，均视为非法行。
            _ => {
                return Err(CctError::InvalidRuleLine {
                    line: idx + 1,
                    content: line.to_string(),
                });
            }
        }
    }

    Ok(rules)
}
