//! The project configuration: `memelint.toml` at the project root.
//!
//! ```toml
//! profile = "basic"                            # or "strict"
//! words = ["gantry", "sprocket"]               # for all files
//! word-lists = ["words.txt", "https://example.com/team-words.txt"]
//! subtree-word-lists = ["experiments/*/memelint-words.txt"]
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
//!
//! [scopes.docs]                                # for the paths it names
//! paths = ["docs/", "crates/memelint/"]        # as in .gitignore
//! profile = "strict"
//! words = ["gantry"]
//! word-lists = ["docs/words.txt"]
//! check = true                                 # examine these paths
//!
//! [scopes.docs.rules]
//! "6.3" = "off"
//! ```
//!
//! An entry of `word-lists` is a path, a URL, or the name of a bundled list.
//! The entries add to the bundled lists, unless `default-word-lists` is
//! not set. A URL is read from the cache that `memelint fetch` fills.
//!
//! An entry of `subtree-word-lists` names word-list files, with `*` and `**`
//! as in `.gitignore`. Each file applies to the directory it is in and to all
//! files below it.
//!
//! The profile gives each rule a severity. `[rules]` changes it for one
//! rule, and `[languages.<name>.rules]` for one rule in one language.
//! `basic`, the default, checks the form of the writing, and no word list is
//! necessary. Unapproved words (1.1) and unknown words (1.6) are off, and
//! long sentences (6.3) are warnings. `strict` adds the controlled
//! vocabulary, with the word list of the project: each finding is an error.
//!
//! A scope, `[scopes.<name>]`, names paths and gives them a profile, rules
//! and words. The scopes stay in the sequence of the file. When two scopes
//! name one file, the words of the two apply, and the last of them in the
//! file sets the severity. See [`Config::severity`].
//!
//! `check` turns memelint on or off: at the top level, for a language, or
//! for a scope. The last scope that names a file and sets `check` applies
//! first, then the language, then the top level. See [`Config::checks`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use indexmap::IndexMap;
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
    /// The severity of each rule, before `rules`. Default: basic.
    #[serde(default = "basic")]
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
    /// Word-list files, by name with `*` and `**`. Each file applies to the
    /// directory it is in and to all files below it.
    #[serde(default)]
    pub subtree_word_lists: Vec<String>,
    /// Use the bundled software list for all files, and the bundled list of
    /// each language for its files.
    #[serde(default = "yes")]
    pub default_word_lists: bool,
    /// Paths that are not examined, in the syntax of `.gitignore`.
    #[serde(default)]
    pub ignore: Vec<String>,
    /// Examine the files. Default: on for code, off for prose documents, for
    /// example Markdown. `true` turns prose on too.
    pub check: Option<bool>,
    /// The configuration for the files of one language, by grammar name.
    #[serde(default)]
    pub languages: BTreeMap<String, Language>,
    /// The profile, rules, words and `check` of the paths that each scope
    /// names, in the sequence of the file.
    #[serde(default)]
    pub scopes: IndexMap<String, Scope>,
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

/// The configuration of the paths that `paths` names.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Scope {
    /// The paths, in the syntax of `.gitignore`, from the directory of the
    /// configuration file. Necessary, and not empty.
    #[serde(default)]
    pub paths: Vec<String>,
    /// The severity of each rule in these paths, before the rules. Default:
    /// the top-level profile.
    pub profile: Option<Profile>,
    /// The severity of one rule, for the files in these paths.
    #[serde(default)]
    pub rules: BTreeMap<String, Severity>,
    #[serde(default)]
    pub words: Vec<String>,
    /// Word lists for the files in these paths.
    #[serde(default)]
    pub word_lists: Vec<String>,
    /// Examine the files in these paths. Default: the `check` of a scope
    /// before it that names the file, then of the language, then the
    /// top-level `check`.
    pub check: Option<bool>,
}

fn yes() -> bool {
    true
}

