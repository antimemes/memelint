//! The rules. A closed vocabulary with a project allowlist. A small
//! part-of-speech check for words that are approved in one part of speech
//! only. Contractions. The -ing form of approved verbs. Sentence length.

use std::collections::HashSet;

use crate::config::CONFIG_FILE;
use crate::dict::{Dictionary, Sense};
use crate::extract::Comment;

/// STE limits descriptive sentences to 25 words and procedural sentences to
/// 20 words. Code comments are descriptive.
pub const MAX_SENTENCE_WORDS: usize = 25;

#[derive(Debug, PartialEq)]
pub struct Finding {
    /// The ASD-STE100 rule the finding is about, e.g. "1.1"; see `rules.rs`.
    pub rule: &'static str,
    pub line: usize,
    pub col: usize,
    pub message: String,
}

/// Words that the project examined and accepted: technical names and verbs
/// in the sense of the spec. They come from `memelint.toml`.
#[derive(Default)]
pub struct Allowlist(HashSet<String>);

impl Allowlist {
    pub fn parse(text: &str) -> Self {
        Self(
            text.lines()
                .map(|l| l.split('#').next().unwrap_or("").trim().to_lowercase())
                .filter(|l| !l.is_empty())
                .collect(),
        )
    }

    /// The word, or a regular inflection of it: the plural, or the -s, -ed
    /// and -ing forms of a technical verb.
    pub fn allows(&self, word: &str) -> bool {
        if self.0.contains(word) {
            return true;
        }
        ["es", "s", "ed", "d", "ing"].iter().any(|s| {
            word.strip_suffix(s)
                .is_some_and(|stem| self.0.contains(stem))
        }) || ["ies", "ied"].iter().any(|s| {
            word.strip_suffix(s)
                .is_some_and(|stem| self.0.contains(&format!("{stem}y")))
        })
    }
}

struct Token {
    /// Byte offset of the token in the comment text.
    start: usize,
    /// The token as a lower-case word, if it is one (not an identifier,
    /// number, acronym or one letter).
    word: Option<String>,
    starts_sentence: bool,
}

fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut sentence_start = true;
    let mut pos = 0;
    for raw in text.split(|c: char| c.is_whitespace()) {
        let start = pos;
        pos += raw.len() + 1;
        if raw.is_empty() {
            continue;
        }
        tokens.push(Token {
            start,
            word: word_of(raw),
            starts_sentence: sentence_start,
        });
        sentence_start = ends_sentence(raw);
    }
    tokens
}

fn ends_sentence(raw: &str) -> bool {
    let t = raw.trim_end_matches(['"', '\'', ')', ']', '”', '’']);
    t.ends_with(['.', '!', '?']) && !matches!(t, "e.g." | "i.e." | "etc." | "vs." | "cf.")
}

fn word_of(raw: &str) -> Option<String> {
    // A suffix or a file extension given as a token, for example `-ing` or
    // `.so`, is not a word.
    if raw.starts_with(['-', '.']) {
        return None;
    }
    // Quotation marks and apostrophes at the ends are not part of the word.
    let t = raw.trim_matches(|c: char| !c.is_alphanumeric());
    let mut chars = t.chars();
    let first = chars.next()?;
    if !first.is_alphabetic() {
        return None;
    }
    // Identifiers, paths, numbers, acronyms and CamelCase are code, not prose.
    if chars
        .clone()
        .any(|c| c.is_uppercase() || c.is_ascii_digit() || "_./\\:=<>@#()[]".contains(c))
    {
        return None;
    }
    chars.next()?;
    Some(t.to_lowercase())
}

/// Contractions, for example `don't` and `we'll`, but not the possessive
/// `pump's`.
fn is_contraction(word: &str) -> bool {
    let w = word.replace('’', "'");
    ["n't", "'re", "'ll", "'ve", "'d", "'m"]
        .iter()
        .any(|s| w.ends_with(s))
        || matches!(
            w.as_str(),
            "it's"
                | "let's"
                | "that's"
                | "what's"
                | "there's"
                | "here's"
                | "who's"
                | "how's"
                | "where's"
                | "when's"
        )
}

