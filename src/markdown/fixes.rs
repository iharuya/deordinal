use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::{Diagnostic, rules};

use super::{frontmatter_end, line_end, markdown_options};

pub(super) fn isolated_strong_label(source: &str, strong: &Range<usize>) -> Option<Range<usize>> {
    let marker = source.get(strong.start..strong.start + 2)?;
    if !matches!(marker, "**" | "__")
        || source.get(strong.end.checked_sub(2)?..strong.end)? != marker
    {
        return None;
    }
    let inner = (strong.start + 2)..(strong.end - 2);
    let mut labels = Vec::new();
    rules::check_line(source.get(inner.clone())?, inner.start, &mut labels, false);
    let label = labels.first()?;
    if source[inner.start..label.start]
        .trim_matches([' ', '\t'])
        .is_empty()
        && source[label.end..inner.end]
            .trim_matches([' ', '\t'])
            .is_empty()
    {
        Some(label.start..label.end)
    } else {
        None
    }
}

pub(super) fn offer_isolated_strong_fix(
    source: &str,
    strong: &Range<usize>,
    diagnostic: &mut Diagnostic,
) {
    let end = line_end(source, strong.end);
    let rest = &source[strong.end..end];
    let whitespace = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    if Parser::new_ext(&rest[whitespace..], markdown_options())
        .any(|event| matches!(event, Event::Text(text) if text.chars().any(char::is_alphanumeric)))
    {
        diagnostic.fix = Some(strong.start..strong.end + whitespace);
    }
}

pub(crate) fn same_structure(before: &str, after: &str, edits: &[Range<usize>]) -> bool {
    fn structure(source: &str, removed_strong: &[Range<usize>]) -> Vec<String> {
        let base = frontmatter_end(source);
        Parser::new_ext(&source[base..], markdown_options())
            .into_offset_iter()
            .filter(|(event, range)| {
                if matches!(event, Event::Text(_)) {
                    return false;
                }
                !matches!(
                    event,
                    Event::Start(Tag::Strong) | Event::End(TagEnd::Strong)
                ) || !removed_strong.contains(&((base + range.start)..(base + range.end)))
            })
            .map(|(event, _)| format!("{event:?}"))
            .collect()
    }

    let removed_strong: Vec<_> = edits
        .iter()
        .filter_map(|edit| {
            let deleted = before.get(edit.clone())?;
            let markup = deleted.trim_end_matches([' ', '\t']);
            let strong = edit.start..edit.start + markup.len();
            isolated_strong_label(before, &strong).map(|_| strong)
        })
        .collect();
    structure(before, &removed_strong) == structure(after, &[])
}

pub(super) fn offer_formatted_text_fix(source: &str, diagnostic: &mut Diagnostic) {
    let rest = &source[diagnostic.end..line_end(source, diagnostic.end)];
    let whitespace = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    if Parser::new_ext(&rest[whitespace..], markdown_options())
        .any(|event| matches!(event, Event::Text(text) if text.chars().any(char::is_alphanumeric)))
    {
        diagnostic.fix = Some(diagnostic.start..diagnostic.end + whitespace);
    }
}
