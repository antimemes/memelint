//! The project configuration: `memelint.toml` at the project root.
//!
//! ```toml
//! words = ["gantry", "sprocket"]               # for all files
//! word-lists = ["words.txt", "https://example.com/team-words.txt"]
//! default-word-lists = true                    # the bundled lists
//! ignore = ["vendor/", "*.generated.go"]
//!
//! [languages.rust]                             # for files of one language
//! words = ["monomorphize"]
//! word-lists = ["rust-words.txt"]
//! default-word-lists = true
//! ```
//!
//! An entry of `word-lists` is a path, a URL, or the name of a bundled list.
//! The entries add to the bundled lists, unless `default-word-lists` is
//! not set. A URL is read from the cache that `memelint fetch` fills.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::check::Allowlist;
use crate::defaults;

pub const CONFIG_FILE: &str = "memelint.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    /// Technical names and verbs that the project accepts, for all files.
    #[serde(default)]
    pub words: Vec<String>,
    /// Word lists for all files: paths, URLs, or names of bundled lists.
    #[serde(default)]
    pub word_lists: Vec<String>,
    /// Use the bundled software list for all files, and the bundled list of
    /// each language for its files.
    #[serde(default = "yes")]
    pub default_word_lists: bool,
    /// Paths that are not examined, in the syntax of `.gitignore`.
    #[serde(default)]
    pub ignore: Vec<String>,
    /// Words for the files of one language, by grammar name.
    #[serde(default)]
    pub languages: BTreeMap<String, LanguageWords>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct LanguageWords {
    #[serde(default)]
    pub words: Vec<String>,
    /// Word lists for the files of this language.
    #[serde(default)]
    pub word_lists: Vec<String>,
    /// Use the bundled list of this language. Default: the top-level value.
    pub default_word_lists: Option<bool>,
}

fn yes() -> bool {
    true
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

    /// The accepted words for files of `language`: the bundled lists, then
    /// the words and word lists of the top level, then those of the language.
    pub fn allowlist(&self, language: &str, config_path: &Path) -> Result<Allowlist> {
        let dir = config_path.parent().unwrap_or(Path::new("."));
        let lang = self.languages.get(language);
        let mut allow = Allowlist::default();

        if self.default_word_lists {
            allow.extend(defaults::list("software").unwrap());
        }
        if lang
            .and_then(|l| l.default_word_lists)
            .unwrap_or(self.default_word_lists)
            && let Some(text) = defaults::list(language)
        {
            allow.extend(text);
        }

        allow.extend(&self.words.join("\n"));
        for source in &self.word_lists {
            allow.extend(&read_list(source, dir)?);
        }
        if let Some(lang) = lang {
            allow.extend(&lang.words.join("\n"));
            for source in &lang.word_lists {
                allow.extend(&read_list(source, dir)?);
            }
        }
        Ok(allow)
    }

    /// All the URLs in the configuration.
    pub fn urls(&self) -> Vec<&str> {
        self.word_lists
            .iter()
            .chain(self.languages.values().flat_map(|l| l.word_lists.iter()))
            .map(String::as_str)
            .filter(|s| is_url(s))
            .collect()
    }
}

pub fn is_url(source: &str) -> bool {
    source.starts_with("https://") || source.starts_with("http://")
}

/// Where the text of a URL is kept: `<cache>/words/<hash of the url>.txt`.
pub fn cached_path(url: &str) -> Option<PathBuf> {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    url.hash(&mut h);
    Some(
        memelint_common::cache_dir()?
            .join("words")
            .join(format!("{:016x}.txt", h.finish())),
    )
}

