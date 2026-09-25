# deordinal

LLMs tend to add numbered or phased labels such as `1.`, `2.`, `Phase A`, and `Step 1` without much reason. This kind of ordering can create unnecessary maintenance costs and constraints when items are later added, removed, or rearranged.

Deordinal is a [Biome](https://github.com/biomejs/biome)-like tool that detects and warns about or removes ordering in source-code comments, documentation, configuration files.

## Detection principles

- Handle ordering labels at the beginning of text and structures that express ordering, such as numbered lists. Distinguish syntax from content in headings, quotes, bullet points, comments, and other contexts. Do not treat every in-text reference to a number as a violation.
- Consider forms such as `1. Overview`, `1: Overview`, `(1) Overview`, `① Overview`, `Step 1`, and `Phase A` as candidates for detection. Do not mistake numbers followed by units, years, or versions—such as `1.29 GB` and `2025`—for ordering labels. Do not emit duplicate warnings for the same location.
- Determine what to inspect based on the document's syntax. Do not scan code or ordinary strings as prose, nor scan embedded code in documents as prose. Do not route formats with different syntax through an existing parser merely by adding their file extensions.
- Do not guess whether ordering is semantically necessary. For ambiguous cases, prefer clearly defining the scope over increasing false positives; allow necessary ordering to be handled with explicit suppressions.

## Explicit suppressions

Allow this linter to ignore ordering in prose files such as procedures and tutorials when ordering is useful:

```md
<!-- deordinal-ignore-start: reason -->
some instructions
<!-- deordinal-ignore-end -->
```

`deordinal` considers **all ordering unnecessary in programming-language comments**. Do not support ignore directives in those comments.

## Development

- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- Keep `README.md` in English and under 100 lines for human readers. Omit details or refer to another file instead.
- `cargo install --path .` to test manually
