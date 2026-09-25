use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use tempfile::tempdir;

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_deordinal"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn assert_counts(line: &str, warnings: usize, errors: usize) {
    assert!(line.contains(&format!("{warnings} warnings")), "{line}");
    assert!(line.contains(&format!("{errors} errors")), "{line}");
}

fn assert_summary(line: &str, files: usize, warnings: usize, errors: usize) {
    assert!(line.contains(&format!("{files} files")), "{line}");
    assert_counts(line, warnings, errors);
}

#[test]
fn exit_codes_and_locations() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("readme.md"), "# Step 1\n").unwrap();
    let warning = run(dir.path(), &["check", "readme.md"]);
    assert_eq!(warning.status.code(), Some(1));
    assert_eq!(
        stdout(&warning),
        "✖ readme.md:1:3: keyword-prefix: Leading phase label\n"
    );

    fs::write(dir.path().join("readme.md"), "# タイトル\n").unwrap();
    let clean = run(dir.path(), &["check", "readme.md"]);
    assert_eq!(clean.status.code(), Some(0));
    assert!(clean.stdout.is_empty());
    assert_eq!(
        run(dir.path(), &["check", "missing.md"]).status.code(),
        Some(2)
    );
    assert_eq!(
        run(dir.path(), &["invalid-argument"]).status.code(),
        Some(2)
    );

    fs::write(
        dir.path().join("readme.md"),
        "<!-- deordinal-ignore-file: -->\n",
    )
    .unwrap();
    let error = run(dir.path(), &["check", "readme.md"]);
    assert_eq!(error.status.code(), Some(2));
    assert_eq!(
        stderr(&error),
        "✖ readme.md:1:1: ignore: ignore-file requires a reason\n"
    );

    fs::write(dir.path().join("readme.md"), [0xff]).unwrap();
    let utf8 = run(dir.path(), &["check", "readme.md"]);
    assert_eq!(utf8.status.code(), Some(2));
    assert!(stderr(&utf8).contains("UTF-8"));
}

#[test]
fn diagnostic_rule_names_and_locations_match_the_cli_output() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("guide.md"),
        "# 1. 概要\n\n1. first\n2. second\n",
    )
    .unwrap();
    let output = run(dir.path(), &["check", "guide.md"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stdout(&output),
        "✖ guide.md:1:3: prefix: Leading ordering label\n✖ guide.md:3:1: ordered-list: Ordered list used\nHint: Re-run this check with `--write --unsafe` to apply supported fixes (review the diff).\n"
    );
}

#[test]
fn groups_repeated_warnings_without_losing_locations_or_counts() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("repeated.md"),
        "# 1. Start\n# (1) Continue\n# ① End\n# Step 1\n1. first\n2. second\n",
    )
    .unwrap();
    fs::write(dir.path().join("second.md"), "# 1. Else\n").unwrap();

    let output = run(dir.path(), &["check", "repeated.md", "second.md", "-v"]);
    assert_eq!(output.status.code(), Some(1));
    let out = stdout(&output);
    assert!(out.contains("✖ repeated.md:1:3: prefix: Leading ordering label (3 occurrences)\n  also at 2:3, 3:3\n"), "{out}");
    assert!(out.contains("✖ repeated.md:4:3: keyword-prefix: Leading phase label\n"));
    assert!(out.contains("✖ repeated.md:5:1: ordered-list: Ordered list used\n"));
    assert!(out.contains("✖ second.md:1:3: prefix: Leading ordering label\n"));
    assert_eq!(out.matches("Hint:").count(), 1);
    assert_summary(out.lines().last().unwrap(), 2, 6, 0);
}

