use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::{Diagnostic, physical_lines, rules};

pub(crate) fn check(source: &str) -> Vec<Diagnostic> {
    let base = frontmatter_end(source);
    let mut diagnostics = Vec::new();
    let mut directives = Vec::new();
    let mut table_depth = 0;
    let mut code_depth = 0;
    let options = Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS;
    for (event, range) in Parser::new_ext(&source[base..], options).into_offset_iter() {
        let range = (base + range.start)..(base + range.end);
        match event {
            Event::Start(Tag::Table(_)) => table_depth += 1,
            Event::End(TagEnd::Table) => table_depth -= 1,
            Event::Start(Tag::CodeBlock(_)) => code_depth += 1,
            Event::End(TagEnd::CodeBlock) => code_depth -= 1,
            Event::Start(Tag::List(Some(_))) if table_depth == 0 && code_depth == 0 => {
                if let Some(marker) = list_marker(source, range.start) {
                    diagnostics.push(rules::ordered_list(marker.start, marker.end));
                }
            }
            Event::Text(_) if table_depth == 0 && code_depth == 0 => {
                let raw = &source[range.clone()];
                for (offset, line) in physical_lines(raw) {
                    let at = range.start + offset;
                    if is_structural_prefix(&source[line_start(source, at)..at]) {
                        rules::check_line(line, at, &mut diagnostics);
                    }
                }
            }
            Event::Html(_) | Event::InlineHtml(_) => {
                let line_start = line_start(source, range.start);
                let end = line_end(source, range.start);
                if source[line_start..end]
                    .trim()
                    .starts_with("<!-- deordinal-ignore")
                {
                    directives.push(line_start..end);
                }
            }
            _ => {}
        }
    }
    directives.sort_by_key(|range| range.start);
    directives.dedup();
    let (ranges, file_ignored, errors) = parse_directives(source, directives);
    diagnostics
        .retain(|diag| !file_ignored && !ranges.iter().any(|range| range.contains(&diag.start)));
    diagnostics.extend(errors);
    diagnostics
}

fn line_start(source: &str, at: usize) -> usize {
    source[..at].rfind('\n').map_or(0, |i| i + 1)
}

fn line_end(source: &str, at: usize) -> usize {
    source[at..].find('\n').map_or(source.len(), |i| at + i)
}

fn frontmatter_end(source: &str) -> usize {
    let first = source
        .strip_prefix('\u{feff}')
        .map_or(0, |_| '\u{feff}'.len_utf8());
    let delimiter = source[first..]
        .lines()
        .next()
        .unwrap_or("")
        .trim_end_matches('\r');
    if delimiter != "---" && delimiter != "+++" {
        return first;
    }
    let after_first = line_end(source, first);
    if after_first == source.len() {
        return first;
    }
    let mut at = after_first + 1;
    while at < source.len() {
        let end = line_end(source, at);
        if source[at..end].trim_end_matches('\r') == delimiter {
            return if end == source.len() { end } else { end + 1 };
        }
        at = end + 1;
    }
    first
}

fn list_marker(source: &str, start: usize) -> Option<Range<usize>> {
    let line = &source[start..line_end(source, start)];
    let leading = line.len() - line.trim_start_matches([' ', '\t', '>']).len();
    let at = start + leading;
    let digits = source[at..].bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 9 || !matches!(source.as_bytes().get(at + digits), Some(b'.' | b')'))
    {
        return None;
    }
    let end = at + digits + 1;
    if matches!(
        source.as_bytes().get(end),
        Some(b' ' | b'\t' | b'\r' | b'\n') | None
    ) {
        Some(at..end)
    } else {
        None
    }
}

fn is_structural_prefix(mut prefix: &str) -> bool {
    prefix = prefix.strip_prefix('\u{feff}').unwrap_or(prefix);
    loop {
        prefix = prefix.trim_start_matches([' ', '\t']);
        if prefix.is_empty() {
            return true;
        }
        if let Some(rest) = prefix.strip_prefix('>') {
            prefix = rest;
            continue;
        }
        if prefix.starts_with('#') {
            let count = prefix.bytes().take_while(|b| *b == b'#').count();
            if count <= 6 && prefix[count..].starts_with([' ', '\t']) {
                prefix = &prefix[count..];
                continue;
            }
        }
        if let Some(rest) = prefix.strip_prefix(['-', '+', '*'])
            && rest.starts_with([' ', '\t'])
        {
            prefix = rest;
            continue;
        }
        if let Some(rest) = prefix
            .strip_prefix("[ ]")
            .or_else(|| prefix.strip_prefix("[x]"))
            .or_else(|| prefix.strip_prefix("[X]"))
        {
            prefix = rest;
            continue;
        }
        let digits = prefix.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0
            && matches!(prefix.as_bytes().get(digits), Some(b'.' | b')'))
            && matches!(prefix.as_bytes().get(digits + 1), Some(b' ' | b'\t'))
        {
            prefix = &prefix[digits + 1..];
            continue;
        }
        return false;
    }
}

