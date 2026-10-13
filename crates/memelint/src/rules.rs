//! The 53 rules of ASD-STE100 Issue 9, and the relation of memelint to each
//! rule. This is the one source for `memelint rules` and for the rule ids in
//! the findings.
//!
//! The summaries are paraphrases, not the text of the spec.

#![allow(dead_code)] // The table is a reference. The tests read it.

/// Does memelint check the rule at this time?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Does {
    Yes,
    Partial,
    No,
}

/// How much of a rule a program can check. This does not change with the
/// code of memelint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feasibility {
    /// A lookup or a count. Fully checkable, with no judgment.
    Mechanical,
    /// Checkable with heuristics that give some incorrect findings.
    Heuristic,
    /// A person must make the decision: meaning and document structure.
    Judgment,
    /// Does not occur in code comments.
    NotApplicable,
}

pub struct Rule {
    pub id: &'static str,
    pub section: &'static str,
    pub summary: &'static str,
    pub does: Does,
    pub feasibility: Feasibility,
    /// The finding that memelint reports for it, if there is one.
    pub check: &'static str,
    pub note: &'static str,
}

use Does::{No as DNo, Partial as DPart, Yes as DYes};
use Feasibility::*;

const WORDS: &str = "1 Words";
const NOUNS: &str = "2 Multi-word nouns";
const VERBS: &str = "3 Verbs";
const SENTENCES: &str = "4 Sentences";
const PROCEDURES: &str = "5 Procedural writing";
const DESCRIPTIONS: &str = "6 Descriptive writing";
const SAFETY: &str = "7 Safety instructions";
const PUNCTUATION: &str = "8 Punctuation and word count";
const PRACTICES: &str = "9 Writing practices";

macro_rules! rule {
    ($id:literal, $section:ident, $summary:literal, $does:expr, $feasibility:expr, $check:literal, $note:literal) => {
        Rule {
            id: $id,
            section: $section,
            summary: $summary,
            does: $does,
            feasibility: $feasibility,
            check: $check,
            note: $note,
        }
    };
}