/// The text of one `word-lists` entry.
fn read_list(source: &str, dir: &Path) -> Result<String> {
    if let Some(text) = defaults::list(source) {
        return Ok(text.to_string());
    }
    if is_url(source) {
        let path = cached_path(source).context("no cache directory: HOME is not set")?;
        if !path.is_file() {
            bail!("{source} is not in the cache; run `memelint fetch` first");
        }
        return std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read {}", path.display()));
    }
    let path = dir.join(source);
    std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read word list {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowlist(toml: &str, language: &str) -> Allowlist {
        let c: Config = toml::from_str(toml).unwrap();
        c.allowlist(language, Path::new("memelint.toml")).unwrap()
    }

    #[test]
    fn parses_all_fields() {
        let c: Config = toml::from_str(
            r#"
            words = ["gantry", "sprocket"]
            word-lists = ["more.txt", "https://example.com/w.txt"]
            default-word-lists = false
            ignore = ["vendor/"]

            [languages.rust]
            words = ["frobnicate"]
            word-lists = ["rust.txt"]
            default-word-lists = true
            "#,
        )
        .unwrap();
        assert_eq!(c.words, ["gantry", "sprocket"]);
        assert_eq!(c.word_lists, ["more.txt", "https://example.com/w.txt"]);
        assert!(!c.default_word_lists);
        assert_eq!(c.ignore, ["vendor/"]);
        assert_eq!(c.languages["rust"].words, ["frobnicate"]);
        assert_eq!(c.languages["rust"].word_lists, ["rust.txt"]);
        assert_eq!(c.languages["rust"].default_word_lists, Some(true));
        assert_eq!(c.urls(), ["https://example.com/w.txt"]);
    }

    #[test]
    fn unknown_fields_are_errors() {
        assert!(toml::from_str::<Config>("wrods = []").is_err());
    }

    #[test]
    fn layers_of_words() {
        let toml = r#"
            words = ["gantry"]
            [languages.rust]
            words = ["frobnicate"]
        "#;
        let rust = allowlist(toml, "rust");
        let python = allowlist(toml, "python");
        // The project word is in each list, the language word in one.
        assert!(rust.allows("gantries") && python.allows("gantries"));
        assert!(rust.allows("frobnicate") && !python.allows("frobnicate"));
        // The bundled lists are on by default: software for all, Rust for Rust.
        assert!(rust.allows("byte") && python.allows("byte"));
        assert!(rust.allows("crate") && !python.allows("crate"));
        assert!(!python.allows("sprocket"));
    }

    #[test]
    fn default_word_lists_switch() {
        // Off at the top: no bundled list for all files.
        let rust = allowlist("default-word-lists = false", "rust");
        assert!(!rust.allows("byte") && !rust.allows("crate"));
        // Off at the top, on for Rust: only the Rust list, for Rust.
        let toml = "default-word-lists = false\n[languages.rust]\ndefault-word-lists = true";
        let rust = allowlist(toml, "rust");
        assert!(!rust.allows("byte") && rust.allows("crate"));
        // Off for Rust only.
        let rust = allowlist("[languages.rust]\ndefault-word-lists = false", "rust");
        assert!(rust.allows("byte") && !rust.allows("crate"));
    }

    #[test]
    fn bundled_names_and_files_in_word_lists() {
        // A bundled name for all files, with the defaults off.
        let python = allowlist(
            r#"default-word-lists = false
            word-lists = ["rust"]"#,
            "python",
        );
        assert!(python.allows("crate") && !python.allows("byte"));
        // A path, from the directory of the configuration file.
        let dir = std::env::temp_dir().join(format!("memelint-words-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("w.txt"), "# ours\nflange\n").unwrap();
        let c: Config = toml::from_str(r#"word-lists = ["w.txt"]"#).unwrap();
        let allow = c.allowlist("python", &dir.join("memelint.toml")).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(allow.allows("flanges"));
        // A URL that is not in the cache is an error that names the command.
        let c: Config =
            toml::from_str(r#"word-lists = ["https://nonexistent.invalid/w.txt"]"#).unwrap();
        let err = c
            .allowlist("python", Path::new("memelint.toml"))
            .unwrap_err();
        assert!(err.to_string().contains("memelint fetch"));
    }
}

#[cfg(test)]
mod schema {
    //! `memelint.schema.json` must agree with `Config`.
    use serde_json::Value;

    #[test]
    fn schema_matches_the_config() {
        let schema: Value = serde_json::from_str(include_str!("../memelint.schema.json")).unwrap();
        let keys = |v: &Value| -> Vec<String> {
            let mut k: Vec<String> = v["properties"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            k.sort();
            k
        };
        assert_eq!(
            keys(&schema),
            [
                "default-word-lists",
                "ignore",
                "languages",
                "word-lists",
                "words"
            ]
        );
        let language = &schema["properties"]["languages"]["additionalProperties"];
        assert_eq!(
            keys(language),
            ["default-word-lists", "word-lists", "words"]
        );
        // The config accepts each key that the schema names.
        let all = r#"
            words = []
            word-lists = []
            default-word-lists = true
            ignore = []
            [languages.rust]
            words = []
            word-lists = []
            default-word-lists = false
        "#;
        toml::from_str::<super::Config>(all).unwrap();
    }
}
