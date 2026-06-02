//! 单遍同时匹配替换引擎。
//!
//! 实现 spec 的 FR-008a/FR-016a/FR-016b（参见 research.md §5/§6）：
//! - 字面模式（-r 0）使用 aho-corasick 多模式自动机，一次扫描同时匹配所有规则；
//! - 正则模式（-r 1）将多条规则合并为一个带分支的正则，一次扫描匹配；
//! - 统一语义：最左、不重叠、重叠时按规则在规则集中的先后顺序优先、替换输出不再参与后续匹配（无级联）；
//! - 大小写开关（-c）对所有规则统一生效；正则模式下 `-n` 支持 `$1`/`${name}` 捕获组引用。
//!
//! 字面模式额外提供**流式替换**（[`Matcher::stream_replace_literal`]），由 aho-corasick 内部
//! 处理跨块匹配，内存占用恒定，用于超过阈值的大文件（FR-019/SC-003）。

use std::cell::Cell;
use std::io::{Read, Write};

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use regex::Regex;

use crate::error::{CctError, CctResult};

use super::rules::RuleSet;

/// 编译后的替换匹配器。
pub enum Matcher {
    /// 字面多模式匹配器。
    Literal {
        /// aho-corasick 自动机。
        ac: AhoCorasick,
        /// 与自动机模式一一对应的替换文本（按规则顺序）。
        replacements: Vec<String>,
    },
    /// 正则匹配器。
    ///
    /// 每条规则保留独立编译的 `Regex` 与替换模板，逐位置按"最左 + 规则顺序优先"选择命中规则，
    /// 并用该规则自身的 `Regex` 展开捕获组引用，从而保证 `$1`/`${name}` 指向规则内部的分组。
    Regex {
        /// 各规则独立编译的正则，按规则顺序。
        regexes: Vec<Regex>,
        /// 各规则的替换模板（含捕获组引用），按规则顺序。
        templates: Vec<String>,
    },
}

impl Matcher {
    /// 由规则集编译匹配器。
    ///
    /// # 错误
    /// 正则模式下，任一规则的模式串非法时返回 [`CctError::InvalidRegex`]。
    pub fn compile(rule_set: &RuleSet) -> CctResult<Self> {
        if rule_set.use_regex {
            Self::compile_regex(rule_set)
        } else {
            Self::compile_literal(rule_set)
        }
    }

    /// 是否为字面模式（用于决定大文件能否走流式路径）。
    pub fn is_literal(&self) -> bool {
        matches!(self, Matcher::Literal { .. })
    }

    /// 编译字面匹配器。
    fn compile_literal(rule_set: &RuleSet) -> CctResult<Self> {
        let patterns: Vec<&str> = rule_set.rules.iter().map(|r| r.old_text.as_str()).collect();
        let replacements: Vec<String> = rule_set.rules.iter().map(|r| r.new_text.clone()).collect();

        // MatchKind::LeftmostFirst：最左优先，且同一位置多个模式命中时取"先登记"的模式，
        // 恰好实现"按规则顺序优先 + 不重叠 + 无级联"语义。
        let ac = AhoCorasickBuilder::new()
            .match_kind(MatchKind::LeftmostFirst)
            .ascii_case_insensitive(!rule_set.case_sensitive)
            .build(&patterns)
            .map_err(|e| CctError::InvalidRegex(e.to_string()))?;

        Ok(Matcher::Literal { ac, replacements })
    }

    /// 编译正则匹配器：为每条规则独立编译一个 `Regex`。
    ///
    /// 大小写不敏感时为每条规则模式加 `(?i)` 前缀（FR-016b，统一生效）。
    fn compile_regex(rule_set: &RuleSet) -> CctResult<Self> {
        let mut regexes = Vec::with_capacity(rule_set.rules.len());
        let mut templates = Vec::with_capacity(rule_set.rules.len());

        for rule in &rule_set.rules {
            let pattern = if rule_set.case_sensitive {
                rule.old_text.clone()
            } else {
                format!("(?i){}", rule.old_text)
            };
            let re = Regex::new(&pattern).map_err(|e| CctError::InvalidRegex(e.to_string()))?;
            regexes.push(re);
            templates.push(rule.new_text.clone());
        }

        Ok(Matcher::Regex { regexes, templates })
    }

    /// 对内存中的整段文本执行替换，返回 (替换后文本, 替换次数)。
    ///
    /// 用于小文件整体替换路径。无匹配时替换次数为 0。
    pub fn replace(&self, input: &str) -> (String, u64) {
        match self {
            Matcher::Literal { ac, replacements } => {
                let mut count = 0u64;
                let mut out = String::with_capacity(input.len());
                let mut last = 0usize;
                for m in ac.find_iter(input) {
                    out.push_str(&input[last..m.start()]);
                    out.push_str(&replacements[m.pattern()]);
                    last = m.end();
                    count += 1;
                }
                out.push_str(&input[last..]);
                (out, count)
            }
            Matcher::Regex { regexes, templates } => {
                let mut count = 0u64;
                let mut out = String::with_capacity(input.len());
                let mut pos = 0usize;

                while pos <= input.len() {
                    // 在 pos 之后，找出所有规则中起始位置最靠左的匹配；
                    // 起始位置相同时，规则顺序在前者优先（实现"规则顺序优先 + 不重叠 + 无级联"）。
                    let mut best: Option<(usize, usize, usize)> = None; // (start, end, rule_idx)
                    for (idx, re) in regexes.iter().enumerate() {
                        if let Some(m) = re.find_at(input, pos) {
                            let cand = (m.start(), m.end(), idx);
                            match best {
                                Some((bs, _, _)) if bs <= cand.0 => {}
                                _ => best = Some(cand),
                            }
                        }
                    }

                    let (start, end, rule_idx) = match best {
                        Some(b) => b,
                        None => break, // 后续无任何匹配。
                    };

                    // 输出匹配前的原文。
                    out.push_str(&input[pos..start]);

                    // 用命中规则自身的正则在该位置取捕获并展开模板（$1/${name} 指向规则内部分组）。
                    if let Some(caps) = regexes[rule_idx].captures_at(input, start) {
                        let mut expanded = String::new();
                        caps.expand(&templates[rule_idx], &mut expanded);
                        out.push_str(&expanded);
                    }
                    count += 1;

                    // 处理零宽匹配，避免死循环。
                    if end == start {
                        if let Some(c) = input[start..].chars().next() {
                            out.push(c);
                            pos = start + c.len_utf8();
                        } else {
                            break;
                        }
                    } else {
                        pos = end;
                    }
                }

                // 输出剩余原文。
                out.push_str(&input[pos.min(input.len())..]);
                (out, count)
            }
        }
    }

    /// 字面模式的流式替换：从 `reader` 读取、替换后写入 `writer`，返回替换次数。
    ///
    /// 由 aho-corasick 内部处理跨块匹配，内存占用恒定（用于大文件，FR-019/SC-003）。
    /// 仅 [`Matcher::Literal`] 支持；对正则匹配器调用将返回 0 且不写入（调用方应改走整体路径）。
    pub fn stream_replace_literal<R: Read, W: Write>(
        &self,
        reader: R,
        writer: W,
    ) -> std::io::Result<u64> {
        match self {
            Matcher::Literal { ac, replacements } => {
                let count = Cell::new(0u64);
                ac.try_stream_replace_all_with(reader, writer, |mat, _bytes, wtr| {
                    count.set(count.get() + 1);
                    wtr.write_all(replacements[mat.pattern()].as_bytes())
                })?;
                Ok(count.get())
            }
            Matcher::Regex { .. } => Ok(0),
        }
    }
}
