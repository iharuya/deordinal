use std::{
    collections::BTreeSet,
    fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use deordinal::{Language, LineIndex, Severity, check, fix_unsafe};
use ignore::WalkBuilder;
use tempfile::NamedTempFile;

mod config;
use config::Config;

#[derive(Parser)]
#[command(version, about = "Detect ordering labels in prose and code comments")]
struct Cli {
    #[arg(short, long, global = true, help = "Show checked files and counts")]
    verbose: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Check {
        #[arg(value_name = "PATH", default_value = ".")]
        paths: Vec<PathBuf>,
        #[arg(long, help = "Write supported fixes (requires --unsafe)")]
        write: bool,
        #[arg(long = "unsafe", help = "Allow unsafe fixes (requires --write)")]
        unsafe_fixes: bool,
    },
    Init {
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct ErrorReport {
    path: PathBuf,
    line: usize,
    column: usize,
    message: String,
}

impl ErrorReport {
    fn path(path: &Path, message: impl ToString) -> Self {
        Self {
            path: path.to_owned(),
            line: 0,
            column: 0,
            message: message.to_string(),
        }
    }

    fn display(&self) -> String {
        if self.line == 0 {
            format!("{}: {}", self.path.display(), self.message)
        } else {
            format!(
                "{}:{}:{}: {}",
                self.path.display(),
                self.line,
                self.column,
                self.message
            )
        }
    }
}

fn normalize(path: &Path) -> PathBuf {
    let result: PathBuf = path
        .components()
        .filter(|part| *part != Component::CurDir)
        .collect();
    if result.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        result
    }
}

fn main() -> ExitCode {
    let Cli { verbose, command } = Cli::parse();
    match command {
        Command::Check {
            paths,
            write,
            unsafe_fixes,
        } => {
            if write != unsafe_fixes {
                eprintln!("Use --write and --unsafe together");
                ExitCode::from(2)
            } else {
                run_check(&paths, verbose, write)
            }
        }
        Command::Init { path } => run_init(&path),
    }
}

fn run_init(path: &Path) -> ExitCode {
    match config::init(path) {
        Ok(created) => {
            println!("Created {}", normalize(&created).display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{}: {}", err.path.display(), err.message);
            ExitCode::from(2)
        }
    }
}

fn discover(paths: &[PathBuf], config: &Config) -> (BTreeSet<PathBuf>, Vec<ErrorReport>) {
    let mut files = BTreeSet::new();
    let mut errors = Vec::new();
    for path in paths {
        match fs::metadata(path) {
            Ok(meta) if meta.is_file() => {
                files.insert(normalize(path));
            }
            Ok(meta) if meta.is_dir() => {
                if fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
                    errors.push(ErrorReport::path(path, "Skipping directory symlink"));
                    continue;
                }
                let mut walk = WalkBuilder::new(path);
                walk.follow_links(false).require_git(false);
                walk.git_ignore(config.use_git_ignore_file)
                    .git_exclude(config.use_git_ignore_file)
                    .git_global(config.use_git_ignore_file)
                    .ignore(config.use_git_ignore_file);
                for entry in walk.build() {
                    match entry {
                        Ok(entry) if entry.file_type().is_some_and(|ty| ty.is_file()) => {
                            files.insert(normalize(entry.path()));
                        }
                        Ok(_) => {}
                        Err(err) => errors.push(ErrorReport::path(path, err)),
                    }
                }
            }
            Ok(_) => errors.push(ErrorReport::path(path, "Expected a file or directory")),
            Err(err) => errors.push(ErrorReport::path(path, err)),
        }
    }
    (files, errors)
}

fn write_fixed(path: &Path, contents: &str) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Refusing to write to symlinks or non-files",
        ));
    }
    let mut temp = NamedTempFile::new_in(path.parent().unwrap_or(Path::new(".")))?;
    temp.as_file().set_permissions(metadata.permissions())?;
    temp.write_all(contents.as_bytes())?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn run_check(paths: &[PathBuf], verbose: bool, write: bool) -> ExitCode {
    let config = match Config::load() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{}: {}", err.path.display(), err.message);
            return ExitCode::from(2);
        }
    };
    let (files, mut errors) = discover(paths, &config);
    let mut checked_files = 0;
    let mut warnings = 0;
    for path in files {
        let Some(language) = Language::from_path(&path) else {
            continue;
        };
        match config.includes(&path) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(err) => {
                errors.push(ErrorReport::path(&err.path, err.message));
                continue;
            }
        }
        let source = match fs::read(&path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(source) => source,
                Err(err) => {
                    errors.push(ErrorReport::path(&path, format!("Invalid UTF-8: {err}")));
                    continue;
                }
            },
            Err(err) => {
                errors.push(ErrorReport::path(&path, err));
                continue;
            }
        };
        checked_files += 1;
        let mut diagnostics = check(&source, language);
        let mut written = None;
        if write
            && !diagnostics.iter().any(|d| d.severity == Severity::Error)
            && let Some(fixed) = fix_unsafe(&source, language)
        {
            match write_fixed(&path, &fixed) {
                Ok(()) => {
                    diagnostics = check(&fixed, language);
                    written = Some(fixed);
                    println!("✔ Applied fixes to {}", path.display());
                }
                Err(err) => errors.push(ErrorReport::path(&path, err)),
            }
        }
        let checked_source = written.as_deref().unwrap_or(&source);
        let lines = LineIndex::new(checked_source);
        let mut file_warnings = 0;
        let mut file_errors = 0;
        for diagnostic in diagnostics {
            let (line, column) = lines.line_column(checked_source, diagnostic.start);
            let message = format!("{}: {}", diagnostic.rule, diagnostic.message);
            if diagnostic.severity == Severity::Error {
                file_errors += 1;
                errors.push(ErrorReport {
                    path: path.clone(),
                    line,
                    column,
                    message,
                });
            } else {
                file_warnings += 1;
                println!("✖ {}:{line}:{column}: {message}", path.display());
            }
        }
        warnings += file_warnings;
        if verbose {
            println!(
                "{}: checked ({file_warnings} warnings, {file_errors} errors)",
                path.display()
            );
        }
    }
    if verbose {
        println!(
            "Summary: {checked_files} files, {warnings} warnings, {} errors",
            errors.len()
        );
    }
    errors.sort();
    for error in &errors {
        if error.line == 0 {
            eprintln!("{}", error.display());
        } else {
            eprintln!("✖ {}", error.display());
        }
    }
    if !errors.is_empty() {
        ExitCode::from(2)
    } else if warnings > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
