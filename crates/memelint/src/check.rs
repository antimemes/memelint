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
    /// A replacement that removes the finding, when there is a mechanical one.
    pub fix: Option<Fix>,
}

/// A replacement of a byte range of the source file.
#[derive(Debug, PartialEq, Clone)]
pub struct Fix {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    /// A safe fix keeps the meaning: a contraction written in full. An unsafe
    /// fix replaces a word with the one alternative that the spec gives.
    pub safe: bool,
}

/// Words that the project examined and accepted: technical names and verbs
/// in the sense of the spec. They come from `memelint.toml`.
#[derive(Debug, Default, Clone)]
pub struct Allowlist(HashSet<String>);

impl Allowlist {
    /// Words with whitespace or line breaks between them. Text after `#` is
    /// a comment.
    #[cfg(test)]
    pub fn parse(text: &str) -> Self {
        let mut allow = Self::default();
        allow.extend(text);
        allow
    }

    pub fn extend(&mut self, text: &str) {
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("");
            self.0
                .extend(line.split_whitespace().map(str::to_lowercase));
        }
    }

    /// The word, or a regular inflection of it: the plural, or the -s and
    /// -ed forms of a technical verb. Not the -ing form: STE does not let you
    /// use it (rule 3.5). A technical name that ends in -ing is listed as a
    /// word. The regular spelling rules apply. `-es` comes after `s`, `x`,
    /// `z`, `ch` and `sh`. `-d` comes after `e`. `-ies` and `-ied` come after
    /// a `y` that is removed. `-s` does not come after `s`, `x`, `z`, `ch` or
    /// `sh`. Before `-ed`, some stems get their last letter again, as in
    /// `pinned`. memelint does not know which stems. Thus it accepts
    /// `pinned` and `pined`.
    /// The word is in the list, with no inflection.
    pub fn allows_exactly(&self, word: &str) -> bool {
        self.0.contains(word)
    }

    pub fn allows(&self, word: &str) -> bool {
        if self.0.contains(word) {
            return true;
        }
        let listed = |stem: &str| self.0.contains(stem);
        let sibilant = |stem: &str| {
            ["s", "x", "z", "ch", "sh"]
                .iter()
                .any(|e| stem.ends_with(e))
        };
        // `pinn` for the listed `pin`.
        let doubled = |stem: &str| {
            let mut chars = stem.chars().rev();
            matches!((chars.next(), chars.next()), (Some(a), Some(b)) if a == b && !"aeiou".contains(a))
                && listed(&stem[..stem.len() - 1])
        };
        word.strip_suffix("es")
            .is_some_and(|stem| sibilant(stem) && listed(stem))
            || word
                .strip_suffix('s')
                .is_some_and(|stem| !sibilant(stem) && listed(stem))
            || word
                .strip_suffix("ed")
                .is_some_and(|stem| !stem.ends_with('e') && (listed(stem) || doubled(stem)))
            || word
                .strip_suffix('d')
                .is_some_and(|stem| stem.ends_with('e') && listed(stem))
            || ["ies", "ied"].iter().any(|suffix| {
                word.strip_suffix(suffix)
                    .is_some_and(|stem| listed(&format!("{stem}y")))
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

/// The contraction written in full, for example `will not` for `won't`.
/// None for the `'s` and `'d` forms: `it's` is `it is` or `it has`, and
/// `we'd` is `we would` or `we had`.
fn expand_contraction(word: &str) -> Option<String> {
    let w = word.replace('’', "'");
    let (base, full) = [
        ("n't", "not"),
        ("'re", "are"),
        ("'ll", "will"),
        ("'ve", "have"),
        ("'m", "am"),
    ]
    .iter()
    .find_map(|(suffix, full)| w.strip_suffix(suffix).map(|base| (base, *full)))?;
    if base.is_empty() {
        return None;
    }
    Some(match (base, full) {
        ("wo", "not") => "will not".to_string(),
        ("ca", "not") => "cannot".to_string(),
        ("sha", "not") => "shall not".to_string(),
        _ => format!("{base} {full}"),
    })
}

/// Each word of `text` is approved or in the allowlist.
fn approved_text(text: &str, dict: &Dictionary, allow: &Allowlist) -> bool {
    text.split_whitespace()
        .all(|w| !dict.approved_pos(w).is_empty() || allow.allows(w))
}

/// `replacement` with an upper-case first letter when `original` has one.
fn recase(original: &str, replacement: &str) -> String {
    if !original.chars().next().is_some_and(char::is_uppercase) {
        return replacement.to_string();
    }
    let mut chars = replacement.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// The byte range in `text` of the word of `tok`: the token without the
/// quotation marks and parentheses at its ends, as in `word_of`.
fn word_range(text: &str, tok: &Token) -> (usize, usize) {
    let raw = text[tok.start..]
        .split(char::is_whitespace)
        .next()
        .unwrap_or("");
    let not_word = |c: char| !c.is_alphanumeric();
    let lead = raw.len() - raw.trim_start_matches(not_word).len();
    let start = tok.start + lead;
    (start, start + raw.trim_matches(not_word).len())
}

/// The fix for a contraction: the words in full. There is no fix when a
/// word is not approved, or when the token is on more than one line.
fn contraction_fix(
    text: &str,
    tok: &Token,
    comment: &Comment,
    dict: &Dictionary,
    allow: &Allowlist,
) -> Option<Fix> {
    let full = expand_contraction(tok.word.as_deref()?)?;
    if !approved_text(&full, dict, allow) {
        return None;
    }
    let (start, end) = word_range(text, tok);
    let (src_start, src_end) = comment.source_range(start, end)?;
    Some(Fix {
        start: src_start,
        end: src_end,
        replacement: recase(&text[start..end], &full),
        safe: true,
    })
}

/// The fix that writes `alt` for `text[start..end]`. It is not safe: the
/// alternatives of the spec are not synonyms in all contexts. There is no fix
/// when the range is on more than one line.
fn alternative_fix(
    text: &str,
    (start, end): (usize, usize),
    comment: &Comment,
    alt: &str,
) -> Option<Fix> {
    let (src_start, src_end) = comment.source_range(start, end)?;
    Some(Fix {
        start: src_start,
        end: src_end,
        replacement: recase(&text[start..end], alt),
        safe: false,
    })
}

/// The alternative to write for an unapproved word: the spec gives one
/// alternative with the same part of speech, in one sense of the word.
fn one_alternative(senses: &[Sense]) -> Option<&str> {
    match senses {
        [sense] => sense.same_pos_alternative.as_deref(),
        _ => None,
    }
}

/// The alternative with the inflection of `word`, for example `lifted` for
/// `hoisted`. Only a form that the dictionary lists is accepted. Thus there
/// is no fix when the regular spelling rules do not give the form.
fn inflected_alternative(word: &str, lemma: &str, alt: &str, dict: &Dictionary) -> Option<String> {
    let in_dictionary = |text: &str| {
        text.split_whitespace()
            .all(|w| !dict.approved_pos(w).is_empty())
    };
    if word == lemma {
        return in_dictionary(alt).then(|| alt.to_string());
    }
    let suffix = word.strip_prefix(lemma)?;
    let mut candidates = vec![
        format!("{alt}{suffix}"),
        format!("{alt}e{suffix}"),
        format!("{}{suffix}", alt.strip_suffix('e').unwrap_or(alt)),
    ];
    // `-es` after `s`, `x`, `z`, `ch` and `sh` is `-s` after other letters.
    if suffix == "es" {
        candidates.push(format!("{alt}s"));
    }
    candidates.into_iter().find(|c| in_dictionary(c))
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

/// The -ing form of an approved verb or of a technical verb in the
/// allowlist, for example `closing`, `running` and `parsing`.
fn ing_verb(word: &str, dict: &Dictionary, allow: &Allowlist) -> Option<String> {
    let stem = word.strip_suffix("ing")?;
    if stem.len() < 2 {
        return None;
    }
    let candidates = [
        stem.to_string(),
        format!("{stem}e"),
        stem[..stem.len() - 1].to_string(), // a stem that has its last letter 2 times
    ];
    candidates
        .into_iter()
        .find(|c| dict.is_verb(c) || allow.allows_exactly(c))
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
            fix: None,
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
            let mut f = finding(
                "4.2",
                tok.start,
                format!("'{word}' is a contraction; write the words in full"),
            );
            f.fix = contraction_fix(&text, tok, comment, dict, allow);
            findings.push(f);
            continue;
        }
        // Possessive: `the pump's inlet` is about the `pump`.
        let full = word;
        let word = word
            .strip_suffix("'s")
            .or_else(|| word.strip_suffix("’s"))
            .unwrap_or(word);
        let possessive = full.len() - word.len();

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
                let senses = dict.unapproved(&phrase);
                let mut f = finding(
                    "1.1",
                    tok.start,
                    unapproved_message(&phrase, &phrase, senses),
                );
                if let Some(alt) = one_alternative(senses)
                    && approved_text(alt, dict, allow)
                {
                    let last = &tokens[i + len - 1];
                    let range = (word_range(&text, tok).0, word_range(&text, last).1);
                    f.fix = alternative_fix(&text, range, comment, alt);
                }
                findings.push(f);
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
        let one_part = parts.len() == 1;
        for part in parts {
            if let Some(verdict) = judge(part, prev, next, tok.starts_sentence, dict, allow) {
                let mut f = finding(verdict.rule, tok.start, verdict.message);
                if let (true, Some(alt)) = (one_part, verdict.replacement) {
                    // The range is the word without the possessive `'s`.
                    let (start, end) = word_range(&text, tok);
                    f.fix = alternative_fix(&text, (start, end - possessive), comment, &alt);
                }
                findings.push(f);
                break;
            }
        }
    }

    findings.sort_by_key(|f| (f.line, f.col));
    findings
}

/// The rule that a word breaks, as `judge` finds it.
struct Verdict {
    rule: &'static str,
    message: String,
    /// The word to write for it, in lower case, when the spec gives one
    /// alternative with the same part of speech (rule 1.1).
    replacement: Option<String>,
}

/// The rule and message for one word, if it breaks a rule.
fn judge(
    word: &str,
    prev: Option<&str>,
    next: Option<&str>,
    starts_sentence: bool,
    dict: &Dictionary,
    allow: &Allowlist,
) -> Option<Verdict> {
    if allow.allows(word) {
        return None;
    }
    let verdict = |rule, message, replacement| {
        Some(Verdict {
            rule,
            message,
            replacement,
        })
    };
    let approved = dict.approved_pos(word);
    if !approved.is_empty() {
        let senses = dict.unapproved(word);
        if senses.is_empty() {
            return None;
        }
        let unapproved_pos: Vec<String> = senses.iter().map(|s| s.pos.clone()).collect();
        let pos = used_as_unapproved(approved, &unapproved_pos, prev, next, starts_sentence)?;
        let sense = senses.iter().find(|s| s.pos == pos).unwrap();
        return verdict(
            "1.2",
            format!(
                "'{word}' is approved only as {}; as {} use {}",
                pos_names(approved),
                pos_name(&pos),
                list(&sense.alternatives)
            ),
            None,
        );
    }

    if let Some((lemma, senses)) = dict.unapproved_lemma(word) {
        let replacement =
            one_alternative(senses).and_then(|alt| inflected_alternative(word, lemma, alt, dict));
        return verdict("1.1", unapproved_message(word, lemma, senses), replacement);
    }

    if let Some(verb) = ing_verb(word, dict, allow) {
        return verdict(
            "3.5",
            format!(
                "'{word}' is the -ing form of '{verb}'; STE allows only the infinitive, imperative, past tense and past participle"
            ),
            None,
        );
    }

    verdict(
        "1.6",
        format!(
            "'{word}' is not in the STE dictionary; if it is a technical name, add it to {CONFIG_FILE}"
        ),
        None,
    )
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
          {"lemma": "do", "pos": "v", "status": "approved", "forms": ["do", "does", "did"]},
          {"lemma": "be", "pos": "v", "status": "approved", "forms": ["be", "are"]},
          {"lemma": "have", "pos": "v", "status": "approved", "forms": ["have", "has", "had"]},
          {"lemma": "will", "pos": "v", "status": "approved", "forms": ["will"]},
          {"lemma": "cannot", "pos": "v", "status": "approved", "forms": ["cannot"]},
          {"lemma": "not", "pos": "adv", "status": "approved", "forms": ["not"]},
          {"lemma": "hoist", "pos": "v", "status": "unapproved", "alternatives": [{"word": "lift", "pos": "v"}]},
          {"lemma": "zorch up", "pos": "v", "status": "unapproved", "alternatives": [{"word": "lift", "pos": "v"}]},
          {"lemma": "blorp", "pos": "v", "status": "unapproved", "alternatives": [{"word": "blorp", "pos": "TN"}]},
          {"lemma": "glimb", "pos": "n", "status": "unapproved", "alternatives": [{"word": "lift", "pos": "v"}]},
          {"lemma": "blish", "pos": "v", "status": "unapproved", "alternatives": [{"word": "frob", "pos": "v"}]},
          {"lemma": "plonk", "pos": "v", "status": "unapproved", "alternatives": [{"word": "lift", "pos": "v"}, {"word": "frob", "pos": "v"}]}
        ]}"#,
        )
        .unwrap()
    }

    /// A comment with `text` as its source. Thus `byte` is the offset of
    /// each line in `text`.
    fn comment(text: &str) -> Comment {
        let mut byte = 0;
        Comment {
            lines: text
                .split('\n')
                .enumerate()
                .map(|(i, t)| {
                    let line = Line {
                        line: i + 1,
                        col: 1,
                        byte,
                        text: t.to_string(),
                    };
                    byte += t.len() + 1;
                    line
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

    /// The fix of the one finding of `rule` in `text`.
    fn fix_of(text: &str, rule: &str) -> Option<Fix> {
        fix_of_allowed(text, rule, "")
    }

    fn fix_of_allowed(text: &str, rule: &str, allow: &str) -> Option<Fix> {
        let findings = check(&comment(text), &dict(), &Allowlist::parse(allow));
        let mut hits = findings.into_iter().filter(|f| f.rule == rule);
        let fix = hits.next().expect("a finding of the rule").fix;
        assert!(hits.next().is_none(), "one finding of the rule");
        fix
    }

    fn contraction_fix_of(text: &str) -> Option<Fix> {
        fix_of(text, "4.2")
    }

    #[test]
    fn contraction_fix_writes_the_words_in_full() {
        let text = "Don't frob the gantry.";
        assert_eq!(
            contraction_fix_of(text),
            Some(Fix {
                start: 0,
                end: "Don't".len(),
                replacement: "Do not".to_string(),
                safe: true,
            })
        );
        assert_eq!(expand_contraction("doesn't").as_deref(), Some("does not"));
        assert_eq!(expand_contraction("we’re").as_deref(), Some("we are"));
        assert_eq!(expand_contraction("they'll").as_deref(), Some("they will"));
        assert_eq!(expand_contraction("i've").as_deref(), Some("i have"));
    }

    #[test]
    fn contraction_fix_excludes_quotation_marks() {
        let text = "Frob 'won't' the gantry.";
        let fix = contraction_fix_of(text).unwrap();
        assert_eq!(&text[fix.start..fix.end], "won't");
        assert_eq!(fix.replacement, "will not");
        assert_eq!(
            contraction_fix_of("The gantry can't lift.")
                .unwrap()
                .replacement,
            "cannot"
        );
    }

    #[test]
    fn ambiguous_or_unapproved_contractions_get_no_fix() {
        // `it's` is `it is` or `it has`.
        assert_eq!(contraction_fix_of("It's wide."), None);
        // `shall` is not in the test dictionary.
        assert_eq!(contraction_fix_of("Frob, shan't we."), None);
    }

    #[test]
    fn contraction_fix_on_a_later_line_maps_to_the_source() {
        let text = "Frob the gantry.\nThe gantry doesn't lift.";
        let fix = contraction_fix_of(text).unwrap();
        assert_eq!(&text[fix.start..fix.end], "doesn't");
        assert_eq!(fix.replacement, "does not");
    }

    #[test]
    fn alternative_fix_replaces_the_word() {
        assert_eq!(
            fix_of("Hoist the gantry.", "1.1"),
            Some(Fix {
                start: 0,
                end: "Hoist".len(),
                replacement: "Lift".to_string(),
                safe: false,
            })
        );
        // An inflection of the word gets the same inflection of the alternative.
        assert_eq!(
            fix_of("The gantry hoisted.", "1.1").unwrap().replacement,
            "lifted"
        );
    }

    #[test]
    fn alternative_fix_inflects_the_alternative() {
        // `blishes` is `blish` + `es`. The form of the alternative `frob` is
        // `frobs`, which the dictionary lists, and not `frobes`.
        assert_eq!(
            fix_of("The gantry blishes.", "1.1").unwrap().replacement,
            "frobs"
        );
        // The allowlist does not make a form approved, also when it has the
        // alternative.
        assert_eq!(
            fix_of_allowed("The gantry blishes.", "1.1", "frob")
                .unwrap()
                .replacement,
            "frobs"
        );
        // `frobbed` has `bb`, which the rules do not give. No fix is better
        // than an incorrect fix.
        assert_eq!(fix_of("The gantry blished.", "1.1"), None);
    }

    #[test]
    fn ing_form_of_a_technical_verb_is_a_finding() {
        // `zib` is a technical verb of the project. Its -ing form is rule 3.5,
        // not an unknown word, and the allowlist does not accept it.
        let msgs = messages("Zibbing the gantry.", "zib");
        assert_eq!(
            msgs,
            [
                "'zibbing' is the -ing form of 'zib'; STE allows only the infinitive, imperative, past tense and past participle"
            ]
        );
        assert!(messages("Zib the gantry. The gantry is zibbed.", "zib").is_empty());
        // An -ing technical name that is listed as a word gets no finding.
        assert!(messages("The zibbing is wide.", "zibbing").is_empty());
    }

    #[test]
    fn allowlist_inflections_follow_spelling_rules() {
        let allow = Allowlist::parse("set box frobe gantry pin");
        for word in [
            "sets", "boxes", "frobed", "gantries", "gantried", "pinned", "pined",
        ] {
            assert!(allow.allows(word), "{word}");
        }
        // No -ing forms: STE rule 3.5. `seeed` is not `see` + `ed`.
        for word in [
            "setes", "setd", "frobeed", "boxs", "boxing", "pinning", "seeed",
        ] {
            assert!(!allow.allows(word), "{word}");
        }
    }

    #[test]
    fn alternative_fix_replaces_the_phrase() {
        let text = "Zorch up the gantry.";
        let fix = fix_of(text, "1.1").unwrap();
        assert_eq!(&text[fix.start..fix.end], "Zorch up");
        assert_eq!(fix.replacement, "Lift");
        assert!(!fix.safe);
        // A phrase on more than one line has no fix.
        assert_eq!(fix_of("Zorch\nup the gantry.", "1.1"), None);
    }

    #[test]
    fn alternative_fix_keeps_the_possessive() {
        let text = "The hoist's gantry.";
        let fix = fix_of(text, "1.1").unwrap();
        assert_eq!(&text[fix.start..fix.end], "hoist");
        assert_eq!(fix.replacement, "lift");
    }

    #[test]
    fn words_without_one_alternative_of_the_same_part_of_speech_get_no_fix() {
        // The alternative is a verb; the word is a noun.
        assert_eq!(fix_of("The glimb is wide.", "1.1"), None);
        // More than one alternative.
        assert_eq!(fix_of("Plonk the gantry.", "1.1"), None);
        // A word with a hyphen.
        assert_eq!(fix_of("Frob-hoist the gantry.", "1.1"), None);
        // The spec points at the word as a technical name.
        assert_eq!(fix_of("Blorp the gantry.", "1.1"), None);
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
