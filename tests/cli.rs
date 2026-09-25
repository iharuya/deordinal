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

#[test]
fn exit_codes_and_locations() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("readme.md"), "# Step 1\n").unwrap();
    let warning = run(dir.path(), &["check", "readme.md"]);
    assert_eq!(warning.status.code(), Some(1));
    assert!(stdout(&warning).contains("readme.md:1:3: deordinal/keyword-prefix:"));

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
        "<!-- deordinal-ignore-file -->\n",
    )
    .unwrap();
    let error = run(dir.path(), &["check", "readme.md"]);
    assert_eq!(error.status.code(), Some(2));
    assert!(
        stderr(&error).contains("deordinal/ignore"),
        "{}",
        stderr(&error)
    );

    fs::write(dir.path().join("readme.md"), [0xff]).unwrap();
    let utf8 = run(dir.path(), &["check", "readme.md"]);
    assert_eq!(utf8.status.code(), Some(2));
    assert!(stderr(&utf8).contains("UTF-8"));
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
    assert!(stdout(&init).contains("deordinal.jsonc"));
    assert_eq!(
        fs::read_to_string(dir.path().join("deordinal.jsonc")).unwrap(),
        "{\n  \"useGitIgnoreFile\": true\n}\n"
    );
    let after = run(dir.path(), &["check", "-v"]);
    assert_eq!(after.status.code(), before.status.code());
    assert_eq!(after.stdout, before.stdout);
    assert_eq!(after.stderr, before.stderr);

    let again = run(dir.path(), &["init"]);
    assert_eq!(again.status.code(), Some(2));
    assert!(stderr(&again).contains("上書きしません"));
    assert!(again.stdout.is_empty());
    assert_eq!(
        fs::read_to_string(dir.path().join("deordinal.jsonc")).unwrap(),
        "{\n  \"useGitIgnoreFile\": true\n}\n"
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
    assert_eq!(
        stdout(&clean),
        "a.md: 検査済み (警告 0 件、エラー 0 件)\n検査結果: 1 ファイル、警告 0 件、エラー 0 件\n"
    );

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
        assert_eq!(lines[0], "a.md: 検査済み (警告 0 件、エラー 0 件)");
        assert!(lines[1].starts_with("b.ts:1:4: deordinal/keyword-prefix:"));
        assert_eq!(lines[2], "b.ts: 検査済み (警告 1 件、エラー 0 件)");
        assert_eq!(lines[3], "検査結果: 2 ファイル、警告 1 件、エラー 0 件");
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
    assert!(out.contains("broken.md: 検査済み (警告 0 件、エラー 1 件)"));
    assert!(out.contains("clean.md: 検査済み (警告 0 件、エラー 0 件)"));
    assert!(!out.contains("unreadable.py: 検査済み"));
    assert!(out.ends_with("検査結果: 2 ファイル、警告 0 件、エラー 2 件\n"));
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
    assert_eq!(out.lines().count(), 3, "{out}");
    assert!(out.find("a.py").unwrap() < out.find("z.md").unwrap());
    assert!(out.find("z.md:1:").unwrap() < out.find("z.md:2:").unwrap());

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
        out.contains("scripts/run.py:1:3: deordinal/keyword-prefix"),
        "{out}"
    );
    assert!(
        out.contains("src/app.md:1:3: deordinal/keyword-prefix"),
        "{out}"
    );
    assert!(!out.contains("generated.ts"), "{out}");
    assert!(!out.contains("dist/"), "{out}");
    assert!(out.ends_with("検査結果: 2 ファイル、警告 2 件、エラー 0 件\n"));

    let excluded = run(dir.path(), &["check", "src/a.generated.ts", "-v"]);
    assert_eq!(excluded.status.code(), Some(0));
    assert_eq!(
        stdout(&excluded),
        "検査結果: 0 ファイル、警告 0 件、エラー 0 件\n"
    );
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
    assert!(stdout(&output).ends_with("検査結果: 1 ファイル、警告 1 件、エラー 0 件\n"));
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
    assert!(stdout(&output).ends_with("検査結果: 3 ファイル、警告 3 件、エラー 0 件\n"));
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
    assert!(stderr(&linked_dir).contains("シンボリックリンク"));
    let init_linked_dir = run(dir.path(), &["init", "linked"]);
    assert_eq!(init_linked_dir.status.code(), Some(2));
    assert!(!dir.path().join("real/deordinal.jsonc").exists());
}
