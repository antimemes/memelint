//! The project configuration: `memelint.toml` at the project root.
//!
//! ```toml
//! profile = "strict"                           # or "basic"
//! words = ["gantry", "sprocket"]               # for all files
//! word-lists = ["words.txt", "https://example.com/team-words.txt"]
//! default-word-lists = true                    # the bundled lists
//! ignore = ["vendor/", "*.generated.go"]
//!
//! [rules]                                      # by ASD-STE100 rule id
//! "6.3" = "warning"                            # "error", "warning" or "off"
//!
//! [languages.rust]                             # for files of one language
//! words = ["monomorphize"]
//! word-lists = ["rust-words.txt"]
//! default-word-lists = true
//!
//! [languages.rust.rules]
//! "1.6" = "off"
//!
//! [languages.markdown]                         # prose is off by default
//! check = true
//! ```
//!
//! An entry of `word-lists` is a path, a URL, or the name of a bundled list.
//! The entries add to the bundled lists, unless `default-word-lists` is
//! not set. A URL is read from the cache that `memelint fetch` fills.
//!
//! The profile gives each rule a severity. `[rules]` changes it for one
//! rule, and `[languages.<name>.rules]` for one rule in one language.
//! `strict` makes each finding an error. `basic` makes unknown words (1.6)
//! and long sentences (6.3) warnings.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::check::Allowlist;
use crate::rules::{Does, Feasibility};
use crate::{defaults, rules};

pub const CONFIG_FILE: &str = "memelint.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    Strict,
    Basic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Error,
    Warning,
    Off,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    /// The severity of each rule, before `rules`. Default: strict.
    #[serde(default = "strict")]
    pub profile: Profile,
    /// The severity of one rule, by ASD-STE100 rule id.
    #[serde(default)]
    pub rules: BTreeMap<String, Severity>,
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
    /// The configuration for the files of one language, by grammar name.
    #[serde(default)]
    pub languages: BTreeMap<String, Language>,
}

