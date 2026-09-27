use std::ops::Range;

use crate::Diagnostic;

pub(super) fn parse_directives(
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
