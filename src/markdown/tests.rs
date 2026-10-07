use crate::{Language, LineIndex, Severity, check, fix_unsafe};

fn hits(source: &str) -> Vec<crate::Diagnostic> {
    check(source, Language::Markdown)
}

fn fixed(source: &str) -> Option<String> {
    fix_unsafe(source, Language::Markdown)
}

fn line_column(source: &str, byte: usize) -> (usize, usize) {
    LineIndex::new(source).line_column(source, byte)
}

#[test]
fn text_positions_and_structures() {
    let src =
        "# 1. Heading\r\n\r\n> Step 1: Quote\r\n- [ ] 2: Task\r\nPlain paragraph\n3: Continue\n";
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
fn units_after_decimal_numbers_are_not_ordering_labels() {
    let src = "2.5 seconds elapsed\n2.5 秒経過\n**2.5 seconds.**\n# **2.5 秒経過**\n# 1.29 Features\n**4.4 コーパスを検査する**\n";
    let ds = hits(src);
    assert_eq!(ds.len(), 2, "{ds:?}");
    assert_eq!(line_column(src, ds[0].start), (5, 3));
    assert_eq!(line_column(src, ds[1].start), (6, 3));
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

#[test]
fn ordered_lists_become_bullet_lists() {
    assert_eq!(
        fixed("1. hello\n2. world\n").as_deref(),
        Some("- hello\n- world\n")
    );
    let src = "1. first\r\n   1. nested\r\n2. second\r\n\r\n> 1) quote\r\n> 2) more\r\n\r\n10. ten\r\n11. eleven\r\n";
    let expected = "- first\r\n   - nested\r\n- second\r\n\r\n> - quote\r\n> - more\r\n\r\n- ten\r\n- eleven\r\n";
    assert_eq!(fixed(src).as_deref(), Some(expected));
    assert!(hits(expected).is_empty(), "{:?}", hits(expected));
}

#[test]
fn letter_markers_and_nested_ordered_lists_follow_the_plain_text() {
    let src = "a. hello\nb. nice\n  1. to\n  2. meet\nc. you\n";
    let expected = "- hello\n- nice\n  - to\n  - meet\n- you\n";
    assert_eq!(fixed(src).as_deref(), Some(expected));
    assert!(hits(expected).is_empty(), "{:?}", hits(expected));
}

#[test]
fn ordinal_labels_starting_prose_lines_become_list_items() {
    let src = "\u{feff}Intro\n(1) first\n② second\nA) third\n1: fourth\n2. fifth\n\n> 一、 quoted\n\n- item\n  a. nested\n\n> quoted\n(1) lazy\n";
    let expected = "\u{feff}Intro\n- first\n- second\n- third\n- fourth\n- fifth\n\n> - quoted\n\n- item\n  - nested\n\n> quoted\n- lazy\n";
    assert_eq!(fixed(src).as_deref(), Some(expected));
}

#[test]
fn other_labels_are_still_removed() {
    let src = "# (1) Heading\n\n(2) Setext\n---\n\n- (3) item\n\n**4.** Bold\n\n*5. emphasized*\n\nStep 1: keyword\n";
    let expected = "# Heading\n\nSetext\n---\n\n- item\n\nBold\n\n*emphasized*\n\nkeyword\n";
    assert_eq!(fixed(src).as_deref(), Some(expected));
}

#[test]
fn labels_inside_ordered_lists_are_fixed_once_the_list_is_bulleted() {
    let src = "1. Step 1: Install\n2. (2) Run\n";
    assert_eq!(fixed(src).as_deref(), Some("- Install\n- Run\n"));
}

#[test]
fn list_fixes_that_change_other_syntax_are_skipped() {
    for src in [
        "- bullet\n1. would merge into the bullet list\n",
        "1. item\n\n       code that the narrower marker would indent\n",
        "(1) [ ] would become a task\n",
        "(1) 2. would become a nested list\n",
        "1. a\n   <!-- deordinal-ignore-start: ordered -->\n2. b\n   <!-- deordinal-ignore-end -->\n",
    ] {
        assert!(fixed(src).is_none(), "{src}");
        assert!(!hits(src).is_empty(), "{src}");
    }
    assert_eq!(
        fixed("1. dot\n1) parenthesis\n").as_deref(),
        Some("1. dot\n- parenthesis\n")
    );
}