#[test]
fn fix_hint_requires_an_applicable_edit() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("unsupported.md"),
        "# Step 1\n# Phase A\n1. first\n2. second\n",
    )
    .unwrap();
    let unsupported = run(dir.path(), &["check", "unsupported.md"]);
    assert_eq!(unsupported.status.code(), Some(1));
    let out = stdout(&unsupported);
    assert!(
        out.contains("keyword-prefix: Leading phase label (2 occurrences)\n  also at 2:3\n"),
        "{out}"
    );
    assert!(!out.contains("Hint:"));

    fs::write(dir.path().join("blocked.md"), "1: ---\n").unwrap();
    let blocked = run(dir.path(), &["check", "blocked.md"]);
    assert_eq!(blocked.status.code(), Some(1));
    assert!(!stdout(&blocked).contains("Hint:"));

    fs::write(
        dir.path().join("invalid.md"),
        "# Step 1: Title\n<!-- deordinal-ignore -->\n",
    )
    .unwrap();
    let invalid = run(dir.path(), &["check", "invalid.md"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(!stdout(&invalid).contains("Hint:"));
}

#[test]
fn init_creates_config_without_changing_default_check_behavior() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("guide.md"), "# Step 1\n").unwrap();
    fs::write(dir.path().join(".gitignore"), "ignored.md\n").unwrap();
    fs::write(dir.path().join("ignored.md"), "# Step 2\n").unwrap();

    let before = run(dir.path(), &["check", "-v"]);
    assert_eq!(before.status.code(), Some(1));
    let init = run(dir.path(), &["init"]);
    assert_eq!(init.status.code(), Some(0), "{}", stderr(&init));
    assert_eq!(stdout(&init), "Created deordinal.jsonc\n");
    let generated = fs::read_to_string(dir.path().join("deordinal.jsonc")).unwrap();
    assert_eq!(generated, include_str!("../assets/default.deordinal.jsonc"));
    let json: serde_json::Value = serde_json::from_str(&generated).unwrap();
    assert_eq!(
        json["$schema"],
        "https://raw.githubusercontent.com/iharuya/deordinal/main/configuration_schema.json"
    );
    assert_eq!(json["useGitIgnoreFile"], true);
    assert!(json.get("includes").is_none());
    let after = run(dir.path(), &["check", "-v"]);
    assert_eq!(after.status.code(), before.status.code());
    assert_eq!(after.stdout, before.stdout);
    assert_eq!(after.stderr, before.stderr);

    let again = run(dir.path(), &["init"]);
    assert_eq!(again.status.code(), Some(2));
    assert!(!again.stderr.is_empty());
    assert!(again.stdout.is_empty());
    assert_eq!(
        fs::read_to_string(dir.path().join("deordinal.jsonc")).unwrap(),
        generated
    );
}

#[test]
fn init_uses_an_existing_target_directory_and_never_overwrites_json() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join("project")).unwrap();
    fs::write(dir.path().join("deordinal.json"), "{ invalid }").unwrap();
    let init = run(dir.path(), &["init", "project"]);
    assert_eq!(init.status.code(), Some(0), "{}", stderr(&init));
    assert!(stdout(&init).contains("project/deordinal.jsonc"));
    fs::write(dir.path().join("project/example.md"), "# Step 1\n").unwrap();
    assert_eq!(
        run(&dir.path().join("project"), &["check"]).status.code(),
        Some(1)
    );

    let blocked = run(dir.path(), &["init"]);
    assert_eq!(blocked.status.code(), Some(2));
    assert!(stderr(&blocked).contains("deordinal.json"));
    assert!(!dir.path().join("deordinal.jsonc").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("deordinal.json")).unwrap(),
        "{ invalid }"
    );

    for path in ["nonexistent", "deordinal.json"] {
        let output = run(dir.path(), &["init", path]);
        assert_eq!(output.status.code(), Some(2), "{path}");
        assert!(!dir.path().join(path).join("deordinal.jsonc").exists());
    }
}

