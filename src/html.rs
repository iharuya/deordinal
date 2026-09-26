use tree_sitter::{Node, Parser};

use crate::{Diagnostic, physical_lines, rules};

fn parse(source: &str) -> tree_sitter::Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_html::LANGUAGE.into())
        .expect("bundled grammar must match tree-sitter");
    parser
        .parse(source, None)
        .expect("parsing without cancellation must complete")
}

pub(crate) fn check(source: &str) -> Vec<Diagnostic> {
    let tree = parse(source);
    let mut diagnostics = Vec::new();
    visit(tree.root_node(), source, &mut diagnostics);
    diagnostics
}

fn visit(node: Node<'_>, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "comment" {
        let start = node.start_byte();
        let raw = &source[start..node.end_byte()];
        if let Some(content) = raw.strip_prefix("<!--").and_then(|s| s.strip_suffix("-->")) {
            for (offset, line) in physical_lines(content) {
                let text = line.trim_start_matches([' ', '\t']);
                let leading = line.len() - text.len();
                let (text, marker) = if let Some(rest) = text.strip_prefix('*') {
                    (rest, 1)
                } else {
                    (text, 0)
                };
                rules::check_line(
                    text,
                    start + 4 + offset + leading + marker,
                    diagnostics,
                    true,
                );
            }
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit(child, source, diagnostics);
    }
}

pub(crate) fn same_structure(before: &str, after: &str) -> bool {
    fn structure(source: &str) -> Vec<(String, Option<String>)> {
        fn visit(node: Node<'_>, source: &str, result: &mut Vec<(String, Option<String>)>) {
            let kind = node.kind().to_owned();
            if kind == "comment" {
                result.push((kind, None));
                return;
            }
            let text = (node.child_count() == 0)
                .then(|| source[node.start_byte()..node.end_byte()].to_owned());
            result.push((kind, text));
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                visit(child, source, result);
            }
        }
        let tree = parse(source);
        let mut result = Vec::new();
        visit(tree.root_node(), source, &mut result);
        result
    }
    structure(before) == structure(after)
}

#[cfg(test)]
mod tests {
    use crate::{Language, LineIndex, check, fix_unsafe};

    fn hits(source: &str) -> Vec<crate::Diagnostic> {
        check(source, Language::Html)
    }

    #[test]
    fn only_html_comments_are_inspected() {
        let source = "<!doctype html>\n<h1>Step 1: title</h1>\n<!-- Step 1: setup -->\n<script>const x = '<!-- Step 2: literal -->'; // Step 3: code</script>\n<style>/* Phase A: style */</style>\n<p title=\"Step 4: attr\">&lt;!-- Step 5: escaped --&gt;</p>\n";
        let diagnostics = hits(source);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(
            LineIndex::new(source).line_column(source, diagnostics[0].start),
            (3, 6)
        );
    }

    #[test]
    fn multiline_comments_and_locations() {
        let source = "<!--\r\n  1. Prepare\r\n  * Phase A: Execute\r\n  * 2. Verify\r\n-->\r\n<!-- ① Finish -->\r\n";
        let diagnostics = hits(source);
        assert_eq!(diagnostics.len(), 4, "{diagnostics:?}");
        let index = LineIndex::new(source);
        assert_eq!(
            diagnostics
                .iter()
                .map(|d| index.line_column(source, d.start))
                .collect::<Vec<_>>(),
            [(2, 3), (3, 5), (4, 5), (6, 6)]
        );
    }

    #[test]
    fn fixes_only_comment_content() {
        let source = "<!-- Step 1: setup -->\r\n<!--\r\n * 1. Prepare\r\n * Phase A: Execute\r\n-->\r\n<h1>Step 3: title</h1>\r\n";
        let expected = "<!-- setup -->\r\n<!--\r\n * Prepare\r\n * Execute\r\n-->\r\n<h1>Step 3: title</h1>\r\n";
        assert_eq!(
            fix_unsafe(source, Language::Html).as_deref(),
            Some(expected)
        );
        assert!(fix_unsafe(expected, Language::Html).is_none());
    }

    #[test]
    fn ignore_text_is_not_a_directive() {
        let source = "<!-- deordinal-ignore-file: reason -->\n<!-- deordinal-ignore-start: reason -->\n<!-- Step 1: setup -->\n<!-- deordinal-ignore-end -->\n<!-- Step 2 -->\n";
        assert_eq!(hits(source).len(), 2);
        assert_eq!(
            fix_unsafe(source, Language::Html).as_deref(),
            Some(
                "<!-- deordinal-ignore-file: reason -->\n<!-- deordinal-ignore-start: reason -->\n<!-- setup -->\n<!-- deordinal-ignore-end -->\n<!-- Step 2 -->\n"
            )
        );
    }

    #[test]
    fn malformed_and_escaped_comments_are_not_scanned_as_prose() {
        let source = "<pre>&lt;!-- Step 1: escaped --&gt;</pre>\n<!-- Step 2: unterminated";
        assert!(hits(source).is_empty());
        assert!(fix_unsafe(source, Language::Html).is_none());
    }

    #[test]
    fn structure_check_protects_markup_and_other_text() {
        assert!(super::same_structure(
            "<p>Original</p><!-- Step 1: setup -->",
            "<p>Original</p><!-- setup -->"
        ));
        assert!(!super::same_structure(
            "<p>Original</p><!-- Step 1: setup -->",
            "<p>Changed</p><!-- setup -->"
        ));
        assert!(!super::same_structure(
            "<p>Original</p><!-- Step 1: setup -->",
            "<div>Original</div><!-- setup -->"
        ));
    }
}
