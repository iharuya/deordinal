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
    pub(crate) fix: Option<Fix>,
}

/// Unsafe edits for one diagnostic, applied all together or not at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fix {
    Remove(Range<usize>),
    /// Replaces every item marker of an ordered list with `-`.
    Bullets(Vec<Range<usize>>),
    /// Replaces a label that starts a line of prose with `- `, making the line a list item.
    BulletItem(Range<usize>),
}

impl Fix {
    pub(crate) fn ranges(&self) -> &[Range<usize>] {
        match self {
            Self::Remove(range) | Self::BulletItem(range) => std::slice::from_ref(range),
            Self::Bullets(markers) => markers,
        }
    }

    pub(crate) fn replacement(&self) -> &'static str {
        match self {
            Self::Remove(_) => "",
            Self::Bullets(_) => "-",
            Self::BulletItem(_) => "- ",
        }
    }

    fn span(&self) -> Option<Range<usize>> {
        Some(self.ranges().first()?.start..self.ranges().last()?.end)
    }
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

/// Turning prose into list items changes the document structure, so those fixes wait until no
/// structure-preserving fix applies and are verified with relaxed rules.
fn fix_once(source: &str, language: Language) -> Option<String> {
    let diagnostics = check(source, language);
    if diagnostics.iter().any(|d| d.severity == Severity::Error) {
        return None;
    }
    let (bullet_items, others): (Vec<_>, Vec<_>) = diagnostics
        .into_iter()
        .filter_map(|d| d.fix)
        .partition(|fix| matches!(fix, Fix::BulletItem(_)));
    apply_checked(source, language, others)
        .or_else(|| apply_checked(source, language, bullet_items))
}

fn apply_checked(source: &str, language: Language, fixes: Vec<Fix>) -> Option<String> {
    let fixes = select_disjoint(source, fixes);
    if fixes.is_empty() {
        return None;
    }
    let result = apply(source, &fixes);
    if preserves_structure(source, &result, language, &fixes) {
        return Some(result);
    }
    let mut result = source.to_owned();
    for fix in fixes {
        let fix = [fix];
        let candidate = apply(&result, &fix);
        if preserves_structure(&result, &candidate, language, &fix) {
            result = candidate;
        }
    }
    (result != source).then_some(result)
}

/// Returns fixes whose spans do not overlap, ordered from the end of the source.
fn select_disjoint(source: &str, fixes: Vec<Fix>) -> Vec<Fix> {
    let mut fixes: Vec<_> = fixes
        .into_iter()
        .filter(|fix| {
            fix.ranges()
                .iter()
                .all(|range| range.start < range.end && source.get(range.clone()).is_some())
        })
        .filter_map(|fix| Some((fix.span()?, fix)))
        .collect();
    fixes.sort_by_key(|(span, _)| (span.start, span.end));
    let mut boundary = source.len();
    let mut selected = Vec::new();
    for (span, fix) in fixes.into_iter().rev() {
        if span.end <= boundary {
            boundary = span.start;
            selected.push(fix);
        }
    }
    selected
}

fn apply(source: &str, fixes_from_end: &[Fix]) -> String {
    let mut result = source.to_owned();
    for fix in fixes_from_end {
        for range in fix.ranges().iter().rev() {
            result.replace_range(range.clone(), fix.replacement());
        }
    }
    result
}

