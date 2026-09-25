use tree_sitter::{Node, Parser};

use crate::{Diagnostic, Language, physical_lines, rules};

pub(crate) fn check(source: &str, language: Language) -> Vec<Diagnostic> {
    let grammar = match language {
        Language::JavaScript | Language::Jsx => tree_sitter_javascript::LANGUAGE.into(),
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::Markdown => unreachable!(),
    };
    let mut parser = Parser::new();
    parser
        .set_language(&grammar)
        .expect("bundled grammar must match tree-sitter");
    let tree = parser
        .parse(source, None)
        .expect("parsing without cancellation must complete");
    let mut diagnostics = Vec::new();
    visit(
        tree.root_node(),
        source,
        language == Language::Python,
        &mut diagnostics,
    );
    diagnostics
}

fn visit(node: Node<'_>, source: &str, python: bool, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "comment" {
        let start = node.start_byte();
        let raw = &source[start..node.end_byte()];
        let block = raw.starts_with("/*");
        for (offset, line) in physical_lines(raw) {
            let mut line_start = start + offset;
            let mut text = line.trim_start_matches([' ', '\t', '\r']);
            line_start += line.len() - text.len();
            let marker = if block && text.starts_with("/*") {
                "/*"
            } else if block && text.starts_with('*') {
                "*"
            } else if text.starts_with("//") {
                "//"
            } else if text.starts_with('#') {
                "#"
            } else if block {
                ""
            } else {
                continue;
            };
            line_start += marker.len();
            text = &text[marker.len()..];
            rules::check_line(text, line_start, diagnostics);
        }
        return;
    }

    if python
        && matches!(
            node.kind(),
            "module" | "class_definition" | "function_definition"
        )
        && let Some(body) = if node.kind() == "module" {
            Some(node)
        } else {
            node.child_by_field_name("body")
        }
    {
        docstring(body, source, diagnostics);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit(child, source, python, diagnostics);
    }
}

fn docstring(body: Node<'_>, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = body.walk();
    let Some(statement) = body
        .named_children(&mut cursor)
        .find(|node| node.kind() != "comment")
    else {
        return;
    };
    if statement.kind() != "expression_statement" || statement.named_child_count() != 1 {
        return;
    }
    let mut expression = statement.named_child(0).unwrap();
    while expression.kind() == "parenthesized_expression" && expression.named_child_count() == 1 {
        expression = expression.named_child(0).unwrap();
    }
    if expression.kind() != "string" {
        return;
    }
    let string = expression;
    let raw = &source[string.start_byte()..string.end_byte()];
    let prefix_len = raw.bytes().take_while(|c| c.is_ascii_alphabetic()).count();
    let prefix = &raw[..prefix_len];
    if !prefix.chars().all(|c| matches!(c, 'r' | 'R' | 'u' | 'U')) {
        return;
    }
    let literal = &raw[prefix_len..];
    let quote = if literal.starts_with("\"\"\"") {
        "\"\"\""
    } else if literal.starts_with("'''") {
        "'''"
    } else if literal.starts_with('"') {
        "\""
    } else if literal.starts_with('\'') {
        "'"
    } else {
        return;
    };
    let Some(content) = literal
        .strip_prefix(quote)
        .and_then(|s| s.strip_suffix(quote))
    else {
        return;
    };
    let start = string.start_byte() + prefix_len + quote.len();
    for (offset, line) in physical_lines(content) {
        rules::check_line(line, start + offset, diagnostics);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Language, LineIndex, check};

    #[test]
    fn javascript_and_typescript_only_comments() {
        for lang in [
            Language::JavaScript,
            Language::Jsx,
            Language::TypeScript,
            Language::Tsx,
        ] {
            let source =
                "const s = '// 1. not a comment'; // 1. note\n/* 2. block\n * Step 3: next */\n";
            let hits = check(source, lang);
            assert_eq!(hits.len(), 3, "{lang:?}: {hits:?}");
            assert_eq!(
                LineIndex::new(source).line_column(source, hits[0].start),
                (1, 37)
            );
        }
    }

    #[test]
    fn jsx_and_tsx_syntax() {
        for lang in [Language::Jsx, Language::Tsx] {
            let src = "const el = <div>{/* Step 1 */}1. text</div>; // Phase A\n";
            assert_eq!(check(src, lang).len(), 2, "{lang:?}");
        }
    }

    #[test]
    fn python_docstrings_and_comments() {
        let src = "\"\"\"Step 1\n2. module\"\"\"\nvalue = \"\"\"3. ordinary\"\"\"\n# 4. comment\nclass A:\n    # preface\n    \"\"\"5. class\"\"\"\n    def f(self):\n        '6. function'\n        x = '''7. ordinary'''; return x\n    async def g(self):\n        r\"\"\"8. async\"\"\"\n";
        let hits = check(src, Language::Python);
        assert_eq!(hits.len(), 6, "{hits:?}");
    }

    #[test]
    fn parenthesized_docstring() {
        let src = "def f():\n    (\"\"\"Step 1\"\"\")\n";
        let hits = check(src, Language::Python);
        assert_eq!(hits.len(), 1, "{hits:?}");
    }

    #[test]
    fn docstring_positions_with_crlf() {
        let src = "class A:\r\n    \"\"\"Overview\r\n    Step 1: setup\r\n    2. ready\"\"\"\r\n";
        let hits = check(src, Language::Python);
        assert_eq!(hits.len(), 2, "{hits:?}");
        assert_eq!(LineIndex::new(src).line_column(src, hits[0].start), (3, 5));
        assert_eq!(LineIndex::new(src).line_column(src, hits[1].start), (4, 5));
    }

    #[test]
    fn not_docstrings_when_not_first_or_not_literal() {
        let src = "def f():\n    x = 1\n    '1. not docstring'\n\ndef g():\n    f'2. interpolated'\n\ndef h():\n    '3. string' + '4. string'\n";
        assert!(check(src, Language::Python).is_empty());
    }

    #[test]
    fn multiline_comments_without_stars_and_ordinary_strings() {
        let src = "const value = `// 1. literal`; /* 2. first\n 3. next\n * 4. last */\n";
        let hits = check(src, Language::TypeScript);
        assert_eq!(hits.len(), 3, "{hits:?}");
    }

    #[test]
    fn markdown_ignore_comments_have_no_special_meaning_in_code() {
        let src = "// deordinal-ignore-start: reason\n// Step 1\n// deordinal-ignore-end\n";
        let hits = check(src, Language::JavaScript);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].rule, "keyword-prefix");
    }

    #[test]
    fn invalid_syntax_does_not_trigger_fallback() {
        let src = "const broken = '// 1. literal'; // 2. real\n???";
        let hits = check(src, Language::JavaScript);
        assert_eq!(hits.len(), 1, "{hits:?}");
    }
}