#[rustfmt::skip]
pub const RULES: &[Rule] = &[
    //     id      section       summary                                                              does   feasibility    memelint finding            note
    rule!("1.1",  WORDS,        "Use only approved words, technical nouns and technical verbs",       DYes,  Mechanical,    "unapproved word", "dictionary plus the project allowlist"),
    rule!("1.2",  WORDS,        "Use approved words only as their listed part of speech",             DPart, Heuristic,     "approved only as", "a rule with cue words. No finding when the context is not clear. A tagger can do more"),
    rule!("1.3",  WORDS,        "Use approved words only in their approved meaning",                  DNo,   Judgment,      "", "the dictionary has the meanings. No program can apply them"),
    rule!("1.4",  WORDS,        "Use only the listed forms of verbs and adjectives",                  DYes,  Mechanical,    "-ing form", "forms list"),
    rule!("1.5",  WORDS,        "Technical nouns are approved by category",                            DYes,  Mechanical,    "", "this is the allowlist"),
    rule!("1.6",  WORDS,        "An unapproved word only when it is a technical noun",                DYes,  Mechanical,    "not in the STE dictionary", "unknown words point at the allowlist"),
    rule!("1.7",  WORDS,        "Do not use technical nouns as verbs",                                DNo,   Heuristic,     "", "allowlisted word in verb position"),
    rule!("1.8",  WORDS,        "Use technical nouns that your organisation accepts",                     DYes,  Mechanical,    "", "the allowlist is examined in the repository"),
    rule!("1.9",  WORDS,        "Select short, clear technical nouns",                                DNo,   Judgment,      "", ""),
    rule!("1.10", WORDS,        "No slang, regional or jargon technical nouns",                       DNo,   Judgment,      "", ""),
    rule!("1.11", WORDS,        "One technical noun for each item",                                        DNo,   Judgment,      "", "to find synonyms is not mechanical"),
    rule!("1.12", WORDS,        "Technical verbs are approved by category",                            DYes,  Mechanical,    "", "allowlist"),
    rule!("1.13", WORDS,        "Do not use technical verbs as nouns",                                DNo,   Heuristic,     "", "the same as 1.7, for verbs"),
    rule!("1.14", WORDS,        "American spelling",                                                  DNo,   Mechanical,    "", "a British-to-American substitution table"),
    rule!("2.1",  NOUNS,        "Multi-word nouns of a maximum of 3 words",                            DNo,   Heuristic,     "", "a sequence of 4 dictionary nouns or allowlisted words"),
    rule!("2.2",  NOUNS,        "Write longer technical nouns in full",                               DNo,   Judgment,      "", ""),
    rule!("3.1",  VERBS,        "Only the verb forms in the dictionary",                              DYes,  Mechanical,    "-ing form", ""),
    rule!("3.2",  VERBS,        "Only the infinitive, imperative, simple present, simple past, simple future and past participle as adjective", DNo, Heuristic, "", "find perfect and progressive constructions"),
    rule!("3.3",  VERBS,        "Past participle only as an adjective",                               DNo,   Heuristic,     "", "overlaps 3.6"),
    rule!("3.4",  VERBS,        "No complex auxiliary constructions",                                 DNo,   Heuristic,     "", "have, has or had plus a participle. Modal verbs in sequence"),
    rule!("3.5",  VERBS,        "-ing only in technical nouns",                                   DYes,  Mechanical,    "-ing form", "the allowlist includes the exception"),
    rule!("3.6",  VERBS,        "Active voice. Passive only when the agent is unknown",               DNo,   Heuristic,     "", "be plus the participle of an approved verb. 'by' plus an agent is the strong signal"),
    rule!("3.7",  VERBS,        "Give an action with a verb, not with a noun",                         DNo,   Heuristic,     "", "make or do plus a noun with the stem of an approved verb"),
    rule!("4.1",  SENTENCES,    "Short, clear sentences",                                             DPart, Heuristic,     "sentence has", "length is mechanical, clarity is judgment"),
    rule!("4.2",  SENTENCES,    "No words removed, no contractions",                                  DPart, Heuristic,     "contraction", "contractions are mechanical. For articles that are not there, see 4.5"),
    rule!("4.3",  SENTENCES,    "Vertical lists for complex text",                                    DNo,   Judgment,      "", ""),
    rule!("4.4",  SENTENCES,    "Connections between related sentences",                        DNo,   Judgment,      "", ""),
    rule!("4.5",  SENTENCES,    "Use articles and demonstratives before nouns",                       DNo,   Heuristic,     "", "a singular noun with no article after a verb. Many incorrect findings in short comments"),
    rule!("5.1",  PROCEDURES,   "Procedural sentences of at most 20 words",                           DNo,   Mechanical,    "", "sentences that start with an approved verb are procedural"),
    rule!("5.2",  PROCEDURES,   "One instruction in each sentence",                                       DNo,   Heuristic,     "", "2 imperatives with 'and' between them"),
    rule!("5.3",  PROCEDURES,   "Instructions in the imperative",                                     DNo,   Heuristic,     "", "'you must' plus a verb points to the imperative"),
    rule!("5.4",  PROCEDURES,   "Condition before the instruction",                                   DNo,   Heuristic,     "", "an imperative with an 'if' clause at the end"),
    rule!("5.5",  PROCEDURES,   "Notes give information, not instructions",                           DNo,   Judgment,      "", ""),
    rule!("6.1",  DESCRIPTIONS, "Give information gradually",                                         DNo,   Judgment,      "", "Congree does it as some smaller rules"),
    rule!("6.2",  DESCRIPTIONS, "Key words give the text a structure",                                DNo,   Judgment,      "", ""),
    rule!("6.3",  DESCRIPTIONS, "Descriptive sentences of at most 25 words",                          DYes,  Mechanical,    "sentence has", ""),
    rule!("6.4",  DESCRIPTIONS, "Paragraphs group related information",                               DNo,   Judgment,      "", ""),
    rule!("6.5",  DESCRIPTIONS, "One topic for each paragraph",                                            DNo,   Judgment,      "", ""),
    rule!("6.6",  DESCRIPTIONS, "A maximum of 6 sentences in each paragraph",                                DNo,   Mechanical,    "", "a comment block is a paragraph"),
    rule!("7.1",  SAFETY,       "Identify the level of risk with warning or caution",                    DNo,   NotApplicable, "", "safety text does not occur in code comments"),
    rule!("7.2",  SAFETY,       "Start a safety instruction with a command or condition",             DNo,   NotApplicable, "", ""),
    rule!("7.3",  SAFETY,       "Tell the risk",                                                   DNo,   NotApplicable, "", ""),
    rule!("8.1",  PUNCTUATION,  "Standard punctuation, but no semicolons",                            DNo,   Mechanical,    "", "must not give findings for code fragments"),
    rule!("8.2",  PUNCTUATION,  "Hyphenate directly related words",                                   DNo,   Judgment,      "", ""),
    rule!("8.3",  PUNCTUATION,  "Parentheses only for the uses in the list",                           DNo,   Judgment,      "", ""),
    rule!("8.4",  PUNCTUATION,  "In a vertical list a colon ends the word count",                     DNo,   Mechanical,    "", "word-count rule"),
    rule!("8.5",  PUNCTUATION,  "Text in parentheses counts as one word",                                 DNo,   Mechanical,    "", "word-count rule"),
    rule!("8.6",  PUNCTUATION,  "Numbers, units, abbreviations, identifiers, quoted text, titles and proper nouns count as one word", DNo, Mechanical, "", "word-count rule"),
    rule!("8.7",  PUNCTUATION,  "Hyphenated words count as one word",                                 DYes,  Mechanical,    "", "tokens split at whitespace"),
    rule!("9.1",  PRACTICES,    "Write the sentence again when word-for-word substitution is not possible", DNo,   Judgment,      "", ""),
    rule!("9.2",  PRACTICES,    "Use each approved word correctly",                                   DNo,   Judgment,      "", ""),
    rule!("9.3",  PRACTICES,    "No phrasal verbs",                                                   DNo,   Mechanical,    "", "an approved verb plus a particle, when the pair is not an approved entry"),
    rule!("9.4",  PRACTICES,    "Consistent terminology and style",                                   DNo,   Judgment,      "", "in part a consistency check for each project"),
];