fn preserves_structure(before: &str, after: &str, language: Language, fixes: &[Fix]) -> bool {
    match language {
        Language::Markdown => markdown::same_structure(before, after, fixes),
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
            "# 1. Overview\n\n1. first\n2. second\n# Step 1\n<!-- deordinal-ignore -->\n",
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
        let source = "\u{feff}# 1. Overview\r\n> Phase A: Plan\r\n- Step 1: Prepare\r\n- [ ] 2: Confirm\r\nPlain paragraph\r\n";
        let expected =
            "\u{feff}# Overview\r\n> Plan\r\n- Prepare\r\n- [ ] Confirm\r\nPlain paragraph\r\n";
        assert_eq!(
            fix_unsafe(source, Language::Markdown).as_deref(),
            Some(expected)
        );
        assert!(fix_unsafe(expected, Language::Markdown).is_none());
    }

    #[test]
    fn leaves_unsupported_markdown_unchanged() {
        let source = "# Step 1\n# Step 2 <!-- note -->\n- Step 3\n```\n# Step 4: code\n```\n";
        assert!(fix_unsafe(source, Language::Markdown).is_none());
        assert_eq!(check(source, Language::Markdown).len(), 3);
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
    fn fixes_labels_in_inline_markup_without_breaking_it() {
        let source = "# **Step 1: Setup**\n[Phase A: Plan](target)\n*1. Overview*\n**1.** Overview\n**Step** 1: Install\n";
        let expected = "# **Setup**\n[Plan](target)\n*Overview*\nOverview\n**Step** 1: Install\n";
        assert_eq!(
            fix_unsafe(source, Language::Markdown).as_deref(),
            Some(expected)
        );
    }

    #[test]
    fn fixes_labels_that_fill_an_entire_strong_span() {
        let source = "**1.** Overview\n# __2.__ Details\n- **3.** Item\n";
        let expected = "Overview\n# Details\n- Item\n";
        assert_eq!(check(source, Language::Markdown).len(), 3);
        assert_eq!(
            fix_unsafe(source, Language::Markdown).as_deref(),
            Some(expected)
        );
    }

    #[test]
    fn fixes_formatted_labels_before_inline_code() {
        let source = "**2. `option` is enabled.**\n**2.4 `+` is visible.**\n[Step 1: `option` is available](target)\n**2. `option`**\n";
        let expected = "**`option` is enabled.**\n**`+` is visible.**\n[`option` is available](target)\n**`option`**\n";
        assert_eq!(
            fix_unsafe(source, Language::Markdown).as_deref(),
            Some(expected)
        );
        assert!(fix_unsafe(expected, Language::Markdown).is_none());
    }

    #[test]
    fn does_not_empty_link_labels_when_fixing() {
        let source = "[Step 1](target) `option` is available\n**1.**\n[Step 1: ![logo](image.png)](target)\n";
        assert!(fix_unsafe(source, Language::Markdown).is_none());
    }

    #[test]
    fn skips_removing_strong_labels_if_the_remainder_changes_markdown_structure() {
        let source = "**1.** 2. Next\n# **3.** Title\n";
        let expected = "**1.** 2. Next\n# Title\n";
        assert_eq!(
            fix_unsafe(source, Language::Markdown).as_deref(),
            Some(expected)
        );
    }

    #[test]
    fn does_not_fix_quantities_as_ordering_labels() {
        let source = "# 2.5 seconds elapsed\n**2.5 秒経過**\n# 1.29 Features\n";
        let expected = "# 2.5 seconds elapsed\n**2.5 秒経過**\n# Features\n";
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
    fn fixes_only_javascript_and_typescript_comments_not_strings() {
        for language in [
            Language::JavaScript,
            Language::Jsx,
            Language::TypeScript,
            Language::Tsx,
        ] {
            let source = "const x = '// Step 1: literal'; // 1. Initialize\n// someCode()\n// 2. Run\n/* Phase A: Prepare */\n/* Step 1 */\n";
            let expected = "const x = '// Step 1: literal'; // Initialize\n// someCode()\n// Run\n/* Prepare */\n/* Step 1 */\n";
            assert_eq!(
                fix_unsafe(source, language).as_deref(),
                Some(expected),
                "{language:?}"
            );
            assert!(fix_unsafe(expected, language).is_none());
        }
    }

    #[test]
    fn fixes_python_docstrings_and_comments_but_not_other_strings() {
        let source = "\"\"\"Step 1: module\r\n  2. details\r\n  Phase A\r\n\"\"\"\r\n# ① Prepare\r\nvalue = '1. ordinary'\r\nclass Thing:\r\n    # preface\r\n    u'''(1) class'''\r\n    def run(self):\r\n        (\"Step 2: method\")\r\n        return '3. ordinary'\r\n    async def fetch(self):\r\n        r\"\"\"Phase B: async\r\n        4. next\"\"\"\r\n";
        let expected = "\"\"\"module\r\n  details\r\n  Phase A\r\n\"\"\"\r\n# Prepare\r\nvalue = '1. ordinary'\r\nclass Thing:\r\n    # preface\r\n    u'''class'''\r\n    def run(self):\r\n        (\"method\")\r\n        return '3. ordinary'\r\n    async def fetch(self):\r\n        r\"\"\"async\r\n        next\"\"\"\r\n";
        assert_eq!(check(source, Language::Python).len(), 8);
        assert_eq!(
            fix_unsafe(source, Language::Python).as_deref(),
            Some(expected)
        );
        assert_eq!(check(expected, Language::Python).len(), 1);
        assert!(fix_unsafe(expected, Language::Python).is_none());
    }

    #[test]
    fn fixes_multiline_javascript_and_typescript_block_comments() {
        let source = "const value = `// 1. literal`; /* 2. first\n 3. next\n * 4. last */\n";
        let expected = "const value = `// 1. literal`; /* first\n next\n * last */\n";
        for language in [Language::JavaScript, Language::TypeScript] {
            assert_eq!(
                fix_unsafe(source, language).as_deref(),
                Some(expected),
                "{language:?}"
            );
            assert!(fix_unsafe(expected, language).is_none(), "{language:?}");
        }
    }

    #[test]
    fn unicode_column_and_crlf() {
        let text = "é🐱: Step 1\r\n# Phase A\n";
        let index = LineIndex::new(text);
        assert_eq!(index.line_column(text, text.find("Step").unwrap()), (1, 5));
        assert_eq!(index.line_column(text, text.find("Phase").unwrap()), (2, 3));
    }
}