#[test]
fn verbose_lists_checked_files_and_summary_without_changing_exit_code() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "# タイトル\n").unwrap();
    fs::write(dir.path().join("b.ts"), "// Step 1\n").unwrap();
    fs::write(dir.path().join("c.mdx"), "# Step 2\n").unwrap();

    let clean = run(dir.path(), &["check", "a.md", "-v"]);
    assert_eq!(clean.status.code(), Some(0));
    let clean_out = stdout(&clean);
    let clean_lines: Vec<_> = clean_out.lines().collect();
    assert_eq!(clean_lines.len(), 2, "{clean_out}");
    assert!(clean_lines[0].starts_with("a.md:"));
    assert_counts(clean_lines[0], 0, 0);
    assert_summary(clean_lines[1], 1, 0, 0);

    let quiet = run(dir.path(), &["check"]);
    assert_eq!(quiet.status.code(), Some(1));
    assert_eq!(stdout(&quiet).lines().count(), 1);

    for args in [&["check", "-v"][..], &["--verbose", "check"][..]] {
        let output = run(dir.path(), args);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(stderr(&output), "");
        let out = stdout(&output);
        let lines: Vec<_> = out.lines().collect();
        assert_eq!(lines.len(), 4, "{out}");
        assert!(lines[0].starts_with("a.md:"));
        assert_counts(lines[0], 0, 0);
        assert!(lines[1].starts_with("✖ b.ts:1:4: keyword-prefix:"));
        assert!(lines[2].starts_with("b.ts:"));
        assert_counts(lines[2], 1, 0);
        assert_summary(lines[3], 2, 1, 0);
    }
}

#[test]
fn verbose_counts_errors_without_marking_unreadable_files_as_checked() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("broken.md"),
        "<!-- deordinal-ignore-file -->\n",
    )
    .unwrap();
    fs::write(dir.path().join("clean.md"), "# 概要\n").unwrap();
    fs::write(dir.path().join("unreadable.py"), [0xff]).unwrap();
    let output = run(dir.path(), &["check", "--verbose"]);
    assert_eq!(output.status.code(), Some(2));
    let out = stdout(&output);
    let lines: Vec<_> = out.lines().collect();
    assert_eq!(lines.len(), 3, "{out}");
    assert!(lines[0].starts_with("broken.md:"));
    assert_counts(lines[0], 0, 1);
    assert!(lines[1].starts_with("clean.md:"));
    assert_counts(lines[1], 0, 0);
    assert!(!out.contains("unreadable.py:"));
    assert_summary(lines[2], 2, 0, 2);
    assert_eq!(stderr(&output).lines().count(), 2);
}

#[test]
fn discovery_gitignore_explicit_files_and_stable_order() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join(".gitignore"), "ignored.md\n").unwrap();
    fs::write(dir.path().join("z.md"), "# Step 1\n# Phase A\n").unwrap();
    fs::write(dir.path().join("a.py"), "# 1. real\n").unwrap();
    fs::write(dir.path().join("ignored.md"), "# Step 2\n").unwrap();
    fs::write(dir.path().join(".hidden.md"), "# Step 2\n").unwrap();
    fs::write(dir.path().join("image.mdx"), "# Step 3\n").unwrap();
    let output = run(dir.path(), &["check"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let out = stdout(&output);
    assert_eq!(out.lines().count(), 4, "{out}");
    assert!(out.find("a.py").unwrap() < out.find("z.md").unwrap());
    assert!(out.contains("z.md:1:3: keyword-prefix: Leading phase label (2 occurrences)"));
    assert!(out.contains("also at 2:3"));

    let explicit = run(dir.path(), &["check", "ignored.md"]);
    assert_eq!(explicit.status.code(), Some(1));
    assert!(stdout(&explicit).contains("ignored.md:1:3:"));

    let hidden = run(dir.path(), &["check", ".hidden.md"]);
    assert_eq!(hidden.status.code(), Some(1));
    assert!(stdout(&hidden).contains(".hidden.md:1:3:"));
}

#[test]
fn all_supported_extensions_and_non_target_files() {
    let dir = tempdir().unwrap();
    for ext in [
        "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts", "py", "pyi",
    ] {
        let content = if ext == "pyi" {
            "\"\"\"Step 1\"\"\"\n"
        } else if ext == "py" {
            "# Step 1\n"
        } else {
            "// Step 1\n"
        };
        fs::write(dir.path().join(format!("a.{ext}")), content).unwrap();
    }
    fs::write(dir.path().join("a.md"), "# Step 1\n").unwrap();
    fs::write(dir.path().join("a.mdx"), "# Step 1\n").unwrap();
    let output = run(dir.path(), &["check"]);
    assert_eq!(stdout(&output).lines().count(), 11, "{}", stdout(&output));
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn errors_are_ordered_by_numeric_line_and_duplicates_are_ignored() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("bad.md"),
        format!(
            "\n<!-- deordinal-ignore -->\n{}<!-- deordinal-ignore -->\n",
            "\n".repeat(7)
        ),
    )
    .unwrap();
    let output = run(dir.path(), &["check", ".", "bad.md"]);
    assert_eq!(output.status.code(), Some(2));
    let errors = stderr(&output);
    assert_eq!(errors.lines().count(), 2, "{errors}");
    assert!(errors.find("bad.md:2:").unwrap() < errors.find("bad.md:10:").unwrap());
}

