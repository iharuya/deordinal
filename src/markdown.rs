use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::{Diagnostic, physical_lines, rules};

pub(crate) fn check(source: &str) -> Vec<Diagnostic> {
    let base = frontmatter_end(source);
    let mut diagnostics = Vec::new();
    let mut directives = Vec::new();
    let mut table_depth = 0;
    let mut code_depth = 0;
    let mut list_stack = Vec::new();
    let mut inline_openers = Vec::new();
    let mut leading_text = LeadingText::default();
    for (event, range) in Parser::new_ext(&source[base..], markdown_options()).into_offset_iter() {
        let range = (base + range.start)..(base + range.end);
        match event {
            Event::Start(Tag::Table(_)) => table_depth += 1,
            Event::End(TagEnd::Table) => table_depth -= 1,
            Event::Start(Tag::CodeBlock(_)) => code_depth += 1,
            Event::End(TagEnd::CodeBlock) => code_depth -= 1,
            Event::Start(Tag::List(number)) => {
                if number.is_some()
                    && table_depth == 0
                    && code_depth == 0
                    && let Some(marker) = list_marker(source, range.start)
                {
                    diagnostics.push(rules::ordered_list(marker.start, marker.end));
                }
                list_stack.push(number.is_some());
            }
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
            }
            Event::Start(Tag::Emphasis) => {
                inline_openers.push(Some(range.start..range.start + 1));
            }
            Event::Start(Tag::Strong | Tag::Strikethrough) => {
                inline_openers.push(Some(range.start..range.start + 2));
            }
            Event::Start(Tag::Link { .. }) => {
                inline_openers.push(
                    source[range.start..]
                        .starts_with('[')
                        .then_some(range.start..range.start + 1),
                );
            }
            Event::Start(Tag::Image { .. }) => inline_openers.push(None),
            Event::End(
                TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Link
                | TagEnd::Image,
            ) => {
                inline_openers.pop();
            }
            Event::Text(_) if table_depth == 0 && code_depth == 0 => {
                for (offset, line) in physical_lines(&source[range.clone()]) {
                    if let Some(diagnostic) = leading_text.scan(
                        source,
                        range.start + offset,
                        line,
                        &inline_openers,
                        !list_stack.contains(&true),
                    ) {
                        diagnostics.push(diagnostic);
                    }
                }
            }
            Event::Code(_) => leading_text.block(source, range.start),
            Event::Html(_) | Event::InlineHtml(_) => {
                leading_text.block(source, range.start);
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

fn markdown_options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH
}

#[derive(Default)]
struct LeadingText {
    line: Option<usize>,
    text: String,
    fragments: Vec<TextFragment>,
    blocked: bool,
    reported: bool,
}

struct TextFragment {
    logical: Range<usize>,
    source: Range<usize>,
}

impl LeadingText {
    fn on_line(&mut self, source: &str, at: usize) {
        let line = line_start(source, at);
        if self.line != Some(line) {
            *self = Self {
                line: Some(line),
                ..Self::default()
            };
        }
    }

    fn block(&mut self, source: &str, at: usize) {
        self.on_line(source, at);
        self.blocked = true;
    }

    fn source_range(&self, range: Range<usize>) -> Range<usize> {
        let first = self
            .fragments
            .iter()
            .find(|fragment| fragment.logical.contains(&range.start))
            .expect("label starts in a text fragment");
        let last = self
            .fragments
            .iter()
            .find(|fragment| fragment.logical.contains(&(range.end - 1)))
            .expect("label ends in a text fragment");
        (first.source.start + range.start - first.logical.start)
            ..(last.source.start + range.end - last.logical.start)
    }

    fn scan(
        &mut self,
        source: &str,
        at: usize,
        text: &str,
        inline_openers: &[Option<Range<usize>>],
        allow_fix: bool,
    ) -> Option<Diagnostic> {
        self.on_line(source, at);
        if self.blocked || self.reported || text.is_empty() {
            return None;
        }
        if self.fragments.is_empty() && !is_leading_prefix(source, at, inline_openers) {
            self.blocked = true;
            return None;
        }

        let start = self.text.len();
        self.text.push_str(text);
        self.fragments.push(TextFragment {
            logical: start..self.text.len(),
            source: at..at + text.len(),
        });

        let mut found = Vec::new();
        rules::check_line(&self.text, 0, &mut found, allow_fix);
        let mut diagnostic = found.pop()?;
        self.reported = true;
        let label = diagnostic.start..diagnostic.end;
        let raw_label = self.source_range(label.clone());
        if let Some(fix) = diagnostic.fix.take() {
            let raw_fix = self.source_range(fix.clone());
            if source.get(raw_fix.clone()) == self.text.get(fix) {
                diagnostic.fix = Some(raw_fix);
            }
        }
        diagnostic.start = raw_label.start;
        diagnostic.end = raw_label.end;
        if allow_fix
            && inline_openers.is_empty()
            && diagnostic.fix.is_none()
            && source.get(raw_label) == self.text.get(label)
        {
            offer_formatted_text_fix(source, &mut diagnostic);
        }
        Some(diagnostic)
    }
}

fn is_leading_prefix(source: &str, at: usize, inline_openers: &[Option<Range<usize>>]) -> bool {
    let line = line_start(source, at);
    let mut cursor = line;
    for opener in inline_openers {
        let Some(opener) = opener else {
            return false;
        };
        if opener.start < line {
            continue;
        }
        if opener.start < cursor
            || opener.end > at
            || !is_structural_prefix(&source[cursor..opener.start])
        {
            return false;
        }
        cursor = opener.end;
    }
    is_structural_prefix(&source[cursor..at])
}

pub(crate) fn same_structure(before: &str, after: &str) -> bool {
    fn structure(source: &str) -> Vec<String> {
        let base = frontmatter_end(source);
        Parser::new_ext(&source[base..], markdown_options())
            .filter(|event| !matches!(event, Event::Text(_)))
            .map(|event| format!("{event:?}"))
            .collect()
    }
    structure(before) == structure(after)
}

fn offer_formatted_text_fix(source: &str, diagnostic: &mut Diagnostic) {
    let rest = &source[diagnostic.end..line_end(source, diagnostic.end)];
    let whitespace = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    if Parser::new_ext(&rest[whitespace..], markdown_options())
        .any(|event| matches!(event, Event::Text(text) if text.chars().any(char::is_alphanumeric)))
    {
        diagnostic.fix = Some(diagnostic.start..diagnostic.end + whitespace);
    }
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
                    "ignore-start requires a reason",
                    range.start,
                    range.end,
                ));
            } else if open.is_some() {
                errors.push(Diagnostic::error(
                    "ignore-start cannot be nested",
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
                    "ignore-end has no matching ignore-start",
                    range.start,
                    range.end,
                ));
            }
        } else if let Some(reason) = reason("deordinal-ignore-file") {
            if reason.is_empty() {
                errors.push(Diagnostic::error(
                    "ignore-file requires a reason",
                    range.start,
                    range.end,
                ));
            } else if first_content
                != Some(range.start + source[range.start..range.end].find('<').unwrap_or(0))
            {
                errors.push(Diagnostic::error(
                    "ignore-file must be at the start of the document",
                    range.start,
                    range.end,
                ));
            } else {
                file_ignored = true;
            }
        } else {
            errors.push(Diagnostic::error(
                "Invalid or unsupported ignore directive",
                range.start,
                range.end,
            ));
        }
    }
    if let Some(start) = open {
        errors.push(Diagnostic::error(
            "ignore-start is not closed",
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
        let src = "# 1. Heading\r\n\r\n> Step 1: Quote\r\n- [ ] 2: Task\r\nPlain paragraph\n3: Continue\n";
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
            ds.iter().filter(|d| d.rule == "ordered-list").count(),
            2,
            "{ds:?}"
        );
        assert_eq!(ds.len(), 2, "{ds:?}");
        assert_eq!(line_column(src, ds[1].start), (3, 4));
    }

    #[test]
    fn excluded_markdown_regions() {
        let src = "---\ntitle: 1. no\n---\n\n```ts\n// Step 1\n1. code\n```\n\n    2. indented code\n\n`1. no`\n| 1. col | Step 1 |\n| --- | --- |\n| x | y |\n<!-- Step 1 -->\n<div>Step 1</div>\n";
        assert!(hits(src).is_empty(), "{:?}", hits(src));
    }

    #[test]
    fn directives_boundaries_and_errors() {
        let src = "# Step 1\n<!-- deordinal-ignore-start: required -->\n1. suppressed\n<!-- deordinal-ignore-end -->\n# Phase A\n";
        let ds = hits(src);
        assert_eq!(ds.len(), 2, "{ds:?}");
        assert_eq!(line_column(src, ds[1].start).0, 5);

        let bad = "<!-- deordinal-ignore-start: -->\n<!-- deordinal-ignore-end -->\n<!-- deordinal-ignore -->\n<!-- deordinal-ignore-file: late -->\n<!-- deordinal-ignore-start: outer -->\n<!-- deordinal-ignore-start: inner -->\n";
        let errors: Vec<_> = hits(bad)
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert_eq!(errors.len(), 6, "{errors:?}");
        assert_eq!(
            errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>(),
            [
                "ignore-start requires a reason",
                "ignore-end has no matching ignore-start",
                "Invalid or unsupported ignore directive",
                "ignore-file must be at the start of the document",
                "ignore-start is not closed",
                "ignore-start cannot be nested",
            ]
        );
    }

    #[test]
    fn crlf_range_never_suppresses_ignore_errors() {
        let src = "<!-- deordinal-ignore-start: required -->\r\n# Step 1\r\n<!-- deordinal-ignore -->\r\n<!-- deordinal-ignore-end -->\r\n# Phase A\r\n";
        let ds = hits(src);
        assert_eq!(ds.len(), 2, "{ds:?}");
        assert_eq!(ds[0].rule, "ignore");
        assert_eq!(line_column(src, ds[1].start), (5, 3));
    }

    #[test]
    fn file_ignore_only_at_start_with_reason() {
        let src = "\u{feff}\n<!-- deordinal-ignore-file: manual -->\n# Step 1\n";
        assert!(hits(src).is_empty(), "{:?}", hits(src));
        let src = "# Step 1\n<!-- deordinal-ignore-file: manual -->\n";
        assert_eq!(hits(src).len(), 2);
    }

    #[test]
    fn consecutive_directives_and_nested_lists() {
        let src = "<!-- deordinal-ignore-start: procedure -->\n<!-- note -->\n1. first\n   1. nested\n<!-- deordinal-ignore-end -->\n# Step 1\n";
        let ds = hits(src);
        assert_eq!(ds.len(), 1, "{ds:?}");
        assert_eq!(line_column(src, ds[0].start).0, 6);
    }

    #[test]
    fn adjacent_and_multiline_directives() {
        let adjacent =
            "<!-- deordinal-ignore-start: procedure -->\n<!-- deordinal-ignore-end -->\n# Step 1\n";
        let ds = hits(adjacent);
        assert_eq!(ds.len(), 1, "{ds:?}");
        assert_eq!(ds[0].rule, "keyword-prefix");

        let multiline = "<!-- deordinal-ignore-start: procedure\n-->\n# Step 1\n";
        let ds = hits(multiline);
        assert_eq!(ds.len(), 2, "{ds:?}");
        assert_eq!(ds[0].rule, "ignore");
    }

    #[test]
    fn blockquotes_and_unordered_lists() {
        let src = "> 1. first\n> 2. second\n\n- Step 1\n- 1: note\n";
        let ds = hits(src);
        assert_eq!(ds.len(), 3, "{ds:?}");
        assert_eq!(ds[0].rule, "ordered-list");
        assert_eq!(line_column(src, ds[0].start), (1, 3));
    }

    #[test]
    fn empty_and_escaped_input() {
        assert!(hits("").is_empty());
        assert!(hits("# \\1. escaped\n").is_empty());
    }

    #[test]
    fn leading_labels_inside_inline_markup() {
        let src = "# **Step 1: Setup**\n**1. Overview**\n**1.** Overview\n*Step 1: Setup*\n__Phase A: Plan__\n- **2. Item**\n> **Phase A: Plan**\n[Step 1: Setup](https://example.com)\n[**1.** Overview](url)\n***Step 3***\n~~Phase B~~\n**Step** 1: Install\n**1**. Overview\n";
        let ds = hits(src);
        assert_eq!(ds.len(), 13, "{ds:?}");
        for (diagnostic, line) in ds.iter().zip(1..) {
            assert_eq!(line_column(src, diagnostic.start).0, line, "{diagnostic:?}");
        }
        assert_eq!(line_column(src, ds[0].start), (1, 5));
        assert_eq!(line_column(src, ds[1].start), (2, 3));
        assert_eq!(line_column(src, ds[7].start), (8, 2));
        assert_eq!(&src[ds[11].start..ds[11].end], "Step** 1:");
        assert_eq!(&src[ds[12].start..ds[12].end], "1**.");
    }

    #[test]
    fn formatted_quantity_is_not_an_ordering_label() {
        let src = "**0.934 → 0.744**\n**8.4 And then a section**\n";
        let ds = hits(src);
        assert_eq!(ds.len(), 1, "{ds:?}");
        assert_eq!(line_column(src, ds[0].start), (2, 3));
    }

    #[test]
    fn inline_markup_does_not_turn_later_text_into_a_label() {
        let src = "Plain **Step 1**\n`code` **Step 1**\n![Step 1](image.png)\n![alt](image.png) **Step 1**\n\\*Step 1*\n<span>Step 1</span>\n**plain** Step 1\n| label | value |\n| --- | --- |\n| **Step 1** | x |\n```md\n**Step 1**\n```\n";
        assert!(hits(src).is_empty(), "{:?}", hits(src));
    }

    #[test]
    fn inline_code_is_not_a_directive() {
        let src = "`<!-- deordinal-ignore-file: reason -->`\n# Step 1\n";
        assert_eq!(hits(src).len(), 1);
    }
}
