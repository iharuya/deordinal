use std::ops::Range;

use crate::{Diagnostic, rules};

use super::{
    fixes::{isolated_strong_label, offer_formatted_text_fix, offer_isolated_strong_fix},
    line_start,
};

#[derive(Default)]
pub(super) struct LeadingText {
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

pub(super) struct InlineMarkup {
    pub(super) opener: Option<Range<usize>>,
    pub(super) strong: Option<Range<usize>>,
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

    pub(super) fn block(&mut self, source: &str, at: usize) {
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

    pub(super) fn scan(
        &mut self,
        source: &str,
        at: usize,
        text: &str,
        inline_openers: &[InlineMarkup],
        allow_fix: bool,
    ) -> Option<(Diagnostic, Option<Range<usize>>)> {
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
        let mut pending_fix = None;
        if allow_fix && diagnostic.fix.is_none() && source.get(raw_label) == self.text.get(label) {
            if inline_openers.is_empty() {
                offer_formatted_text_fix(source, &mut diagnostic);
            } else if inline_openers.len() == 1
                && let Some(strong) = &inline_openers[0].strong
                && isolated_strong_label(source, strong) == Some(diagnostic.start..diagnostic.end)
            {
                offer_isolated_strong_fix(source, strong, &mut diagnostic);
            } else if source[diagnostic.end..at + text.len()]
                .bytes()
                .all(|byte| matches!(byte, b' ' | b'\t'))
            {
                pending_fix = Some(diagnostic.start..at + text.len());
            }
        }
        Some((diagnostic, pending_fix))
    }
}

fn is_leading_prefix(source: &str, at: usize, inline_openers: &[InlineMarkup]) -> bool {
    let line = line_start(source, at);
    let mut cursor = line;
    for markup in inline_openers {
        let Some(opener) = &markup.opener else {
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
