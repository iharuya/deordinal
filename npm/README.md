# deordinal

[![CI](https://github.com/iharuya/deordinal/actions/workflows/checks.yml/badge.svg?branch=main)](https://github.com/iharuya/deordinal/actions/workflows/checks.yml)
[![npm](https://img.shields.io/npm/v/deordinal.svg)](https://www.npmjs.com/package/deordinal)
[![crates.io](https://img.shields.io/crates/v/deordinal.svg)](https://crates.io/crates/deordinal)
[![License: MIT](https://img.shields.io/crates/l/deordinal.svg)](https://github.com/iharuya/deordinal/blob/main/LICENSE)

![Before and after: deordinal removes step numbers from JavaScript comments without changing the code](https://raw.githubusercontent.com/iharuya/deordinal/main/assets/what-is-this.png)

**Remove the numbers. Make room for change.**

When code changes, numbered comments fall out of sync. deordinal detects unnecessary ordering in Markdown and code comments and can remove supported labels without changing the code itself.

```sh
pnpm add -D deordinal
pnpm exec deordinal check
```

Requires Node.js to launch the bundled native executable. Binaries are built on macOS (arm64, x64), Ubuntu (x64), and Windows (x64). Other platforms can install from [crates.io](https://crates.io/crates/deordinal) with Rust instead.

See the [project documentation](https://github.com/iharuya/deordinal#readme) for commands and configuration.
