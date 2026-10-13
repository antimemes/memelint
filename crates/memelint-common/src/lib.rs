//! The parts that all memelint binaries use: the vocabulary file that
//! `memelint-import` writes and `memelint` reads, and its location.
//!
//! Each machine makes the file from the document of a standard, and the file
//! does not go to other machines. Thus the programs must agree on its shape
//! and its location, and no binary contains it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Identifier of a standard, used for its file name and its subcommand.
pub const ASD_STE100: &str = "asd-ste100";

/// The environment variable that gives the vocabulary file to use.
pub const ENV_VOCAB: &str = "MEMELINT_VOCAB";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Vocabulary {
    /// E.g. "ASD-STE100".
    #[serde(default)]
    pub standard: String,
    pub issue: Option<u32>,
    #[serde(default)]
    pub issue_date: Option<String>,
    #[serde(default)]
    pub source: Option<Source>,
    #[serde(default)]
    pub notice: Option<String>,
    #[serde(default)]
    pub counts: Option<Counts>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub pdf: String,
    pub pdf_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Counts {
    pub entries: usize,
    pub approved: usize,
    pub unapproved: usize,
    pub dictionary_pages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Entry {
    /// As shown in the spec, in lower case, for example `close` or `prevent (from)`.
    #[serde(default)]
    pub headword: String,
    /// The word without its qualifier, for example `prevent`.
    pub lemma: String,
    pub pos: String,
    /// "approved" or "unapproved".
    pub status: String,
    /// Approved words only: the headword and the inflected forms that the spec shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forms: Option<Vec<String>>,
    /// Approved words only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meanings: Option<Vec<Meaning>>,
    /// Unapproved words: the approved alternatives. Approved words: the
    /// alternatives below `For other meanings, use:`.
    #[serde(default)]
    pub alternatives: Vec<Alternative>,
    #[serde(default)]
    pub notes: Vec<String>,
    /// Page of the source document.
    #[serde(default)]
    pub page: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Meaning {
    pub number: u32,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub ste_example: String,
    #[serde(default)]
    pub non_ste_example: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Alternative {
    #[serde(default)]
    pub text: String,
    pub word: String,
    #[serde(default)]
    pub pos: String,
    #[serde(default)]
    pub ste_example: String,
    #[serde(default)]
    pub non_ste_example: String,
}

impl Vocabulary {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read vocabulary {}: {e}", path.display()))?;
        Self::parse(&text)
            .map_err(|e| format!("{} is not a memelint vocabulary: {e}", path.display()))
    }

    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// `$XDG_CACHE_HOME/memelint`, or `~/.cache/memelint`.
pub fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("memelint"))
}

/// Where the vocabulary for a standard is written by default:
/// `<cache_dir>/<standard>.json`.
pub fn default_path(standard: &str) -> Option<PathBuf> {
    Some(cache_dir()?.join(format!("{standard}.json")))
}

/// The vocabulary file to read: a path from the command line, then
/// `$MEMELINT_VOCAB`, then the default location, which must be there.
pub fn locate(explicit: Option<&Path>, standard: &str) -> Result<PathBuf, String> {
    if let Some(p) = explicit {
        return Ok(p.to_path_buf());
    }
    if let Some(p) = std::env::var_os(ENV_VOCAB) {
        return Ok(PathBuf::from(p));
    }
    let default = default_path(standard)
        .ok_or("no cache directory: neither XDG_CACHE_HOME nor HOME is set")?;
    if default.is_file() {
        return Ok(default);
    }
    Err(format!(
        "no vocabulary at {}; run `memelint-import {standard}` first, or pass --vocab or set {ENV_VOCAB}",
        default.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_file_parses() {
        // Entries with words that are not English, and only the fields that a
        // small file has.
        let v = Vocabulary::parse(
            r#"{"issue": 0, "entries": [
              {"lemma": "frob", "pos": "v", "status": "approved", "forms": ["frob"]},
              {"lemma": "zorch", "pos": "v", "status": "unapproved", "alternatives": [{"word": "frob"}]}
            ]}"#,
        )
        .unwrap();
        assert_eq!(v.entries.len(), 2);
        assert_eq!(v.entries[1].alternatives[0].word, "frob");
        assert!(v.standard.is_empty());
    }

    #[test]
    fn explicit_path_wins() {
        let p = Path::new("/somewhere/v.json");
        assert_eq!(locate(Some(p), ASD_STE100).unwrap(), p);
    }
}
