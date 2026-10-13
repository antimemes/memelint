//! Word lookups for the checker, made from a vocabulary file.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use memelint_common::Vocabulary;

/// One unapproved use of a word.
pub struct Sense {
    /// The part of speech it is unapproved as.
    pub pos: String,
    /// Approved words to use for it.
    pub alternatives: Vec<String>,
    /// The parts of speech in which the same word is approved, when the spec
    /// points back at it. `n` is a noun, `TN` is a technical name, and `TV` is
    /// a technical verb.
    pub same_word_as: Vec<String>,
}

pub struct Dictionary {
    pub issue: Option<u32>,
    /// Each approved form (lower case) and the parts of speech it is approved as.
    approved: HashMap<String, Vec<String>>,
    /// Each unapproved word or phrase (lower case) and its unapproved uses.
    unapproved: HashMap<String, Vec<Sense>>,
    /// The lemmas of the approved verbs, to find their -ing forms.
    verbs: HashSet<String>,
}

impl Dictionary {
    pub fn load(path: &Path) -> Result<Self> {
        let vocab = Vocabulary::load(path).map_err(anyhow::Error::msg)?;
        Ok(Self::from_vocabulary(vocab))
    }

    #[cfg(test)]
    pub fn from_json(text: &str) -> Result<Self> {
        Ok(Self::from_vocabulary(Vocabulary::parse(text)?))
    }

    pub fn from_vocabulary(vocab: Vocabulary) -> Self {
        let mut dict = Dictionary {
            issue: vocab.issue,
            approved: HashMap::new(),
            unapproved: HashMap::new(),
            verbs: HashSet::new(),
        };
        for e in vocab.entries {
            if e.status == "approved" {
                for form in e.forms.iter().flatten().chain(std::iter::once(&e.lemma)) {
                    dict.add_approved(form, &e.pos);
                    // The spec shows the verb forms, but it does not show the noun plurals.
                    if e.pos == "n" {
                        dict.add_approved(&plural(form), &e.pos);
                    }
                }
                if e.pos == "v" {
                    dict.verbs.insert(e.lemma.clone());
                }
            } else {
                let mut sense = Sense {
                    pos: e.pos,
                    alternatives: Vec::new(),
                    same_word_as: Vec::new(),
                };
                for a in e.alternatives {
                    if a.word.is_empty() {
                        continue;
                    }
                    if a.word == e.lemma {
                        sense.same_word_as.push(a.pos);
                    } else {
                        sense.alternatives.push(a.word);
                    }
                }
                dict.unapproved.entry(e.lemma).or_default().push(sense);
            }
        }
        dict
    }

    fn add_approved(&mut self, form: &str, pos: &str) {
        let list = self.approved.entry(form.to_string()).or_default();
        if !list.iter().any(|p| p == pos) {
            list.push(pos.to_string());
        }
    }

    /// Parts of speech `form` is approved as; empty if it is not approved.
    pub fn approved_pos(&self, form: &str) -> &[String] {
        self.approved.get(form).map_or(&[], Vec::as_slice)
    }

    /// The unapproved uses of `word`, which can be a phrase with spaces.
    pub fn unapproved(&self, word: &str) -> &[Sense] {
        self.unapproved.get(word).map_or(&[], Vec::as_slice)
    }

    /// The unapproved uses of `word`, or of the lemma of which it is an
    /// inflection, for example `returns` or `returned` for the unapproved
    /// verb `return`.
    pub fn unapproved_lemma(&self, word: &str) -> Option<(&str, &[Sense])> {
        if let Some((k, v)) = self.unapproved.get_key_value(word) {
            return Some((k, v));
        }
        for suffix in ["s", "es", "ed", "d", "ing"] {
            let Some(stem) = word.strip_suffix(suffix) else {
                continue;
            };
            if let Some((k, v)) = self.unapproved.get_key_value(stem) {
                return Some((k, v));
            }
        }
        None
    }

    pub fn is_verb(&self, lemma: &str) -> bool {
        self.verbs.contains(lemma)
    }
}

/// Regular English plural.
fn plural(noun: &str) -> String {
    let ends = |s: &str| noun.ends_with(s);
    if ends("s") || ends("x") || ends("z") || ends("ch") || ends("sh") {
        format!("{noun}es")
    } else if ends("y") && !ends("ay") && !ends("ey") && !ends("oy") && !ends("uy") {
        format!("{}ies", &noun[..noun.len() - 1])
    } else {
        format!("{noun}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plurals() {
        assert_eq!(plural("widget"), "widgets");
        assert_eq!(plural("box"), "boxes");
        assert_eq!(plural("latch"), "latches");
        assert_eq!(plural("pulley"), "pulleys");
        assert_eq!(plural("gantry"), "gantries");
    }

    #[test]
    fn lookups() {
        // Entries in the shape of the vocabulary file. The words are not English.
        let d = Dictionary::from_json(
            r#"{"issue": 0, "entries": [
            {"lemma": "frob", "pos": "v", "status": "approved", "forms": ["frob", "frobs", "frobbed"]},
            {"lemma": "frob", "pos": "adj", "status": "unapproved", "alternatives": [{"word": "wide", "pos": "adj"}]},
            {"lemma": "gantry", "pos": "n", "status": "approved", "forms": ["gantry"]},
            {"lemma": "zorch up", "pos": "v", "status": "unapproved", "alternatives": [{"word": "lift", "pos": "v"}]},
            {"lemma": "blorp", "pos": "v", "status": "unapproved", "alternatives": [{"word": "blorp", "pos": "TN"}]}
            ]}"#,
        )
        .unwrap();

        assert_eq!(d.approved_pos("frobbed"), ["v"]);
        assert_eq!(d.approved_pos("gantries"), ["n"]);
        assert!(d.approved_pos("frobbing").is_empty());
        assert!(d.is_verb("frob"));
        assert_eq!(d.unapproved("frob")[0].alternatives, ["wide"]);
        assert_eq!(d.unapproved("zorch up")[0].alternatives, ["lift"]);
        assert!(d.unapproved("gantry").is_empty());
        assert_eq!(d.unapproved_lemma("blorped").unwrap().0, "blorp");
        assert!(d.unapproved("blorp")[0].alternatives.is_empty());
        assert_eq!(d.unapproved("blorp")[0].same_word_as, ["TN"]);
    }
}