fn basic() -> Profile {
    Profile::Basic
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
        let scope_rules = config.scopes.values().flat_map(|s| s.rules.keys());
        for id in config.rules.keys().chain(language_rules).chain(scope_rules) {
            check_rule_id(id)?;
        }
        for (name, scope) in &config.scopes {
            if scope.paths.is_empty() {
                bail!("`[scopes.{name}]` needs `paths`");
            }
        }
        let dir = path.parent().unwrap_or(Path::new("."));
        config.scope_matchers(dir)?;
        for entry in &config.subtree_word_lists {
            if entry.ends_with('/') {
                bail!("`subtree-word-lists`: `{entry}` must name a file, not a directory");
            }
            // A name with `*`, `?`, `[` or `{` can give no file. A name
            // without them must be a file.
            if is_literal_pattern(entry) && !dir.join(entry).is_file() {
                bail!("`subtree-word-lists`: `{entry}` is not a file");
            }
        }
        config.subtree_globs()?;
        Ok(config)
    }

    /// The entries of `subtree-word-lists` as one `GlobSet`. `*` stops at
    /// `/`; `**` does not.
    pub fn subtree_globs(&self) -> Result<GlobSet> {
        let mut set = GlobSetBuilder::new();
        for entry in &self.subtree_word_lists {
            let glob = GlobBuilder::new(entry)
                .literal_separator(true)
                .build()
                .with_context(|| {
                    format!("`subtree-word-lists`: `{entry}` is not a valid pattern")
                })?;
            set.add(glob);
        }
        set.build()
            .context("`subtree-word-lists`: the patterns do not compile")
    }

    /// The `paths` of each scope as one `Gitignore` from `dir`, the directory
    /// of the configuration file, in the sequence of `scopes`.
    pub fn scope_matchers(&self, dir: &Path) -> Result<Vec<Gitignore>> {
        self.scopes
            .iter()
            .map(|(name, scope)| {
                let mut builder = GitignoreBuilder::new(dir);
                for glob in &scope.paths {
                    builder.add_line(None, glob).with_context(|| {
                        format!("`[scopes.{name}]`: `{glob}` is not a valid pattern")
                    })?;
                }
                builder
                    .build()
                    .with_context(|| format!("`[scopes.{name}]`: the paths do not compile"))
            })
            .collect()
    }

    /// The scopes that name `path`, as positions in `scopes`. `path` starts
    /// at the directory of the configuration file.
    pub fn scopes_of(&self, matchers: &[Gitignore], path: &Path) -> Vec<usize> {
        matchers
            .iter()
            .enumerate()
            .filter(|(_, m)| m.matched_path_or_any_parents(path, false).is_ignore())
            .map(|(i, _)| i)
            .collect()
    }

    /// The severity of a rule in a file of `language`. `scopes` gives the
    /// scopes that name the file, as positions in `scopes`, in the sequence
    /// of the file. The rules of the last scope apply first, then the rules
    /// of each scope before it. Then the rules of the language, then `rules`.
    /// Then the profile of the last scope that has one, then the top-level
    /// profile.
    pub fn severity(&self, rule: &str, language: &str, scopes: &[usize]) -> Severity {
        let scopes = || {
            scopes
                .iter()
                .rev()
                .filter_map(|&i| self.scopes.get_index(i))
                .map(|(_, s)| s)
        };
        if let Some(s) = scopes().find_map(|s| s.rules.get(rule)) {
            return *s;
        }
        if let Some(s) = self.languages.get(language).and_then(|l| l.rules.get(rule)) {
            return *s;
        }
        if let Some(s) = self.rules.get(rule) {
            return *s;
        }
        let profile = scopes().find_map(|s| s.profile).unwrap_or(self.profile);
        match (profile, rule) {
            (Profile::Basic, "1.1" | "1.6") => Severity::Off,
            (Profile::Basic, "6.3") => Severity::Warning,
            _ => Severity::Error,
        }
    }

    /// Is a file of `language` examined? `scopes` gives the scopes that name
    /// the file, as positions in `scopes`. The `check` of the last scope that
    /// has one applies first, then the `check` of the language, then the
    /// top-level `check`. Without them, code is examined, and a prose
    /// document (`prose`) is not.
    pub fn checks(&self, language: &str, prose: bool, scopes: &[usize]) -> bool {
        scopes
            .iter()
            .rev()
            .filter_map(|&i| self.scopes.get_index(i))
            .find_map(|(_, s)| s.check)
            .or_else(|| self.languages.get(language).and_then(|l| l.check))
            .or(self.check)
            .unwrap_or(!prose)
    }

    /// The accepted words for files of `language` in the scopes `scopes`
    /// (positions in `scopes`). These are the bundled lists, then the words
    /// and word lists of the top level. Then those of the language, and then
    /// those of each scope.
    pub fn allowlist(
        &self,
        language: &str,
        config_path: &Path,
        scopes: &[usize],
    ) -> Result<Allowlist> {
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
        for scope in scopes.iter().filter_map(|&i| self.scopes.get_index(i)) {
            allow.extend(&scope.1.words.join("\n"));
            for source in &scope.1.word_lists {
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
            .chain(self.scopes.values().flat_map(|s| s.word_lists.iter()))
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

/// Does `s` name one file? It does if it has none of `*`, `?`, `[` and `{`.
pub fn is_literal_pattern(s: &str) -> bool {
    !s.contains(['*', '?', '[', '{'])
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
            .allowlist(language, Path::new("memelint.toml"), &[])
            .unwrap()
    }

    #[test]
    fn parses_all_fields() {
        let c = config(
            r#"
            profile = "strict"
            words = ["gantry", "sprocket"]
            word-lists = ["more.txt", "https://example.com/w.txt"]
            subtree-word-lists = ["experiments/*/memelint-words.txt"]
            default-word-lists = false
            ignore = ["vendor/"]
            check = false

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
        assert_eq!(c.profile, Profile::Strict);
        assert_eq!(c.rules["6.3"], Severity::Off);
        assert_eq!(c.words, ["gantry", "sprocket"]);
        assert_eq!(c.word_lists, ["more.txt", "https://example.com/w.txt"]);
        assert_eq!(c.subtree_word_lists, ["experiments/*/memelint-words.txt"]);
        assert!(!c.default_word_lists);
        assert_eq!(c.ignore, ["vendor/"]);
        assert_eq!(c.check, Some(false));
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
        assert!(c.checks("python", false, &[]));
        assert!(!c.checks("markdown", true, &[]));
        // The configuration turns a language on or off.
        let c = config("[languages.markdown]\ncheck = true");
        assert!(c.checks("markdown", true, &[]));
        let c = config("[languages.python]\ncheck = false");
        assert!(!c.checks("python", false, &[]));
    }

    #[test]
    fn check_of_the_top_level_and_of_scopes() {
        let c = config(
            r#"
            check = false

            [languages.rust]
            check = true

            [scopes.on]
            paths = ["on/"]
            check = true

            [scopes.off]
            paths = ["off/"]
            check = false

            [scopes.words]
            paths = ["words/"]
            words = ["gantry"]
            "#,
        );
        // The top-level `check = false` turns code off, but not a language
        // with `check = true`.
        assert!(!c.checks("python", false, &[]));
        assert!(c.checks("rust", false, &[]));
        // A scope with `check = true` applies before the top level, for
        // code and for prose.
        assert!(c.checks("python", false, &[0]));
        assert!(c.checks("markdown", true, &[0]));
        // A scope with `check = false` applies before the language.
        assert!(!c.checks("rust", false, &[1]));
        // When two scopes set `check`, the last of them applies.
        assert!(!c.checks("python", false, &[0, 1]));
        // A scope without `check` does not change the language or the
        // scope before it.
        assert!(c.checks("rust", false, &[2]));
        assert!(!c.checks("python", false, &[2]));
        assert!(!c.checks("rust", false, &[1, 2]));
        // The top-level `check = true` turns prose on too, and a language
        // applies before it.
        let c = config("check = true");
        assert!(c.checks("markdown", true, &[]));
        let c = config("check = true\n[languages.markdown]\ncheck = false");
        assert!(!c.checks("markdown", true, &[]));
    }

    #[test]
    fn unknown_fields_are_errors() {
        assert!(toml::from_str::<Config>("wrods = []").is_err());
    }

    #[test]
    fn severities() {
        // The default profile is basic.
        let c = Config::default();
        assert_eq!(c.profile, Profile::Basic);
        assert_eq!(c.severity("1.1", "rust", &[]), Severity::Off);
        assert_eq!(c.severity("1.6", "rust", &[]), Severity::Off);
        assert_eq!(c.severity("6.3", "rust", &[]), Severity::Warning);
        assert_eq!(c.severity("4.2", "rust", &[]), Severity::Error);
        let c = config("profile = \"basic\"");
        assert_eq!(c.severity("1.1", "rust", &[]), Severity::Off);
        assert_eq!(c.severity("1.6", "rust", &[]), Severity::Off);
        assert_eq!(c.severity("6.3", "rust", &[]), Severity::Warning);
        assert_eq!(c.severity("4.2", "rust", &[]), Severity::Error);
        let c = config("profile = \"strict\"");
        assert_eq!(c.severity("1.1", "rust", &[]), Severity::Error);
        assert_eq!(c.severity("1.6", "rust", &[]), Severity::Error);
        assert_eq!(c.severity("6.3", "rust", &[]), Severity::Error);
        let c = config("[rules]\n\"1.1\" = \"off\"\n\"6.3\" = \"warning\"");
        assert_eq!(c.severity("1.1", "rust", &[]), Severity::Off);
        assert_eq!(c.severity("6.3", "rust", &[]), Severity::Warning);
        assert_eq!(c.severity("4.2", "rust", &[]), Severity::Error);
    }

    #[test]
    fn severities_for_one_language() {
        let c =
            config("[rules]\n\"1.6\" = \"warning\"\n[languages.markdown.rules]\n\"1.6\" = \"off\"");
        assert_eq!(c.severity("1.6", "markdown", &[]), Severity::Off);
        assert_eq!(c.severity("1.6", "rust", &[]), Severity::Warning);
        assert_eq!(c.severity("4.2", "markdown", &[]), Severity::Error);
    }

    #[test]
    fn parses_a_scope_with_all_keys() {
        let c = config(
            r#"
            [scopes.docs]
            paths = ["docs/", "*.md"]
            profile = "strict"
            words = ["gantry"]
            word-lists = ["docs/words.txt", "https://example.com/docs.txt"]
            check = true

            [scopes.docs.rules]
            "6.3" = "off"
            "#,
        );
        let docs = &c.scopes["docs"];
        assert_eq!(docs.paths, ["docs/", "*.md"]);
        assert_eq!(docs.profile, Some(Profile::Strict));
        assert_eq!(docs.words, ["gantry"]);
        assert_eq!(
            docs.word_lists,
            ["docs/words.txt", "https://example.com/docs.txt"]
        );
        assert_eq!(docs.rules["6.3"], Severity::Off);
        assert_eq!(docs.check, Some(true));
        // The URL of a scope is in `urls`, for `memelint fetch`.
        assert_eq!(c.urls(), ["https://example.com/docs.txt"]);
        // A scope has no `ignore`, and no other key that is not in the list.
        assert!(toml::from_str::<Config>("[scopes.x]\npaths = [\"x/\"]\nignore = []").is_err());
    }

    #[test]
    fn scopes_stay_in_file_order() {
        let c = config("[scopes.zeta]\npaths = [\"z/\"]\n[scopes.alpha]\npaths = [\"a/\"]");
        let names: Vec<&str> = c.scopes.keys().map(String::as_str).collect();
        assert_eq!(names, ["zeta", "alpha"]);
    }

    #[test]
    fn severities_in_scopes() {
        let c = config(
            r#"
            profile = "basic"

            [rules]
            "4.2" = "warning"

            [languages.rust.rules]
            "3.5" = "warning"

            [scopes.first]
            paths = ["a/"]
            profile = "strict"

            [scopes.first.rules]
            "1.2" = "off"
            "4.2" = "off"

            [scopes.second]
            paths = ["a/b/"]
            profile = "basic"

            [scopes.second.rules]
            "1.2" = "warning"
            "3.5" = "off"
            "#,
        );
        // No scope: the language, then `rules`, then the top-level profile.
        assert_eq!(c.severity("3.5", "rust", &[]), Severity::Warning);
        assert_eq!(c.severity("4.2", "rust", &[]), Severity::Warning);
        assert_eq!(c.severity("1.6", "rust", &[]), Severity::Off);
        // One scope: its rules, then the language, then `rules`, then its
        // profile, then the top-level profile.
        assert_eq!(c.severity("4.2", "rust", &[0]), Severity::Off);
        assert_eq!(c.severity("3.5", "rust", &[0]), Severity::Warning);
        assert_eq!(c.severity("1.6", "rust", &[0]), Severity::Error);
        assert_eq!(c.severity("1.6", "rust", &[1]), Severity::Off);
        // The scope rules of the second scope apply before the first.
        assert_eq!(c.severity("1.2", "rust", &[0, 1]), Severity::Warning);
        assert_eq!(c.severity("3.5", "rust", &[0, 1]), Severity::Off);
        assert_eq!(c.severity("4.2", "rust", &[0, 1]), Severity::Off);
        // The profile of the last scope applies: basic, thus 1.6 is off.
        assert_eq!(c.severity("1.6", "rust", &[0, 1]), Severity::Off);
        // A scope without a profile does not change the profile of the scope
        // before it.
        let c = config(
            "[scopes.a]\npaths = [\"a/\"]\nprofile = \"strict\"\n[scopes.b]\npaths = [\"b/\"]",
        );
        assert_eq!(c.severity("1.6", "rust", &[0, 1]), Severity::Error);
        assert_eq!(c.severity("1.6", "rust", &[1]), Severity::Off);
    }

    #[test]
    fn scopes_are_checked_at_load() {
        let dir = std::env::temp_dir().join(format!("memelint-scopes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("memelint.toml");
        std::fs::write(&path, "[scopes.docs]\nprofile = \"strict\"").unwrap();
        let missing = Config::load(&path).unwrap_err().to_string();
        std::fs::write(&path, "[scopes.docs]\npaths = []").unwrap();
        let empty = Config::load(&path).unwrap_err().to_string();
        // `.gitignore` reads `[` without `]` as the character; a range that
        // goes down is an error.
        std::fs::write(&path, "[scopes.docs]\npaths = [\"docs/[z-a]\"]").unwrap();
        let bad = Config::load(&path).unwrap_err().to_string();
        std::fs::write(
            &path,
            "[scopes.docs]\npaths = [\"docs/\"]\n[scopes.docs.rules]\n\"6.4\" = \"off\"",
        )
        .unwrap();
        let rule = Config::load(&path).unwrap_err().to_string();
        // A path that names no file is accepted.
        std::fs::write(
            &path,
            "[scopes.docs]\npaths = [\"nothing/\", \"/README.md\"]",
        )
        .unwrap();
        let ok = Config::load(&path);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(missing, "`[scopes.docs]` needs `paths`");
        assert_eq!(empty, "`[scopes.docs]` needs `paths`");
        assert!(bad.contains("`[scopes.docs]`: `docs/[z-a]`"), "{bad}");
        assert!(rule.contains("6.4"), "{rule}");
        assert_eq!(ok.unwrap().scopes.len(), 1);
    }

    #[test]
    fn scopes_name_paths_as_gitignore_does() {
        let c = config(
            r#"
            [scopes.docs]
            paths = ["docs/", "*.md", "/README.txt"]
            [scopes.deep]
            paths = ["docs/deep/"]
            "#,
        );
        let m = c.scope_matchers(Path::new(".")).unwrap();
        let of = |p: &str| c.scopes_of(&m, Path::new(p));
        assert_eq!(of("docs/x.rs"), [0]);
        assert_eq!(of("docs/deep/x.rs"), [0, 1]);
        assert_eq!(of("src/x.md"), [0]);
        assert_eq!(of("README.txt"), [0]);
        assert_eq!(of("src/README.txt"), Vec::<usize>::new());
        assert_eq!(of("src/x.rs"), Vec::<usize>::new());
    }

    #[test]
    fn words_of_scopes_add_to_the_allowlist() {
        let toml = r#"
            words = ["gantry"]
            [scopes.a]
            paths = ["a/"]
            words = ["sprocket"]
            [scopes.b]
            paths = ["b/"]
            words = ["flange"]
        "#;
        let none = allowlist(toml, "rust");
        let both = config(toml)
            .allowlist("rust", Path::new("memelint.toml"), &[0, 1])
            .unwrap();
        assert!(none.allows("gantry") && !none.allows("sprocket") && !none.allows("flange"));
        assert!(both.allows("gantry") && both.allows("sprockets") && both.allows("flange"));
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
    fn literal_patterns() {
        assert!(is_literal_pattern("experiments/memelint-words.txt"));
        assert!(is_literal_pattern("a-b_c.d/e"));
        assert!(!is_literal_pattern("experiments/*/memelint-words.txt"));
        assert!(!is_literal_pattern("**/memelint-words.txt"));
        assert!(!is_literal_pattern("w?.txt"));
        assert!(!is_literal_pattern("w[ab].txt"));
        assert!(!is_literal_pattern("w{a,b}.txt"));
    }

    #[test]
    fn subtree_globs_keep_to_one_level_unless_doubled() {
        let one = config(r#"subtree-word-lists = ["experiments/*/memelint-words.txt"]"#)
            .subtree_globs()
            .unwrap();
        assert!(one.is_match("experiments/govsim/memelint-words.txt"));
        assert!(!one.is_match("experiments/a/b/memelint-words.txt"));
        assert!(!one.is_match("experiments/memelint-words.txt"));
        let all = config(r#"subtree-word-lists = ["**/memelint-words.txt"]"#)
            .subtree_globs()
            .unwrap();
        assert!(all.is_match("experiments/govsim/memelint-words.txt"));
        assert!(all.is_match("experiments/a/b/memelint-words.txt"));
        assert!(all.is_match("memelint-words.txt"));
        // No entries: a set that selects no path.
        assert!(!Config::default().subtree_globs().unwrap().is_match("x"));
    }

    #[test]
    fn subtree_word_lists_are_checked_at_load() {
        let dir = std::env::temp_dir().join(format!("memelint-subtree-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("experiments/govsim")).unwrap();
        let path = dir.join("memelint.toml");
        // A name without `*` must be a file.
        std::fs::write(&path, r#"subtree-word-lists = ["experiments/absent.txt"]"#).unwrap();
        let missing = Config::load(&path).unwrap_err().to_string();
        // A directory is not a word list.
        std::fs::write(&path, r#"subtree-word-lists = ["experiments/govsim/"]"#).unwrap();
        let slash = Config::load(&path).unwrap_err().to_string();
        // An entry that is not correct.
        std::fs::write(&path, r#"subtree-word-lists = ["experiments/["]"#).unwrap();
        let bad = Config::load(&path).unwrap_err().to_string();
        // A name with `*` can give no file; a name without `*` that is a
        // file is accepted.
        std::fs::write(
            dir.join("experiments/govsim/memelint-words.txt"),
            "flange\n",
        )
        .unwrap();
        std::fs::write(
            &path,
            "subtree-word-lists = [\"nothing/*/here.txt\", \"experiments/govsim/memelint-words.txt\"]",
        )
        .unwrap();
        let ok = Config::load(&path);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(
            missing.contains("`experiments/absent.txt` is not a file"),
            "{missing}"
        );
        assert!(
            slash.contains("`experiments/govsim/` must name a file"),
            "{slash}"
        );
        assert!(bad.contains("`experiments/[`"), "{bad}");
        assert_eq!(ok.unwrap().subtree_word_lists.len(), 2);
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
        let allow = c
            .allowlist("python", &dir.join("memelint.toml"), &[])
            .unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(allow.allows("flanges"));
        // A URL that is not in the cache is an error that gives the instruction.
        let c = config(r#"word-lists = ["https://nonexistent.invalid/w.txt"]"#);
        let err = c
            .allowlist("python", Path::new("memelint.toml"), &[])
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
                "check",
                "default-word-lists",
                "ignore",
                "languages",
                "profile",
                "rules",
                "scopes",
                "subtree-word-lists",
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
        let scope = &schema["properties"]["scopes"]["additionalProperties"];
        assert_eq!(
            keys(scope),
            ["check", "paths", "profile", "rules", "word-lists", "words"]
        );
        assert_eq!(scope["required"], serde_json::json!(["paths"]));
        assert_eq!(
            scope["properties"]["profile"]["enum"],
            schema["properties"]["profile"]["enum"]
        );
        // Each `rules` table refers to the one definition. Its ids are the
        // rules that give findings.
        for rules in [
            &schema["properties"]["rules"],
            &language["properties"]["rules"],
            &scope["properties"]["rules"],
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
            subtree-word-lists = []
            default-word-lists = true
            ignore = []
            check = true
            [rules]
            "1.1" = "error"
            [languages.rust]
            words = []
            word-lists = []
            default-word-lists = false
            check = true
            [languages.rust.rules]
            "1.1" = "off"
            [scopes.docs]
            paths = ["docs/"]
            profile = "strict"
            words = []
            word-lists = []
            check = false
            [scopes.docs.rules]
            "1.1" = "off"
        "#;
        toml::from_str::<super::Config>(all).unwrap();
    }
}
