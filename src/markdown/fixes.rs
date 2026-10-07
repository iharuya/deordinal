use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::{Diagnostic, Fix, rules};

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
        diagnostic.fix = Some(Fix::Remove(strong.start..strong.end + whitespace));
    }
}

pub(crate) fn same_structure(before: &str, after: &str, fixes: &[Fix]) -> bool {
    if fixes.iter().any(|fix| matches!(fix, Fix::BulletItem(_))) {
        return prose_structure(before, &[]) == prose_structure(after, &bullet_items_after(fixes));
    }
    let removed_strong: Vec<_> = fixes
        .iter()
        .filter_map(|fix| {
            let Fix::Remove(edit) = fix else {
                return None;
            };
            let markup = before.get(edit.clone())?.trim_end_matches([' ', '\t']);
            let strong = edit.start..edit.start + markup.len();
            isolated_strong_label(before, &strong).map(|_| strong)
        })
        .collect();
    structure(before, &removed_strong) == structure(after, &[])
}

fn structure(source: &str, removed_strong: &[Range<usize>]) -> Vec<String> {
    syntax_events(source)
        .filter_map(|(event, range)| match event {
            Event::Start(Tag::Strong) | Event::End(TagEnd::Strong)
                if removed_strong.contains(&range) =>
            {
                None
            }
            Event::Start(Tag::List(_)) => Some("Start(List)".to_owned()),
            Event::End(TagEnd::List(_)) => Some("End(List)".to_owned()),
            event => Some(format!("{event:?}")),
        })
        .collect()
}

/// Paragraphs and list items may be rearranged as the plain text reads, but no list other than
/// those started by new items may appear, disappear, or merge.
fn prose_structure(source: &str, new_items: &[usize]) -> Vec<String> {
    syntax_events(source)
        .filter_map(|(event, range)| match event {
            Event::Start(Tag::Paragraph | Tag::Item)
            | Event::End(TagEnd::Paragraph | TagEnd::Item | TagEnd::List(_))
            | Event::SoftBreak => None,
            Event::Start(Tag::List(_)) if new_items.contains(&indent_end(source, range.start)) => {
                None
            }
            event => Some(format!("{event:?}")),
        })
        .collect()
}

/// Keeps code block text because a narrower list marker can add indentation to code in the item.
fn syntax_events(source: &str) -> impl Iterator<Item = (Event<'_>, Range<usize>)> {
    let base = frontmatter_end(source);
    let mut in_code_block = false;
    Parser::new_ext(&source[base..], markdown_options())
        .into_offset_iter()
        .filter_map(move |(event, range)| {
            match event {
                Event::Start(Tag::CodeBlock(_)) => in_code_block = true,
                Event::End(TagEnd::CodeBlock) => in_code_block = false,
                Event::Text(_) if !in_code_block => return None,
                _ => {}
            }
            Some((event, (base + range.start)..(base + range.end)))
        })
}

fn bullet_items_after(fixes: &[Fix]) -> Vec<usize> {
    let mut edits: Vec<_> = fixes
        .iter()
        .flat_map(|fix| fix.ranges().iter().map(move |range| (range, fix)))
        .collect();
    edits.sort_by_key(|(range, _)| range.start);
    let mut shift = 0isize;
    let mut items = Vec::new();
    for (range, fix) in edits {
        if matches!(fix, Fix::BulletItem(_)) {
            items.push(range.start.saturating_add_signed(shift));
        }
        shift += fix.replacement().len() as isize - range.len() as isize;
    }
    items
}

fn indent_end(source: &str, at: usize) -> usize {
    source.len() - source[at..].trim_start_matches([' ', '\t']).len()
}

pub(super) fn offer_formatted_text_fix(source: &str, diagnostic: &mut Diagnostic) {
    let rest = &source[diagnostic.end..line_end(source, diagnostic.end)];
    let whitespace = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    if Parser::new_ext(&rest[whitespace..], markdown_options())
        .any(|event| matches!(event, Event::Text(text) if text.chars().any(char::is_alphanumeric)))
    {
        diagnostic.fix = Some(Fix::Remove(diagnostic.start..diagnostic.end + whitespace));
    }
}