/// Context rule for a word that is approved as some parts of speech and
/// unapproved as other parts of speech. The result is the unapproved part of
/// speech that the context shows, if there is one.
fn used_as_unapproved(
    approved: &[String],
    unapproved: &[String],
    prev: Option<&str>,
    next: Option<&str>,
    starts_sentence: bool,
) -> Option<String> {
    const VERB_CUES: &[&str] = &[
        "to", "not", "you", "we", "they", "it", "can", "cannot", "must", "will", "do", "does",
        "did", "and", "or", "then",
    ];
    const NOUN_CUES: &[&str] = &[
        "the", "a", "an", "this", "that", "these", "those", "its", "your", "our", "their", "each",
        "no", "of", "one",
    ];
    let has = |list: &[String], pos: &str| list.iter().any(|p| p == pos);

    // `close to` is the adjective or the adverb. The other parts of speech of
    // `close` do not apply.
    if next == Some("to")
        && (has(unapproved, "adj") || has(unapproved, "adv"))
        && !has(approved, "adj")
    {
        return unapproved
            .iter()
            .find(|p| *p == "adj" || *p == "adv")
            .cloned();
    }
    if has(approved, "v") && (starts_sentence || prev.is_some_and(|p| VERB_CUES.contains(&p))) {
        return None;
    }
    if has(approved, "n") && prev.is_some_and(|p| NOUN_CUES.contains(&p)) {
        return None;
    }
    // Not sure: do not flag.
    None
}

/// Remove the words that occur more than one time. Keep the first of each.
fn dedup(words: &mut Vec<String>) {
    let mut seen = HashSet::new();
    words.retain(|w| seen.insert(w.clone()));
}

fn list(words: &[String]) -> String {
    words
        .iter()
        .map(|w| format!("'{w}'"))
        .collect::<Vec<_>>()
        .join(" or ")
}

fn pos_name(pos: &str) -> &str {
    match pos {
        "v" => "a verb",
        "n" => "a noun",
        "adj" => "an adjective",
        "adv" => "an adverb",
        "prep" => "a preposition",
        "pron" => "a pronoun",
        "conj" => "a conjunction",
        "art" => "an article",
        "prefix" => "a prefix",
        "TN" => "a technical name",
        "TV" => "a technical verb",
        _ => "a word",
    }
}

fn pos_names(list: &[String]) -> String {
    list.iter()
        .map(|p| pos_name(p))
        .collect::<Vec<_>>()
        .join(" or ")
}

/// The message for an unapproved word. It gives the alternatives, and the
/// parts of speech in which the same word is approved, for example as a
/// technical name.
fn unapproved_message(word: &str, lemma: &str, senses: &[Sense]) -> String {
    let shown = if word == lemma {
        format!("'{word}'")
    } else {
        format!("'{word}' ('{lemma}')")
    };
    let mut alts: Vec<String> = senses.iter().flat_map(|s| s.alternatives.clone()).collect();
    let mut same: Vec<String> = senses.iter().flat_map(|s| s.same_word_as.clone()).collect();
    dedup(&mut alts);
    dedup(&mut same);
    let mut message = format!("{shown} is not approved");
    if !alts.is_empty() {
        message.push_str(&format!("; use {}", list(&alts)));
    }
    if !same.is_empty() {
        message.push_str(&format!("; it is approved only as {}", pos_names(&same)));
        if same.iter().any(|p| p == "TN" || p == "TV") {
            message.push_str(&format!(" (add it to {CONFIG_FILE} if it is one here)"));
        }
    }
    message
}

/// The -ing form of an approved verb, for example `closing` and `running`.
fn ing_verb(word: &str, dict: &Dictionary) -> Option<String> {
    let stem = word.strip_suffix("ing")?;
    if stem.len() < 2 {
        return None;
    }
    let candidates = [
        stem.to_string(),
        format!("{stem}e"),
        stem[..stem.len() - 1].to_string(), // a stem that has its last letter 2 times
    ];
    candidates.into_iter().find(|c| dict.is_verb(c))
}

