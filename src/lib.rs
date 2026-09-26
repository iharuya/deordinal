mod code;
mod html;
mod markdown;
mod rules;

use std::{ops::Range, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub rule: &'static str,
    pub message: String,
    pub start: usize,
    pub end: usize,
    pub severity: Severity,
    pub(crate) fix: Option<Range<usize>>,
}

impl Diagnostic {
    fn warning(rule: &'static str, message: &str, start: usize, end: usize) -> Self {
        Self {
            rule,
            message: message.into(),
            start,
            end,
            severity: Severity::Warning,
            fix: None,
        }
    }

    fn error(message: &str, start: usize, end: usize) -> Self {
        Self {
            rule: "ignore",
            message: message.into(),
            start,
            end,
            severity: Severity::Error,
            fix: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Markdown,
    Html,
    JavaScript,
    Jsx,
    TypeScript,
    Tsx,
    Python,
}

impl Language {
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "md" => Some(Self::Markdown),
            "html" | "htm" => Some(Self::Html),
            "js" | "mjs" | "cjs" => Some(Self::JavaScript),
            "jsx" => Some(Self::Jsx),
            "ts" | "mts" | "cts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "py" | "pyi" => Some(Self::Python),
            _ => None,
        }
    }
}

pub fn check(source: &str, language: Language) -> Vec<Diagnostic> {
    let mut diagnostics = match language {
        Language::Markdown => markdown::check(source),
        Language::Html => html::check(source),
        _ => code::check(source, language),
    };
    diagnostics.sort_by(|a, b| a.start.cmp(&b.start).then(a.rule.cmp(b.rule)));
    diagnostics
}

/// Applies supported, explicitly unsafe edits. Returns `None` when nothing can be changed.
pub fn fix_unsafe(source: &str, language: Language) -> Option<String> {
    let mut result = source.to_owned();
    let mut changed = false;
    while let Some(next) = fix_once(&result, language) {
        result = next;
        changed = true;
    }
    changed.then_some(result)
}

fn fix_once(source: &str, language: Language) -> Option<String> {
    let diagnostics = check(source, language);
    if diagnostics.iter().any(|d| d.severity == Severity::Error) {
        return None;
    }
    let mut edits: Vec<_> = diagnostics.into_iter().filter_map(|d| d.fix).collect();
    edits.sort_by_key(|range| (range.start, range.end));
    let mut result = source.to_owned();
    let mut boundary = source.len();
    let mut accepted = Vec::new();
    for range in edits.into_iter().rev() {
        if range.start < range.end && range.end <= boundary && source.get(range.clone()).is_some() {
            result.replace_range(range.clone(), "");
            boundary = range.start;
            accepted.push(range);
        }
    }
    if accepted.is_empty() {
        return None;
    }
    if preserves_structure(source, &result, language) {
        return Some(result);
    }
    let mut result = source.to_owned();
    for range in accepted {
        let mut candidate = result.clone();
        candidate.replace_range(range, "");
        if preserves_structure(&result, &candidate, language) {
            result = candidate;
        }
    }
    (result != source).then_some(result)
}

fn preserves_structure(before: &str, after: &str, language: Language) -> bool {
    match language {
        Language::Markdown => markdown::same_structure(before, after),
        Language::Html => html::same_structure(before, after),
        _ => true,
    }
}

pub struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(source: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(source.match_indices('\n').map(|(at, _)| at + 1));
        Self { starts }
    }

    pub fn line_column(&self, source: &str, byte: usize) -> (usize, usize) {
        let line = self.starts.partition_point(|&start| start <= byte);
        (
            line,
            source[self.starts[line - 1]..byte].chars().count() + 1,
        )
    }
}

