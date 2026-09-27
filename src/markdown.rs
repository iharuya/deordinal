use pulldown_cmark::Options;

mod directives;
mod fixes;
mod leading_text;
mod scanner;
#[cfg(test)]
mod tests;

pub(crate) use fixes::same_structure;
pub(crate) use scanner::check;

fn markdown_options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH
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
