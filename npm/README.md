# deordinal

![Before and after: deordinal removes step numbers from JavaScript comments without changing the code](https://raw.githubusercontent.com/iharuya/deordinal/main/assets/what-is-this.png)

**Remove the numbers. Make room for change.**

When code changes, numbered comments fall out of sync. deordinal detects unnecessary ordering in Markdown and code comments and can remove supported labels without changing the code itself.

```sh
pnpm add -D deordinal
pnpm exec deordinal check
```

Requires Node.js to launch the bundled native executable. Binaries are built on macOS (arm64, x64), Ubuntu (x64), and Windows (x64). Other platforms can install from [crates.io](https://crates.io/crates/deordinal) with Rust instead.

See the [project documentation](https://github.com/iharuya/deordinal#readme) for commands and configuration.