#[cfg(test)]
pub fn find(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifty_three_rules_in_nine_sections() {
        assert_eq!(RULES.len(), 53);
        let mut ids: Vec<&str> = RULES.iter().map(|r| r.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), 53, "duplicate rule id");
        let per_section = |s: &str| RULES.iter().filter(|r| r.section == s).count();
        assert_eq!(per_section(WORDS), 14);
        assert_eq!(per_section(NOUNS), 2);
        assert_eq!(per_section(VERBS), 7);
        assert_eq!(per_section(SENTENCES), 5);
        assert_eq!(per_section(PROCEDURES), 5);
        assert_eq!(per_section(DESCRIPTIONS), 6);
        assert_eq!(per_section(SAFETY), 3);
        assert_eq!(per_section(PUNCTUATION), 7);
        assert_eq!(per_section(PRACTICES), 4);
    }

    #[test]
    fn columns_are_consistent() {
        for r in RULES {
            // A rule that a program cannot check is not done.
            if matches!(r.feasibility, Judgment | NotApplicable) {
                assert_eq!(r.does, DNo, "rule {} is {:?} yet done", r.id, r.feasibility);
            }
            // A rule that is done names its finding or the mechanism.
            if r.does != DNo {
                assert!(
                    !r.check.is_empty() || !r.note.is_empty(),
                    "rule {} says done but how?",
                    r.id
                );
            }
        }
    }
}