#[test]
fn jsonc_config_filters_explicit_and_discovered_files() {
    let dir = tempdir().unwrap();
    for (name, contents) in [
        ("src/app.md", "# Step 1\n"),
        ("src/a.generated.ts", "// Step 2\n"),
        ("scripts/run.py", "# Step 3\n"),
        ("dist/output.md", "# Step 4\n"),
        ("coverage/report.md", "# Step 5\n"),
        ("other.md", "# Step 6\n"),
    ] {
        let path = dir.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    fs::write(
        dir.path().join("deordinal.jsonc"),
        r#"{
        // Order matters: later matches override earlier ones.
        "$schema": "./configuration_schema.json",
        "includes": [
            "src/**", "scripts/**", "!**/*.generated.ts", "!dist", "!coverage",
        ],
    }"#,
    )
    .unwrap();
    let output = run(dir.path(), &["check", "-v"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let out = stdout(&output);
    assert!(
        out.contains("✖ scripts/run.py:1:3: keyword-prefix"),
        "{out}"
    );
    assert!(out.contains("✖ src/app.md:1:3: keyword-prefix"), "{out}");
    assert!(!out.contains("generated.ts"), "{out}");
    assert!(!out.contains("dist/"), "{out}");
    let summary = out.lines().last().unwrap();
    assert_summary(summary, 2, 2, 0);

    let excluded = run(dir.path(), &["check", "src/a.generated.ts", "-v"]);
    assert_eq!(excluded.status.code(), Some(0));
    let excluded_out = stdout(&excluded);
    let summary = excluded_out.trim_end();
    assert_eq!(summary.lines().count(), 1, "{excluded_out}");
    assert_summary(summary, 0, 0, 0);
}

#[test]
fn config_globs_are_relative_to_config_not_current_directory() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join("nested")).unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("deordinal.json"),
        r#"{"includes": ["src/**"]}"#,
    )
    .unwrap();
    fs::write(dir.path().join("src/entry.md"), "# Step 1\n").unwrap();
    fs::write(dir.path().join("nested/other.md"), "# Step 2\n").unwrap();
    let output = run(&dir.path().join("nested"), &["check", "..", "-v"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stdout(&output).contains("../src/entry.md:1:3:"));
    let out = stdout(&output);
    let summary = out.lines().last().unwrap();
    assert_summary(summary, 1, 1, 0);
}

