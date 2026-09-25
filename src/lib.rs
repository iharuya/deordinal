mod code;
mod markdown;
mod rules;

use std::path::Path;

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
}

impl Diagnostic {
    fn warning(rule: &'static str, message: &str, start: usize, end: usize) -> Self {
        Self {
            rule,
            message: message.into(),
            start,
            end,
            severity: Severity::Warning,
        }
    }

    fn error(message: &str, start: usize, end: usize) -> Self {
        Self {
            rule: "deordinal/ignore",
            message: message.into(),
            start,
            end,
            severity: Severity::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Markdown,
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
        _ => code::check(source, language),
    };
    diagnostics.sort_by(|a, b| a.start.cmp(&b.start).then(a.rule.cmp(b.rule)));
    diagnostics
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
    fn unicode_column_and_crlf() {
        let text = "あ🐱: Step 1\r\n# Phase A\n";
        let index = LineIndex::new(text);
        assert_eq!(index.line_column(text, text.find("Step").unwrap()), (1, 5));
        assert_eq!(index.line_column(text, text.find("Phase").unwrap()), (2, 3));
    }
}
