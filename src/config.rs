use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use globset::{GlobBuilder, GlobMatcher};
use jsonc_parser::{ParseOptions, parse_to_serde_value};
use serde::{Deserialize, Deserializer};

pub(crate) struct Config {
    pub root: PathBuf,
    pub use_git_ignore_file: bool,
    includes: Option<Vec<Pattern>>,
}

struct Pattern {
    exclude: bool,
    matcher: GlobMatcher,
}

impl Pattern {
    fn parse(path: &Path, index: usize, pattern: &str) -> Result<Self, ConfigError> {
        let (exclude, glob) = match pattern.strip_prefix('!') {
            Some(glob) => (true, glob),
            None => (false, pattern),
        };
        let glob = glob.trim_start_matches("./");
        if glob.is_empty()
            || glob.starts_with('!')
            || Path::new(glob).is_absolute()
            || Path::new(glob)
                .components()
                .any(|part| part == Component::ParentDir)
        {
            return Err(ConfigError::new(
                path,
                format!("includes[{index}]: 無効な相対 glob: {pattern:?}"),
            ));
        }
        let matcher = GlobBuilder::new(glob)
            .literal_separator(true)
            .build()
            .map_err(|err| ConfigError::new(path, format!("includes[{index}]: {err}")))?
            .compile_matcher();
        Ok(Self { exclude, matcher })
    }

    fn matches(&self, path: &Path) -> bool {
        path.ancestors()
            .take_while(|ancestor| !ancestor.as_os_str().is_empty())
            .any(|ancestor| self.matcher.is_match(ancestor))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(rename = "$schema", default, deserialize_with = "present_value")]
    _schema: Option<String>,
    #[serde(rename = "useGitIgnoreFile", default = "git_ignore_default")]
    use_git_ignore_file: bool,
    #[serde(default, deserialize_with = "present_value")]
    includes: Option<Vec<String>>,
}

fn git_ignore_default() -> bool {
    true
}

const DEFAULT_JSONC: &str = include_str!("../assets/default.deordinal.jsonc");

pub(crate) fn init(directory: &Path) -> Result<PathBuf, ConfigError> {
    let metadata =
        fs::symlink_metadata(directory).map_err(|err| ConfigError::new(directory, err))?;
    if !metadata.is_dir() {
        return Err(ConfigError::new(
            directory,
            "既存の通常のディレクトリを指定してください",
        ));
    }
    let json = directory.join("deordinal.json");
    if exists(&json)? {
        return Err(ConfigError::new(
            &json,
            "設定ファイルが既に存在します（上書きしません）",
        ));
    }
    let jsonc = directory.join("deordinal.jsonc");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&jsonc)
        .map_err(|err| init_error(&jsonc, err))?;
    if let Err(err) = file.write_all(DEFAULT_JSONC.as_bytes()) {
        drop(file);
        let _ = fs::remove_file(&jsonc);
        return Err(ConfigError::new(&jsonc, err));
    }
    Ok(jsonc)
}

fn init_error(path: &Path, err: io::Error) -> ConfigError {
    if err.kind() == io::ErrorKind::AlreadyExists {
        ConfigError::new(path, "設定ファイルが既に存在します（上書きしません）")
    } else {
        ConfigError::new(path, err)
    }
}

fn exists(path: &Path) -> Result<bool, ConfigError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(ConfigError::new(path, err)),
    }
}

fn present_value<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug)]
pub(crate) struct ConfigError {
    pub path: PathBuf,
    pub message: String,
}