fn parse_directives(
    source: &str,
    directives: Vec<Range<usize>>,
) -> (Vec<Range<usize>>, bool, Vec<Diagnostic>) {
    let mut ranges = Vec::new();
    let mut errors = Vec::new();
    let mut open: Option<Range<usize>> = None;
    let mut file_ignored = false;
    let without_bom = source.strip_prefix('\u{feff}').unwrap_or(source);
    let first_content = without_bom
        .bytes()
        .position(|b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        .map(|at| at + source.len() - without_bom.len());
    for range in directives {
        let text = source[range.clone()].trim();
        let body = text
            .strip_prefix("<!-- ")
            .and_then(|s| s.strip_suffix(" -->"));
        let reason = |name: &str| {
            body.and_then(|s| s.strip_prefix(name))
                .and_then(|s| s.strip_prefix(':'))
                .map(str::trim)
        };
        if let Some(reason) = reason("deordinal-ignore-start") {
            if reason.is_empty() {
                errors.push(Diagnostic::error(
                    "ignore-start には理由が必要です",
                    range.start,
                    range.end,
                ));
            } else if open.is_some() {
                errors.push(Diagnostic::error(
                    "ignore-start を入れ子にできません",
                    range.start,
                    range.end,
                ));
            } else {
                open = Some(range);
            }
        } else if body == Some("deordinal-ignore-end") {
            if let Some(start) = open.take() {
                ranges.push(start.end..range.start);
            } else {
                errors.push(Diagnostic::error(
                    "対応する ignore-start がありません",
                    range.start,
                    range.end,
                ));
            }
        } else if let Some(reason) = reason("deordinal-ignore-file") {
            if reason.is_empty() {
                errors.push(Diagnostic::error(
                    "ignore-file には理由が必要です",
                    range.start,
                    range.end,
                ));
            } else if first_content
                != Some(range.start + source[range.start..range.end].find('<').unwrap_or(0))
            {
                errors.push(Diagnostic::error(
                    "ignore-file は文書の先頭に置いてください",
                    range.start,
                    range.end,
                ));
            } else {
                file_ignored = true;
            }
        } else {
            errors.push(Diagnostic::error(
                "不正または未対応の ignore 記法です",
                range.start,
                range.end,
            ));
        }
    }
    if let Some(start) = open {
        errors.push(Diagnostic::error(
            "ignore-start が閉じられていません",
            start.start,
            start.end,
        ));
    }
    (ranges, file_ignored, errors)
}

#[cfg(test)]
mod tests {
    use crate::{Language, LineIndex, Severity, check};

    fn hits(source: &str) -> Vec<crate::Diagnostic> {
        check(source, Language::Markdown)
    }

    fn line_column(source: &str, byte: usize) -> (usize, usize) {
        LineIndex::new(source).line_column(source, byte)
    }

    #[test]
    fn text_positions_and_structures() {
        let src = "# 1. 見出し\r\n\r\n> Step 1: 引用\r\n- [ ] 2: タスク\r\n普通の段落\n3: 続き\n";
        let ds = hits(src);
        assert_eq!(ds.len(), 4, "{ds:?}");
        assert_eq!(line_column(src, ds[0].start), (1, 3));
        assert_eq!(line_column(src, ds[1].start), (3, 3));
        assert_eq!(line_column(src, ds[2].start), (4, 7));
        assert_eq!(line_column(src, ds[3].start), (6, 1));
    }

    #[test]
    fn bom_without_frontmatter() {
        let src = "\u{feff}# Step 1\n";
        let ds = hits(src);
        assert_eq!(ds.len(), 1, "{ds:?}");
        assert_eq!(line_column(src, ds[0].start), (1, 4));
    }

    #[test]
    fn each_ordered_list_once_including_nested() {
        let src = "1. first\n2. second\n   1. nested\n   2. nested\n3. third\n";
        let ds = hits(src);
        assert_eq!(
            ds.iter()
                .filter(|d| d.rule == "deordinal/ordered-list")
                .count(),
            2,
            "{ds:?}"
        );
        assert_eq!(ds.len(), 2, "{ds:?}");
        assert_eq!(line_column(src, ds[1].start), (3, 4));
    }

    #[test]
    fn excluded_markdown_regions() {
        let src = "---\ntitle: 1. no\n---\n\n```ts\n// Step 1\n1. code\n```\n\n    2. indented code\n\n`1. no`\n**1.** 概要\n| 1. col | Step 1 |\n| --- | --- |\n| x | y |\n<!-- Step 1 -->\n<div>Step 1</div>\n";
        assert!(hits(src).is_empty(), "{:?}", hits(src));
    }

    #[test]
    fn directives_boundaries_and_errors() {
        let src = "# Step 1\n<!-- deordinal-ignore-start: 必須 -->\n1. suppressed\n<!-- deordinal-ignore-end -->\n# Phase A\n";
        let ds = hits(src);
        assert_eq!(ds.len(), 2, "{ds:?}");
        assert_eq!(line_column(src, ds[1].start).0, 5);

        let bad = "<!-- deordinal-ignore-start: -->\n<!-- deordinal-ignore-end -->\n<!-- deordinal-ignore -->\n<!-- deordinal-ignore-file: late -->\n<!-- deordinal-ignore-start: outer -->\n<!-- deordinal-ignore-start: inner -->\n";
        let errors: Vec<_> = hits(bad)
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert_eq!(errors.len(), 6, "{errors:?}");
    }

    #[test]
    fn crlf_range_never_suppresses_ignore_errors() {
        let src = "<!-- deordinal-ignore-start: 必須 -->\r\n# Step 1\r\n<!-- deordinal-ignore -->\r\n<!-- deordinal-ignore-end -->\r\n# Phase A\r\n";
        let ds = hits(src);
        assert_eq!(ds.len(), 2, "{ds:?}");
        assert_eq!(ds[0].rule, "deordinal/ignore");
        assert_eq!(line_column(src, ds[1].start), (5, 3));
    }

    #[test]
    fn file_ignore_only_at_start_with_reason() {
        let src = "\u{feff}\n<!-- deordinal-ignore-file: 手順書 -->\n# Step 1\n";
        assert!(hits(src).is_empty(), "{:?}", hits(src));
        let src = "# Step 1\n<!-- deordinal-ignore-file: 手順書 -->\n";
        assert_eq!(hits(src).len(), 2);
    }

    #[test]
    fn consecutive_directives_and_nested_lists() {
        let src = "<!-- deordinal-ignore-start: 手順 -->\n<!-- note -->\n1. first\n   1. nested\n<!-- deordinal-ignore-end -->\n# Step 1\n";
        let ds = hits(src);
        assert_eq!(ds.len(), 1, "{ds:?}");
        assert_eq!(line_column(src, ds[0].start).0, 6);
    }

    #[test]
    fn adjacent_and_multiline_directives() {
        let adjacent =
            "<!-- deordinal-ignore-start: 手順 -->\n<!-- deordinal-ignore-end -->\n# Step 1\n";
        let ds = hits(adjacent);
        assert_eq!(ds.len(), 1, "{ds:?}");
        assert_eq!(ds[0].rule, "deordinal/keyword-prefix");

        let multiline = "<!-- deordinal-ignore-start: 手順\n-->\n# Step 1\n";
        let ds = hits(multiline);
        assert_eq!(ds.len(), 2, "{ds:?}");
        assert_eq!(ds[0].rule, "deordinal/ignore");
    }

    #[test]
    fn blockquotes_and_unordered_lists() {
        let src = "> 1. first\n> 2. second\n\n- Step 1\n- 1: note\n";
        let ds = hits(src);
        assert_eq!(ds.len(), 3, "{ds:?}");
        assert_eq!(ds[0].rule, "deordinal/ordered-list");
        assert_eq!(line_column(src, ds[0].start), (1, 3));
    }

    #[test]
    fn empty_and_escaped_input() {
        assert!(hits("").is_empty());
        assert!(hits("# \\1. escaped\n**1.** 装飾付き\n").is_empty());
    }

    #[test]
    fn inline_code_is_not_a_directive() {
        let src = "`<!-- deordinal-ignore-file: reason -->`\n# Step 1\n";
        assert_eq!(hits(src).len(), 1);
    }
}