pub fn check(comment: &Comment, dict: &Dictionary, allow: &Allowlist) -> Vec<Finding> {
    let text = comment.text();
    let tokens = tokenize(&text);
    let mut findings = Vec::new();
    let finding = |rule: &'static str, start: usize, message: String| {
        let (line, col) = comment.position(start);
        Finding {
            rule,
            line,
            col,
            message,
        }
    };

    // Sentence length.
    let mut sentence_start = 0;
    for i in 0..=tokens.len() {
        if i == tokens.len() || (i > 0 && tokens[i].starts_sentence) {
            let n = i - sentence_start;
            if n > MAX_SENTENCE_WORDS {
                findings.push(finding(
                    "6.3",
                    tokens[sentence_start].start,
                    format!("sentence has {n} words; the STE limit for descriptive text is {MAX_SENTENCE_WORDS}"),
                ));
            }
            sentence_start = i;
        }
    }

    // Vocabulary.
    let mut skip_until = 0;
    for (i, tok) in tokens.iter().enumerate() {
        if i < skip_until {
            continue;
        }
        let Some(word) = &tok.word else { continue };

        if is_contraction(word) {
            findings.push(finding(
                "4.2",
                tok.start,
                format!("'{word}' is a contraction; write the words in full"),
            ));
            continue;
        }
        // Possessive: `the pump's inlet` is about the `pump`.
        let word = word
            .strip_suffix("'s")
            .or_else(|| word.strip_suffix("’s"))
            .unwrap_or(word);

        // Unapproved phrases of 2 or 3 words.
        let phrase_hit = (2..=3).rev().find_map(|len| {
            let words: Option<Vec<&str>> = tokens
                .get(i..i + len)?
                .iter()
                .map(|t| t.word.as_deref())
                .collect();
            let phrase = words?.join(" ");
            (!dict.unapproved(&phrase).is_empty() && dict.approved_pos(&phrase).is_empty())
                .then_some((len, phrase))
        });
        if let Some((len, phrase)) = phrase_hit {
            if !allow.allows(&phrase) {
                findings.push(finding(
                    "1.1",
                    tok.start,
                    unapproved_message(&phrase, &phrase, dict.unapproved(&phrase)),
                ));
            }
            skip_until = i + len;
            continue;
        }

        // A word with hyphens: each part is examined, unless the full word is
        // in the allowlist.
        let parts: Vec<&str> = if allow.allows(word) {
            Vec::new()
        } else {
            word.split('-').filter(|p| !p.is_empty()).collect()
        };
        let prev = i.checked_sub(1).and_then(|j| tokens[j].word.as_deref());
        let next = tokens.get(i + 1).and_then(|t| t.word.as_deref());
        for part in parts {
            if let Some((rule, message)) = judge(part, prev, next, tok.starts_sentence, dict, allow)
            {
                findings.push(finding(rule, tok.start, message));
                break;
            }
        }
    }

    findings.sort_by_key(|f| (f.line, f.col));
    findings
}