impl ConfigError {
    fn new(path: &Path, message: impl ToString) -> Self {
        Self {
            path: path.to_owned(),
            message: message.to_string(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let cwd = std::env::current_dir().map_err(|err| ConfigError::new(Path::new("."), err))?;
        Self::load_from(&cwd)
    }

    fn load_from(cwd: &Path) -> Result<Self, ConfigError> {
        let cwd = cwd
            .canonicalize()
            .map_err(|err| ConfigError::new(cwd, err))?;
        for root in cwd.ancestors() {
            let json = root.join("deordinal.json");
            let jsonc = root.join("deordinal.jsonc");
            let has_json = exists(&json)?;
            let has_jsonc = exists(&jsonc)?;
            if has_json && has_jsonc {
                return Err(ConfigError::new(
                    root,
                    "deordinal.json と deordinal.jsonc を同じ場所に置くことはできません",
                ));
            }
            if has_json {
                return Self::read(&json, false);
            }
            if has_jsonc {
                return Self::read(&jsonc, true);
            }
        }
        Ok(Self {
            root: cwd,
            use_git_ignore_file: true,
            includes: None,
        })
    }

    fn read(path: &Path, jsonc: bool) -> Result<Self, ConfigError> {
        let source = fs::read_to_string(path).map_err(|err| ConfigError::new(path, err))?;
        let options = ParseOptions {
            allow_comments: jsonc,
            allow_trailing_commas: jsonc,
            allow_loose_object_property_names: false,
            allow_missing_commas: false,
            allow_single_quoted_strings: false,
            allow_hexadecimal_numbers: false,
            allow_unary_plus_numbers: false,
        };
        let raw: RawConfig =
            parse_to_serde_value(&source, &options).map_err(|err| ConfigError::new(path, err))?;
        let includes = raw
            .includes
            .map(|patterns| {
                patterns
                    .iter()
                    .enumerate()
                    .map(|(index, pattern)| Pattern::parse(path, index, pattern))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        Ok(Self {
            root: path.parent().unwrap().to_owned(),
            use_git_ignore_file: raw.use_git_ignore_file,
            includes,
        })
    }

    pub fn includes(&self, path: &Path) -> Result<bool, ConfigError> {
        let Some(patterns) = &self.includes else {
            return Ok(true);
        };
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let absolute = parent
            .canonicalize()
            .map_err(|err| ConfigError::new(path, err))?
            .join(path.file_name().expect("checked files have a filename"));
        let Ok(relative) = absolute.strip_prefix(&self.root) else {
            return Ok(false);
        };
        let mut included = false;
        for pattern in patterns {
            if pattern.matches(relative) {
                included = !pattern.exclude;
            }
        }
        Ok(included)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn published_schema_describes_public_fields_and_defaults() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../configuration_schema.json")).unwrap();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"]["useGitIgnoreFile"]["default"], true);
        assert_eq!(schema["properties"]["includes"]["type"], "array");
    }

    #[test]
    fn ordered_globs_and_directory_exclusions() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("deordinal.jsonc"), r#"{
            // includes are evaluated in order
            "includes": ["src/**", "scripts/**", "dist/**", "coverage/**", "!**/*.generated.ts", "!dist", "!coverage", "src/keep.generated.ts"],
        }"#).unwrap();
        for name in [
            "src/main.ts",
            "src/nested/file.md",
            "src/file.generated.ts",
            "src/keep.generated.ts",
            "scripts/run.py",
            "dist/index.md",
            "coverage/report.md",
            "elsewhere.md",
        ] {
            let path = dir.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        let config = Config::load_from(dir.path()).unwrap();
        for name in [
            "src/main.ts",
            "src/nested/file.md",
            "src/keep.generated.ts",
            "scripts/run.py",
        ] {
            assert!(config.includes(&dir.path().join(name)).unwrap(), "{name}");
        }
        for name in [
            "src/file.generated.ts",
            "dist/index.md",
            "coverage/report.md",
            "elsewhere.md",
        ] {
            assert!(!config.includes(&dir.path().join(name)).unwrap(), "{name}");
        }
    }

    #[test]
    fn closest_configuration_wins() {
        let dir = tempdir().unwrap();
        let child = dir.path().join("child");
        fs::create_dir(&child).unwrap();
        let file = child.join("file.md");
        fs::write(&file, "").unwrap();
        fs::write(dir.path().join("deordinal.json"), r#"{"includes": []}"#).unwrap();
        assert!(!Config::load_from(&child).unwrap().includes(&file).unwrap());
        fs::write(child.join("deordinal.jsonc"), r#"{"includes": ["./**",],}"#).unwrap();
        assert!(Config::load_from(&child).unwrap().includes(&file).unwrap());
    }

    #[test]
    fn missing_empty_and_only_negative_includes() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("file.md");
        fs::write(&file, "").unwrap();
        assert!(
            Config::load_from(dir.path())
                .unwrap()
                .includes(&file)
                .unwrap()
        );
        for (value, expected) in [
            ("[]", false),
            ("[\"!**/*.md\"]", false),
            ("[\"**\", \"!**/*.md\", \"file.md\"]", true),
        ] {
            fs::write(
                dir.path().join("deordinal.json"),
                format!("{{\"includes\": {value}}}"),
            )
            .unwrap();
            assert_eq!(
                Config::load_from(dir.path())
                    .unwrap()
                    .includes(&file)
                    .unwrap(),
                expected
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn explicit_file_symlink_is_matched_by_its_path_not_its_target() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let external = tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(external.path().join("outside.md"), "").unwrap();
        symlink(
            external.path().join("outside.md"),
            dir.path().join("src/alias.md"),
        )
        .unwrap();
        fs::write(
            dir.path().join("deordinal.json"),
            r#"{"includes": ["src/**"]}"#,
        )
        .unwrap();
        let config = Config::load_from(dir.path()).unwrap();
        assert!(config.includes(&dir.path().join("src/alias.md")).unwrap());
        assert!(
            !config
                .includes(&external.path().join("outside.md"))
                .unwrap()
        );
    }

    #[test]
    fn rejects_bad_config_without_falling_back() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("deordinal.json");
        for text in [
            r#"{ "includes": ["["] }"#,
            r#"{ "includes": ["../other/**"] }"#,
            r#"{ "includes": ["!!dist"] }"#,
            r#"{ "includes": [""] }"#,
            r#"{ "unknown": true }"#,
            r#"{ "useGitIgnoreFile": "yes" }"#,
            r#"{ "includes": ["**",] }"#,
            r#"{ "includes": null }"#,
            r#"{ "$schema": null }"#,
            r#"{ "includes": ["**"], "includes": [] }"#,
            r#"{ // not JSON
        "includes": ["**"] }"#,
        ] {
            fs::write(&path, text).unwrap();
            let err = Config::load_from(dir.path()).err().expect(text);
            assert_eq!(err.path, path.canonicalize().unwrap());
        }
        fs::write(dir.path().join("deordinal.jsonc"), "{includes: ['**']}").unwrap();
        fs::remove_file(&path).unwrap();
        assert!(Config::load_from(dir.path()).is_err());
        fs::write(&path, "{}").unwrap();
        assert!(
            Config::load_from(dir.path())
                .err()
                .unwrap()
                .message
                .contains("同じ場所")
        );
    }
}