#[test]
fn git_ignore_option_preserves_the_default_and_can_disable_ignore_files() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join(".gitignore"), "ignored.md\n").unwrap();
    fs::write(dir.path().join(".ignore"), "ignored-by-dotignore.md\n").unwrap();
    fs::write(dir.path().join("ignored.md"), "# Step 1\n").unwrap();
    fs::write(dir.path().join("ignored-by-dotignore.md"), "# Step 3\n").unwrap();
    fs::create_dir(dir.path().join("nested")).unwrap();
    fs::write(dir.path().join("nested/.gitignore"), "ignored.md\n").unwrap();
    fs::write(dir.path().join("nested/ignored.md"), "# Step 2\n").unwrap();
    assert_eq!(run(dir.path(), &["check"]).status.code(), Some(0));
    fs::write(
        dir.path().join("deordinal.json"),
        "{\"useGitIgnoreFile\": true}",
    )
    .unwrap();
    assert_eq!(run(dir.path(), &["check"]).status.code(), Some(0));
    assert_eq!(
        run(dir.path(), &["check", "ignored.md"]).status.code(),
        Some(1)
    );
    fs::write(
        dir.path().join("deordinal.json"),
        "{\"useGitIgnoreFile\": false}",
    )
    .unwrap();
    let output = run(dir.path(), &["check", "-v"]);
    assert_eq!(output.status.code(), Some(1));
    let out = stdout(&output);
    let summary = out.lines().last().unwrap();
    assert_summary(summary, 3, 3, 0);
}

#[test]
fn invalid_configuration_stops_before_checking_files() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "# Step 1\n").unwrap();
    for (name, contents) in [
        ("deordinal.json", "{\"includes\": [\"[\"]}"),
        ("deordinal.json", "{\"unknown\": true}"),
        ("deordinal.json", "{\"includes\": [\"**\",]}"),
    ] {
        fs::write(dir.path().join(name), contents).unwrap();
        let output = run(dir.path(), &["check", "-v"]);
        assert_eq!(output.status.code(), Some(2), "{contents}");
        assert!(stdout(&output).is_empty());
        assert!(stderr(&output).contains(name));
    }
    fs::write(dir.path().join("deordinal.json"), "{}").unwrap();
    fs::write(dir.path().join("deordinal.jsonc"), "{}").unwrap();
    let output = run(dir.path(), &["check"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("deordinal.jsonc"));
}

#[test]
fn write_requires_explicit_unsafe_opt_in_and_never_writes_on_argument_error() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("guide.md");
    fs::write(&path, "# Step 1: Setup\n").unwrap();
    for args in [
        &["check", "--write", "guide.md"][..],
        &["check", "--unsafe", "guide.md"][..],
    ] {
        let output = run(dir.path(), args);
        assert_eq!(output.status.code(), Some(2));
        assert!(stdout(&output).is_empty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "# Step 1: Setup\n");
    }
    assert_eq!(
        run(dir.path(), &["check", "guide.md"]).status.code(),
        Some(1)
    );
}

