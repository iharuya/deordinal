# deordinal

A CLI that best-effort detects and removes ordering labels in prose and code comments.

## Installation

In JS/TS project: `pnpm add -D deordinal`

Global install: `cargo install deordinal --locked`

## CLI usage

```sh
cd to_project
deordinal init                 # create a configuration file
deordinal check                # recursively check the current directory
deordinal check README.md src  # check the specified files and directories
```

Autofix is experimental: rerun the same `check` command with `--write --unsafe` to apply supported edits.
Review the diff before committing; unsupported diagnostics remain unchanged.

Supported files: `.md`, `.js` / `.jsx` / `.mjs` / `.cjs`, `.ts` / `.tsx` / `.mts` / `.cts`, `.py` / `.pyi`

## Default rules

- `ordered-list`: Detects numbered Markdown lists, treating each list separately. Nested lists are separate lists.
- `prefix`: Detects labels at the start of prose, such as `1. Overview`, `1: Overview`, `A) Overview`, `(1) Overview`, and `① Overview`.
- `keyword-prefix`: Detects labels at the start of prose, such as `Step 1` and `Phase A`. Headings containing only a label are also checked.

Some expressions that look like numbers with units, years, or versions are excluded, but the tool does not determine whether ordering is necessary based on meaning. References within sentences are not checked.

## Configuration

Specify target files with `glob` patterns; use `!` to exclude files.

See the [JSON Schema](./configuration_schema.json) for details.

## Markdown ignores

For ordering that is necessary, use a reasoned range or file ignore directive in a standalone HTML comment.

Range ignore:
````md
<!-- deordinal-ignore-start: These instructions must be followed in order. -->
1. Stop the service
2. Create a backup
<!-- deordinal-ignore-end -->
````

Ignore the entire file:
````md
<!-- deordinal-ignore-file: This is an ordered operations manual. -->
````

Ordering in code comments is not needed, so ignore directives are not supported in them.
