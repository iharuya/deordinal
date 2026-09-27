use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::{Diagnostic, physical_lines, rules};

use super::{
    directives::parse_directives,
    frontmatter_end,
    leading_text::{InlineMarkup, LeadingText},
    line_end, line_start, markdown_options,
};

pub(crate) fn check(source: &str) -> Vec<Diagnostic> {
    let base = frontmatter_end(source);
    let mut scanner = Scanner::new(source);
    for (event, range) in Parser::new_ext(&source[base..], markdown_options()).into_offset_iter() {
        scanner.visit(event, (base + range.start)..(base + range.end));
    }
    scanner.finish()
}

struct Scanner<'a> {
    source: &'a str,
    diagnostics: Vec<Diagnostic>,
    directives: Vec<Range<usize>>,
    table_depth: usize,
    code_depth: usize,
    list_stack: Vec<bool>,
    inline_openers: Vec<InlineMarkup>,
    leading_text: LeadingText,
    pending_fix: Option<PendingFix>,
}

impl<'a> Scanner<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            diagnostics: Vec::new(),
            directives: Vec::new(),
            table_depth: 0,
            code_depth: 0,
            list_stack: Vec::new(),
            inline_openers: Vec::new(),
            leading_text: LeadingText::default(),
            pending_fix: None,
        }
    }

    fn visit(&mut self, event: Event<'_>, range: Range<usize>) {
        match event {
            Event::Start(Tag::Table(_)) => self.table_depth += 1,
            Event::End(TagEnd::Table) => self.table_depth -= 1,
            Event::Start(Tag::CodeBlock(_)) => self.code_depth += 1,
            Event::End(TagEnd::CodeBlock) => self.code_depth -= 1,
            Event::Start(Tag::List(number)) => {
                if number.is_some()
                    && self.table_depth == 0
                    && self.code_depth == 0
                    && let Some(marker) = list_marker(self.source, range.start)
                {
                    self.diagnostics
                        .push(rules::ordered_list(marker.start, marker.end));
                }
                self.list_stack.push(number.is_some());
            }
            Event::End(TagEnd::List(_)) => {
                self.list_stack.pop();
            }
            Event::Start(Tag::Emphasis) => {
                self.inline_openers.push(InlineMarkup {
                    opener: Some(range.start..range.start + 1),
                    strong: None,
                });
            }
            Event::Start(Tag::Strong) => {
                self.inline_openers.push(InlineMarkup {
                    opener: Some(range.start..range.start + 2),
                    strong: Some(range),
                });
            }
            Event::Start(Tag::Strikethrough) => {
                self.inline_openers.push(InlineMarkup {
                    opener: Some(range.start..range.start + 2),
                    strong: None,
                });
            }
            Event::Start(Tag::Link { .. }) => {
                self.inline_openers.push(InlineMarkup {
                    opener: self.source[range.start..]
                        .starts_with('[')
                        .then_some(range.start..range.start + 1),
                    strong: None,
                });
            }
            Event::Start(Tag::Image { .. }) => self.inline_openers.push(InlineMarkup {
                opener: None,
                strong: None,
            }),
            Event::End(
                TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Link
                | TagEnd::Image,
            ) => {
                if self
                    .pending_fix
                    .as_ref()
                    .is_some_and(|fix| fix.depth == self.inline_openers.len())
                {
                    self.pending_fix = None;
                }
                self.inline_openers.pop();
            }
            Event::Text(_) if self.table_depth == 0 && self.code_depth == 0 => {
                self.scan_text(range);
            }
            Event::Code(text) => {
                self.offer_pending_fix(range.start, &text);
                self.leading_text.block(self.source, range.start);
            }
            Event::SoftBreak | Event::HardBreak => self.pending_fix = None,
            Event::Html(_) | Event::InlineHtml(_) => {
                self.leading_text.block(self.source, range.start);
                self.record_directive(range.start);
            }
            _ => {}
        }
    }

    fn scan_text(&mut self, range: Range<usize>) {
        for (offset, line) in physical_lines(&self.source[range.clone()]) {
            let at = range.start + offset;
            self.offer_pending_fix(at, line);
            if let Some((diagnostic, candidate)) = self.leading_text.scan(
                self.source,
                at,
                line,
                &self.inline_openers,
                !self.list_stack.contains(&true),
            ) {
                self.pending_fix = candidate.map(|range| PendingFix {
                    diagnostic: self.diagnostics.len(),
                    range,
                    depth: self.inline_openers.len(),
                    line_end: line_end(self.source, at),
                });
                self.diagnostics.push(diagnostic);
            }
        }
    }

    fn offer_pending_fix(&mut self, at: usize, text: &str) {
        let Some(fix) = &self.pending_fix else {
            return;
        };
        if at >= fix.line_end {
            self.pending_fix = None;
        } else if self.inline_openers.len() >= fix.depth
            && self
                .inline_openers
                .iter()
                .all(|markup| markup.opener.is_some())
            && text.chars().any(char::is_alphanumeric)
        {
            self.diagnostics[fix.diagnostic].fix = Some(fix.range.clone());
            self.pending_fix = None;
        }
    }

    fn record_directive(&mut self, at: usize) {
        let start = line_start(self.source, at);
        let end = line_end(self.source, at);
        if self.source[start..end]
            .trim()
            .starts_with("<!-- deordinal-ignore")
        {
            self.directives.push(start..end);
        }
    }

    fn finish(mut self) -> Vec<Diagnostic> {
        self.directives.sort_by_key(|range| range.start);
        self.directives.dedup();
        let (ranges, file_ignored, errors) = parse_directives(self.source, self.directives);
        self.diagnostics.retain(|diag| {
            !file_ignored && !ranges.iter().any(|range| range.contains(&diag.start))
        });
        self.diagnostics.extend(errors);
        self.diagnostics
    }
}

struct PendingFix {
    diagnostic: usize,
    range: Range<usize>,
    depth: usize,
    line_end: usize,
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