#[test]
fn write_rechecks_files_and_reports_only_remaining_warnings() {
    let dir = tempdir().unwrap();
    let markdown = dir.path().join("guide.md");
    let code = dir.path().join("main.js");
    fs::write(&markdown, "# Step 1: Setup\n1. first\n2. second\n").unwrap();
    fs::write(&code, "// 1. init\n// someCode()\n// 2. run\n").unwrap();
    let output = run(dir.path(), &["check", "--write", "--unsafe", "-v"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_eq!(
        fs::read_to_string(&markdown).unwrap(),
        "# Setup\n1. first\n2. second\n"
    );
    assert_eq!(
        fs::read_to_string(&code).unwrap(),
        "// init\n// someCode()\n// run\n"
    );
    assert_eq!(stdout(&output).matches("✖ ").count(), 1);
    assert_eq!(stdout(&output).matches("Applied fixes").count(), 2);
    assert!(!stdout(&output).contains("Hint:"));
    assert!(stdout(&output).contains("guide.md:2:1: ordered-list"));
    assert_summary(stdout(&output).lines().last().unwrap(), 2, 1, 0);
    let again = run(dir.path(), &["check", "--write", "--unsafe"]);
    assert_eq!(again.status.code(), Some(1));
    assert_eq!(stdout(&again).lines().count(), 1);
    assert!(!stdout(&again).contains("Applied fixes"));
}

#[test]
fn write_respects_ignore_and_does_not_edit_files_with_ignore_errors() {
    let dir = tempdir().unwrap();
    let ignored = "<!-- deordinal-ignore-start: required -->\n# Step 1: ignored\n<!-- deordinal-ignore-end -->\n# Step 2: edit\n";
    fs::write(dir.path().join("ignored.md"), ignored).unwrap();
    let invalid = "# Step 1: unchanged\n<!-- deordinal-ignore -->\n";
    fs::write(dir.path().join("invalid.md"), invalid).unwrap();
    let output = run(dir.path(), &["check", "--write", "--unsafe"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        fs::read_to_string(dir.path().join("ignored.md")).unwrap(),
        ignored.replace("# Step 2: edit", "# edit")
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("invalid.md")).unwrap(),
        invalid
    );
}

#[test]
fn write_respects_configuration_for_explicit_paths() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("deordinal.json"),
        r#"{"includes":["src/**"]}"#,
    )
    .unwrap();
    let included = dir.path().join("src/inside.md");
    let excluded = dir.path().join("outside.md");
    fs::write(&included, "# Step 1: Inside\n").unwrap();
    fs::write(&excluded, "# Step 1: Outside\n").unwrap();
    let output = run(
        dir.path(),
        &[
            "check",
            "--write",
            "--unsafe",
            "src/inside.md",
            "outside.md",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(fs::read_to_string(included).unwrap(), "# Inside\n");
    assert_eq!(fs::read_to_string(excluded).unwrap(), "# Step 1: Outside\n");
}

#[cfg(unix)]
#[test]
fn check_does_not_suggest_writing_through_a_symlink() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let real = dir.path().join("real.md");
    fs::write(&real, "# Step 1: Title\n").unwrap();
    symlink(&real, dir.path().join("alias.md")).unwrap();
    let output = run(dir.path(), &["check", "alias.md"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("alias.md:1:3: keyword-prefix"));
    assert!(!stdout(&output).contains("Hint:"));
}

#[cfg(unix)]
#[test]
fn write_rejects_explicit_file_symlinks_and_preserves_permissions() {
    use std::os::unix::{fs::PermissionsExt, fs::symlink};

    let dir = tempdir().unwrap();
    let real = dir.path().join("real.md");
    fs::write(&real, "# Step 1: Title\n").unwrap();
    fs::set_permissions(&real, fs::Permissions::from_mode(0o755)).unwrap();
    symlink(&real, dir.path().join("alias.md")).unwrap();
    let rejected = run(dir.path(), &["check", "--write", "--unsafe", "alias.md"]);
    assert_eq!(rejected.status.code(), Some(2));
    assert!(stderr(&rejected).contains("symlinks"));
    assert_eq!(fs::read_to_string(&real).unwrap(), "# Step 1: Title\n");

    let written = run(dir.path(), &["check", "--write", "--unsafe", "real.md"]);
    assert_eq!(written.status.code(), Some(0));
    assert_eq!(fs::read_to_string(&real).unwrap(), "# Title\n");
    assert_eq!(
        fs::metadata(&real).unwrap().permissions().mode() & 0o777,
        0o755
    );
}

#[cfg(unix)]
#[test]
fn directory_symlinks_are_not_followed_but_explicit_file_symlinks_are_read() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join("real")).unwrap();
    fs::write(dir.path().join("real/a.md"), "# Step 1\n").unwrap();
    symlink(dir.path().join("real"), dir.path().join("linked")).unwrap();
    symlink(dir.path().join("real/a.md"), dir.path().join("alias.md")).unwrap();
    let found = stdout(&run(dir.path(), &["check"]));
    assert_eq!(found.lines().count(), 1, "{found}");
    assert!(!found.contains("linked"));
    assert!(!found.contains("alias.md"));

    let explicit = run(dir.path(), &["check", "alias.md"]);
    assert_eq!(explicit.status.code(), Some(1));
    assert!(stdout(&explicit).contains("alias.md:"));

    let linked_dir = run(dir.path(), &["check", "linked"]);
    assert_eq!(linked_dir.status.code(), Some(2));
    assert!(stderr(&linked_dir).contains("symlink"));
    let init_linked_dir = run(dir.path(), &["init", "linked"]);
    assert_eq!(init_linked_dir.status.code(), Some(2));
    assert!(!dir.path().join("real/deordinal.jsonc").exists());
}
