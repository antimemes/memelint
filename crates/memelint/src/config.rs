//! The project configuration: `memelint.toml` at the project root.
//!
//! ```toml
//! words = ["gantry", "sprocket"]
//! words-file = "more-words.txt"
//! ignore = ["vendor/", "*.generated.go"]
//! ```

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::check::Allowlist;

pub const CONFIG_FILE: &str = "memelint.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    /// Technical names and verbs that the project accepts.
    #[serde(default)]
    pub words: Vec<String>,
    /// A file with more accepted words, one on each line, with `#` comments.
    /// The path starts at the directory of the configuration file.
    pub words_file: Option<PathBuf>,
    /// Paths that are not examined, in the syntax of `.gitignore`.
    #[serde(default)]
    pub ignore: Vec<String>,
}

impl Config {
    /// Read `path`, or use the defaults when there is no file.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("{} is not valid", path.display()))
    }

    /// The accepted words: the inline list, plus the words file if there is one.
    pub fn allowlist(&self, config_path: &Path) -> Result<Allowlist> {
        let mut text: String = self.words.join("\n");
        if let Some(file) = &self.words_file {
            let path = config_path.parent().unwrap_or(Path::new(".")).join(file);
            let more = std::fs::read_to_string(&path)
                .with_context(|| format!("cannot read words file {}", path.display()))?;
            text.push('\n');
            text.push_str(&more);
        }
        Ok(Allowlist::parse(&text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_fields() {
        let c: Config = toml::from_str(
            r#"
            words = ["gantry", "sprocket"]
            words-file = "more.txt"
            ignore = ["vendor/"]
            "#,
        )
        .unwrap();
        assert_eq!(c.words, ["gantry", "sprocket"]);
        assert_eq!(c.words_file.as_deref(), Some(Path::new("more.txt")));
        assert_eq!(c.ignore, ["vendor/"]);
    }

    #[test]
    fn unknown_fields_are_errors() {
        assert!(toml::from_str::<Config>("wrods = []").is_err());
    }

    #[test]
    fn inline_words_make_an_allowlist() {
        let c = Config {
            words: vec!["gantry".into()],
            ..Default::default()
        };
        let allow = c.allowlist(Path::new("memelint.toml")).unwrap();
        assert!(allow.allows("gantries"));
        assert!(!allow.allows("sprocket"));
    }
}
