use std::path::{Path, PathBuf};

use deordinal::{Diagnostic, LineIndex, Severity};

#[derive(Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct ErrorReport {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl ErrorReport {
    pub fn path(path: &Path, message: impl ToString) -> Self {
        Self {
            path: path.to_owned(),
            line: 0,
            column: 0,
            message: message.to_string(),
        }
    }

    pub fn display(&self) -> String {
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

struct WarningGroup {
    rule: &'static str,
    message: String,
    locations: Vec<(usize, usize)>,
}

pub(crate) struct FileReport {
    warnings: Vec<WarningGroup>,
    errors: Vec<ErrorReport>,
}

impl FileReport {
    pub fn new(path: &Path, source: &str, diagnostics: Vec<Diagnostic>) -> Self {
        let index = LineIndex::new(source);
        let mut report = Self {
            warnings: Vec::<WarningGroup>::new(),
            errors: Vec::new(),
        };
        for diagnostic in diagnostics {
            let (line, column) = index.line_column(source, diagnostic.start);
            match diagnostic.severity {
                Severity::Error => report.errors.push(ErrorReport {
                    path: path.to_owned(),
                    line,
                    column,
                    message: format!("{}: {}", diagnostic.rule, diagnostic.message),
                }),
                Severity::Warning => {
                    let group = report.warnings.iter_mut().find(|group| {
                        group.rule == diagnostic.rule && group.message == diagnostic.message
                    });
                    if let Some(group) = group {
                        if group.locations.last() != Some(&(line, column)) {
                            group.locations.push((line, column));
                        }
                    } else {
                        report.warnings.push(WarningGroup {
                            rule: diagnostic.rule,
                            message: diagnostic.message,
                            locations: vec![(line, column)],
                        });
                    }
                }
            }
        }
        report
    }

    pub fn warning_count(&self) -> usize {
        self.warnings
            .iter()
            .map(|group| group.locations.len())
            .sum()
    }

    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    pub fn print_warnings(&self, path: &Path) {
        for group in &self.warnings {
            let (line, column) = group.locations[0];
            if group.locations.len() == 1 {
                println!(
                    "✖ {}:{line}:{column}: {}: {}",
                    path.display(),
                    group.rule,
                    group.message
                );
            } else {
                println!(
                    "✖ {}:{line}:{column}: {}: {} ({} occurrences)",
                    path.display(),
                    group.rule,
                    group.message,
                    group.locations.len()
                );
                for chunk in group.locations[1..].chunks(8) {
                    let locations = chunk
                        .iter()
                        .map(|(line, column)| format!("{line}:{column}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    println!("  also at {locations}");
                }
            }
        }
    }

    pub fn into_errors(self) -> Vec<ErrorReport> {
        self.errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deordinal::{Language, check};

    #[test]
    fn repeated_locations_are_counted_once_without_hiding_distinct_ones() {
        let source = "# 1. First\n# 2. Second\n";
        let mut diagnostics = check(source, Language::Markdown);
        diagnostics.insert(1, diagnostics[0].clone());
        let report = FileReport::new(Path::new("example.md"), source, diagnostics);
        assert_eq!(report.warning_count(), 2);
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.warnings[0].locations, [(1, 3), (2, 3)]);
    }
}