/// The rule and message for one word, if it breaks a rule.
fn judge(
    word: &str,
    prev: Option<&str>,
    next: Option<&str>,
    starts_sentence: bool,
    dict: &Dictionary,
    allow: &Allowlist,
) -> Option<(&'static str, String)> {
    if allow.allows(word) {
        return None;
    }
    let approved = dict.approved_pos(word);
    if !approved.is_empty() {
        let senses = dict.unapproved(word);
        if senses.is_empty() {
            return None;
        }
        let unapproved_pos: Vec<String> = senses.iter().map(|s| s.pos.clone()).collect();
        let pos = used_as_unapproved(approved, &unapproved_pos, prev, next, starts_sentence)?;
        let sense = senses.iter().find(|s| s.pos == pos).unwrap();
        return Some((
            "1.2",
            format!(
                "'{word}' is approved only as {}; as {} use {}",
                pos_names(approved),
                pos_name(&pos),
                list(&sense.alternatives)
            ),
        ));
    }

    if let Some((lemma, senses)) = dict.unapproved_lemma(word) {
        return Some(("1.1", unapproved_message(word, lemma, senses)));
    }

    if let Some(verb) = ing_verb(word, dict) {
        return Some((
            "3.5",
            format!(
                "'{word}' is the -ing form of '{verb}'; STE allows only the infinitive, imperative, past tense and past participle"
            ),
        ));
    }

    Some((
        "1.6",
        format!(
            "'{word}' is not in the STE dictionary; if it is a technical name, add it to {CONFIG_FILE}"
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::Line;

    /// A dictionary in the shape of the vocabulary file. The words are not
    /// English words, and no entry comes from ASD-STE100.
    fn dict() -> Dictionary {
        Dictionary::from_json(
            r#"{"issue": 0, "entries": [
          {"lemma": "frob", "pos": "v", "status": "approved", "forms": ["frob", "frobs", "frobbed"]},
          {"lemma": "frob", "pos": "adj", "status": "unapproved", "alternatives": [{"word": "wide", "pos": "adj"}]},
          {"lemma": "lift", "pos": "v", "status": "approved", "forms": ["lift", "lifts", "lifted"]},
          {"lemma": "gantry", "pos": "n", "status": "approved", "forms": ["gantry"]},
          {"lemma": "the", "pos": "art", "status": "approved", "forms": ["the"]},
          {"lemma": "is", "pos": "v", "status": "approved", "forms": ["is"]},
          {"lemma": "to", "pos": "prep", "status": "approved", "forms": ["to"]},
          {"lemma": "wide", "pos": "adj", "status": "approved", "forms": ["wide"]},
          {"lemma": "hoist", "pos": "v", "status": "unapproved", "alternatives": [{"word": "lift", "pos": "v"}]},
          {"lemma": "zorch up", "pos": "v", "status": "unapproved", "alternatives": [{"word": "lift", "pos": "v"}]},
          {"lemma": "blorp", "pos": "v", "status": "unapproved", "alternatives": [{"word": "blorp", "pos": "TN"}]}
        ]}"#,
        )
        .unwrap()
    }

    fn comment(text: &str) -> Comment {
        Comment {
            lines: text
                .split('\n')
                .enumerate()
                .map(|(i, t)| Line {
                    line: i + 1,
                    col: 1,
                    text: t.to_string(),
                })
                .collect(),
        }
    }

    fn messages(text: &str, allow: &str) -> Vec<String> {
        check(&comment(text), &dict(), &Allowlist::parse(allow))
            .into_iter()
            .map(|f| f.message)
            .collect()
    }

    #[test]
    fn approved_text_passes() {
        assert!(messages("Frob the gantry. The gantries lift.", "").is_empty());
    }

    #[test]
    fn unapproved_word_gets_alternative() {
        assert_eq!(
            messages("Hoist the gantry.", ""),
            ["'hoist' is not approved; use 'lift'"]
        );
        assert_eq!(
            messages("The gantry hoisted.", ""),
            ["'hoisted' ('hoist') is not approved; use 'lift'"]
        );
    }

    #[test]
    fn unapproved_phrase_is_matched_whole() {
        assert_eq!(
            messages("Zorch up the gantry.", ""),
            ["'zorch up' is not approved; use 'lift'"]
        );
    }

    #[test]
    fn technical_name_pointer() {
        assert_eq!(
            messages("Blorp the gantry.", ""),
            [
                "'blorp' is not approved; it is approved only as a technical name (add it to memelint.toml if it is one here)"
            ]
        );
        assert!(messages("Blorp the gantry.", "blorp").is_empty());
    }

    #[test]
    fn part_of_speech_context() {
        // Imperative verb: correct.
        assert!(messages("Frob the gantry.", "").is_empty());
        // `frob to` reads as the adjective, which is unapproved.
        assert_eq!(
            messages("The gantry is frob to the wall.", "wall"),
            ["'frob' is approved only as a verb; as an adjective use 'wide'"]
        );
    }

    #[test]
    fn unknown_words_and_allowlist() {
        let msgs = messages("Lift the sprocket.", "");
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].starts_with("'sprocket' is not in the STE dictionary"));
        assert!(messages("Lift the sprockets.", "sprocket # part name").is_empty());
    }

    #[test]
    fn ing_forms_contractions_and_identifiers() {
        assert_eq!(
            messages("Frobbing is wide.", ""),
            [
                "'frobbing' is the -ing form of 'frob'; STE allows only the infinitive, imperative, past tense and past participle"
            ]
        );
        assert_eq!(
            messages("Don't frob.", ""),
            ["'don't' is a contraction; write the words in full"]
        );
        assert!(messages("The gantry's frob is wide.", "").is_empty());
        // Identifiers, acronyms, numbers and one-letter tokens are not words.
        assert!(messages("Lift gantry_id to GANTRY 2 (frob `x`).", "").is_empty());
    }

    #[test]
    fn long_sentences() {
        let long = ["frob"; 26].join(" ") + ".";
        assert_eq!(
            messages(&long, ""),
            ["sentence has 26 words; the STE limit for descriptive text is 25"]
        );
        let two = ["frob"; 20].join(" ") + ". " + &["lift"; 20].join(" ") + ".";
        assert!(messages(&two, "").is_empty());
    }

    #[test]
    fn every_finding_cites_a_known_rule() {
        let text = "Hoist the gantry. Don't frob. Frobbing is wide. Lift the sprocket. The gantry is frob to the wall.";
        let findings = check(&comment(text), &dict(), &Allowlist::default());
        assert!(findings.len() >= 5);
        for f in findings {
            assert!(
                crate::rules::find(f.rule).is_some(),
                "unknown rule {}",
                f.rule
            );
        }
    }

    #[test]
    fn findings_carry_positions() {
        let f = check(
            &comment("Lift the gantry.\nHoist it."),
            &dict(),
            &Allowlist::default(),
        );
        assert_eq!(f.len(), 2);
        assert_eq!((f[0].line, f[0].col), (2, 1));
    }
}