pub(crate) fn physical_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut start = 0;
    source.split_inclusive('\n').map(move |part| {
        let offset = start;
        start += part.len();
        (offset, part.trim_end_matches('\n').trim_end_matches('\r'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_rules_use_public_names() {
        let diagnostics = check(
            "# 1. 概要\n\n1. first\n2. second\n# Step 1\n<!-- deordinal-ignore -->\n",
            Language::Markdown,
        );
        let rules: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.rule)
            .collect();
        assert_eq!(
            rules,
            ["prefix", "ordered-list", "keyword-prefix", "ignore"]
        );
    }

    #[test]
    fn fixes_markdown_prose_without_changing_syntax() {
        let source = "\u{feff}# 1. 概要\r\n> Phase A: 計画\r\n- Step 1: 準備\r\n- [ ] 2: 確認\r\n普通の段落\r\n3: 続き\r\n";
        let expected = "\u{feff}# 概要\r\n> 計画\r\n- 準備\r\n- [ ] 確認\r\n普通の段落\r\n続き\r\n";
        assert_eq!(
            fix_unsafe(source, Language::Markdown).as_deref(),
            Some(expected)
        );
        assert!(fix_unsafe(expected, Language::Markdown).is_none());
    }

    #[test]
    fn leaves_unsupported_markdown_unchanged() {
        let source = "# Step 1\n# Step 2 <!-- note -->\n1. first\n2. second\n- Step 3\n```\n# Step 4: code\n```\n";
        assert!(fix_unsafe(source, Language::Markdown).is_none());
        assert_eq!(check(source, Language::Markdown).len(), 4);
    }

    #[test]
    fn fixes_headings_with_formatted_prose() {
        let source = "# 1. **Strong**\n## Step 1: [Link](target)\n";
        let expected = "# **Strong**\n## [Link](target)\n";
        assert_eq!(
            fix_unsafe(source, Language::Markdown).as_deref(),
            Some(expected)
        );
    }

    #[test]
    fn honors_ignore_and_rejects_invalid_directives() {
        let source = "# Step 1: before\n<!-- deordinal-ignore-start: required -->\n# Step 2: ignored\n<!-- deordinal-ignore-end -->\n# Step 3: after\n";
        let expected = "# before\n<!-- deordinal-ignore-start: required -->\n# Step 2: ignored\n<!-- deordinal-ignore-end -->\n# after\n";
        assert_eq!(
            fix_unsafe(source, Language::Markdown).as_deref(),
            Some(expected)
        );
        let invalid = "# Step 1: keep\n<!-- deordinal-ignore -->\n";
        assert!(fix_unsafe(invalid, Language::Markdown).is_none());
    }

    #[test]
    fn skips_markdown_fixes_that_change_parsing_without_losing_other_edits() {
        let source = "1: ---\n# Step 1: Title\n";
        assert_eq!(
            fix_unsafe(source, Language::Markdown).as_deref(),
            Some("1: ---\n# Title\n")
        );
    }

    #[test]
    fn fixes_stacked_labels_in_one_run() {
        let markdown = "# 1: Phase A: Plan\n";
        let code = "// 1: Step 2: Setup\n";
        assert_eq!(
            fix_unsafe(markdown, Language::Markdown).as_deref(),
            Some("# Plan\n")
        );
        assert_eq!(
            fix_unsafe(code, Language::JavaScript).as_deref(),
            Some("// Setup\n")
        );
        assert!(fix_unsafe("# Plan\n", Language::Markdown).is_none());
    }

    #[test]
    fn fixes_only_comments_not_strings_or_docstrings() {
        for language in [
            Language::JavaScript,
            Language::Jsx,
            Language::TypeScript,
            Language::Tsx,
        ] {
            let source = "const x = '// Step 1: literal'; // 1. 初期化\n// someCode()\n// 2. 実行\n/* Phase A: 準備 */\n/* Step 1 */\n";
            let expected = "const x = '// Step 1: literal'; // 初期化\n// someCode()\n// 実行\n/* 準備 */\n/* Step 1 */\n";
            assert_eq!(
                fix_unsafe(source, language).as_deref(),
                Some(expected),
                "{language:?}"
            );
            assert!(fix_unsafe(expected, language).is_none());
        }
        let python = "\"\"\"Step 1: docstring\"\"\"\n# ① 準備\nvalue = '1. ordinary'\n";
        let expected = "\"\"\"Step 1: docstring\"\"\"\n# 準備\nvalue = '1. ordinary'\n";
        assert_eq!(
            fix_unsafe(python, Language::Python).as_deref(),
            Some(expected)
        );
    }

    #[test]
    fn unicode_column_and_crlf() {
        let text = "あ🐱: Step 1\r\n# Phase A\n";
        let index = LineIndex::new(text);
        assert_eq!(index.line_column(text, text.find("Step").unwrap()), (1, 5));
        assert_eq!(index.line_column(text, text.find("Phase").unwrap()), (2, 3));
    }
}