impl Default for Config {
    fn default() -> Self {
        toml::from_str("").expect("an empty configuration is valid")
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Language {
    /// The severity of one rule, for the files of this language.
    #[serde(default)]
    pub rules: BTreeMap<String, Severity>,
    #[serde(default)]
    pub words: Vec<String>,
    /// Word lists for the files of this language.
    #[serde(default)]
    pub word_lists: Vec<String>,
    /// Use the bundled list of this language. Default: the top-level value.
    pub default_word_lists: Option<bool>,
    /// Examine the files of this language. Default: on for code, off for
    /// prose documents, for example Markdown.
    pub check: Option<bool>,
}

fn yes() -> bool {
    true
}

fn strict() -> Profile {
    Profile::Strict
}

impl Config {
    /// Read `path`, or use the defaults when there is no file.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let config: Self =
            toml::from_str(&text).with_context(|| format!("{} is not valid", path.display()))?;
        let language_rules = config.languages.values().flat_map(|l| l.rules.keys());
        for id in config.rules.keys().chain(language_rules) {
            check_rule_id(id)?;
        }
        Ok(config)
    }

    /// The severity of a rule in files of `language`: from the rules of the
    /// language, then from `rules`, then from the profile.
    pub fn severity(&self, rule: &str, language: &str) -> Severity {
        if let Some(s) = self.languages.get(language).and_then(|l| l.rules.get(rule)) {
            return *s;
        }
        if let Some(s) = self.rules.get(rule) {
            return *s;
        }
        match (self.profile, rule) {
            (Profile::Basic, "1.6" | "6.3") => Severity::Warning,
            _ => Severity::Error,
        }
    }

    /// Are the files of `language` examined? Code is, and a prose document
    /// (`prose`) is not, unless the configuration turns the language on or off.
    pub fn checks(&self, language: &str, prose: bool) -> bool {
        self.languages
            .get(language)
            .and_then(|l| l.check)
            .unwrap_or(!prose)
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
    #[cfg(any(feature = "fetch", test))]
    pub fn urls(&self) -> Vec<&str> {
        self.word_lists
            .iter()
            .chain(self.languages.values().flat_map(|l| l.word_lists.iter()))
            .map(String::as_str)
            .filter(|s| is_url(s))
            .collect()
    }
}

/// A rule id in `[rules]` must be a rule that gives findings.
fn check_rule_id(id: &str) -> Result<()> {
    if rules::CHECKED.contains(&id) {
        return Ok(());
    }
    let Some(rule) = rules::find(id) else {
        bail!("there is no rule {id}; the rules are 1.1 to 9.4");
    };
    let why = match (rule.does, rule.feasibility) {
        (Does::No, Feasibility::Judgment | Feasibility::NotApplicable) => {
            "a program cannot check it"
        }
        (Does::No, _) => "memelint does not check it yet",
        _ => "it gives no findings of its own",
    };
    bail!(
        "rule {id} ({}) cannot be set: {why}; the rules that give findings are {}",
        rule.summary,
        rules::CHECKED.join(", ")
    )
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
            #[cfg(feature = "fetch")]
            bail!("{source} is not in the cache; run `memelint fetch` first");
            #[cfg(not(feature = "fetch"))]
            bail!(
                "{source} is not in the cache; this memelint has no `fetch`, put its text at {}",
                path.display()
            );
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

    fn config(toml: &str) -> Config {
        toml::from_str(toml).unwrap()
    }

    fn allowlist(toml: &str, language: &str) -> Allowlist {
        config(toml)
            .allowlist(language, Path::new("memelint.toml"))
            .unwrap()
    }

    #[test]
    fn parses_all_fields() {
        let c = config(
            r#"
            profile = "basic"
            words = ["gantry", "sprocket"]
            word-lists = ["more.txt", "https://example.com/w.txt"]
            default-word-lists = false
            ignore = ["vendor/"]

            [rules]
            "6.3" = "off"

            [languages.rust]
            words = ["frobnicate"]
            word-lists = ["rust.txt"]
            default-word-lists = true
            check = true

            [languages.rust.rules]
            "1.6" = "warning"
            "#,
        );
        assert_eq!(c.profile, Profile::Basic);
        assert_eq!(c.rules["6.3"], Severity::Off);
        assert_eq!(c.words, ["gantry", "sprocket"]);
        assert_eq!(c.word_lists, ["more.txt", "https://example.com/w.txt"]);
        assert!(!c.default_word_lists);
        assert_eq!(c.ignore, ["vendor/"]);
        assert_eq!(c.languages["rust"].words, ["frobnicate"]);
        assert_eq!(c.languages["rust"].word_lists, ["rust.txt"]);
        assert_eq!(c.languages["rust"].default_word_lists, Some(true));
        assert_eq!(c.languages["rust"].rules["1.6"], Severity::Warning);
        assert_eq!(c.languages["rust"].check, Some(true));
        assert_eq!(c.urls(), ["https://example.com/w.txt"]);
    }

    #[test]
    fn check_defaults_to_code_only() {
        let c = Config::default();
        assert!(c.checks("python", false));
        assert!(!c.checks("markdown", true));
        // The configuration turns a language on or off.
        let c = config("[languages.markdown]\ncheck = true");
        assert!(c.checks("markdown", true));
        let c = config("[languages.python]\ncheck = false");
        assert!(!c.checks("python", false));
    }

    #[test]
    fn unknown_fields_are_errors() {
        assert!(toml::from_str::<Config>("wrods = []").is_err());
    }

    #[test]
    fn severities() {
        let c = Config::default();
        assert_eq!(c.severity("1.1", "rust"), Severity::Error);
        assert_eq!(c.severity("6.3", "rust"), Severity::Error);
        let c = config("profile = \"basic\"");
        assert_eq!(c.severity("1.1", "rust"), Severity::Error);
        assert_eq!(c.severity("1.6", "rust"), Severity::Warning);
        assert_eq!(c.severity("6.3", "rust"), Severity::Warning);
        let c = config("[rules]\n\"1.1\" = \"off\"\n\"6.3\" = \"warning\"");
        assert_eq!(c.severity("1.1", "rust"), Severity::Off);
        assert_eq!(c.severity("6.3", "rust"), Severity::Warning);
        assert_eq!(c.severity("4.2", "rust"), Severity::Error);
    }

    #[test]
    fn severities_for_one_language() {
        let c =
            config("[rules]\n\"1.6\" = \"warning\"\n[languages.markdown.rules]\n\"1.6\" = \"off\"");
        assert_eq!(c.severity("1.6", "markdown"), Severity::Off);
        assert_eq!(c.severity("1.6", "rust"), Severity::Warning);
        assert_eq!(c.severity("1.1", "markdown"), Severity::Error);
    }

    #[test]
    fn rules_that_give_no_findings_are_errors() {
        let dir = std::env::temp_dir().join(format!("memelint-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("memelint.toml");
        std::fs::write(&path, "[rules]\n\"6.4\" = \"off\"").unwrap();
        let err = Config::load(&path).unwrap_err().to_string();
        std::fs::write(&path, "[rules]\n\"12.1\" = \"off\"").unwrap();
        let err2 = Config::load(&path).unwrap_err().to_string();
        std::fs::write(&path, "[languages.rust.rules]\n\"5.1\" = \"off\"").unwrap();
        let err3 = Config::load(&path).unwrap_err().to_string();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(err.contains("6.4") && err.contains("cannot check") && err.contains("1.1"));
        assert!(err2.contains("no rule 12.1"));
        assert!(err3.contains("5.1") && err3.contains("not check it yet"));
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
            "default-word-lists = false\nword-lists = [\"rust\"]",
            "python",
        );
        assert!(python.allows("crate") && !python.allows("byte"));
        // A path, from the directory of the configuration file.
        let dir = std::env::temp_dir().join(format!("memelint-words-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("w.txt"), "# ours\nflange\n").unwrap();
        let c = config(r#"word-lists = ["w.txt"]"#);
        let allow = c.allowlist("python", &dir.join("memelint.toml")).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(allow.allows("flanges"));
        // A URL that is not in the cache is an error that gives the instruction.
        let c = config(r#"word-lists = ["https://nonexistent.invalid/w.txt"]"#);
        let err = c
            .allowlist("python", Path::new("memelint.toml"))
            .unwrap_err();
        assert!(err.to_string().contains("is not in the cache"));
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
                "profile",
                "rules",
                "word-lists",
                "words"
            ]
        );
        let language = &schema["properties"]["languages"]["additionalProperties"];
        assert_eq!(
            keys(language),
            [
                "check",
                "default-word-lists",
                "rules",
                "word-lists",
                "words"
            ]
        );
        // Each `rules` table refers to the one definition. Its ids are the
        // rules that give findings.
        for rules in [
            &schema["properties"]["rules"],
            &language["properties"]["rules"],
        ] {
            assert_eq!(rules["$ref"], "#/$defs/rules");
        }
        {
            let mut ids: Vec<&str> = schema["$defs"]["rules"]["properties"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            ids.sort();
            let mut checked = crate::rules::CHECKED.to_vec();
            checked.sort();
            assert_eq!(ids, checked);
        }
        // The config accepts each key that the schema names.
        let all = r#"
            profile = "basic"
            words = []
            word-lists = []
            default-word-lists = true
            ignore = []
            [rules]
            "1.1" = "error"
            [languages.rust]
            words = []
            word-lists = []
            default-word-lists = false
            check = true
            [languages.rust.rules]
            "1.1" = "off"
        "#;
        toml::from_str::<super::Config>(all).unwrap();
    }
}
