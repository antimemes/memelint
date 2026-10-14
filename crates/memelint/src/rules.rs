//! The 53 rules of ASD-STE100 Issue 9, and the relation of memelint to each
//! rule. The rule ids in the findings and in `[rules]` of `memelint.toml`
//! come from this list. The summaries are paraphrases, not the text of the
//! spec.

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
    pub summary: &'static str,
    pub does: Does,
    pub feasibility: Feasibility,
}

use Does::{No as DNo, Partial as DPart, Yes as DYes};
use Feasibility::*;

macro_rules! rule {
    ($id:literal, $summary:literal, $does:expr, $feasibility:expr) => {
        Rule {
            id: $id,
            summary: $summary,
            does: $does,
            feasibility: $feasibility,
        }
    };
}

#[rustfmt::skip]
pub const RULES: &[Rule] = &[
    // 1 Words
    rule!("1.1",  "Use only approved words, technical nouns and technical verbs",       DYes,  Mechanical),
    rule!("1.2",  "Use approved words only as their listed part of speech",             DPart, Heuristic),
    rule!("1.3",  "Use approved words only in their approved meaning",                  DNo,   Judgment),
    rule!("1.4",  "Use only the listed forms of verbs and adjectives",                  DYes,  Mechanical),
    rule!("1.5",  "Technical nouns are approved by category",                           DYes,  Mechanical),
    rule!("1.6",  "An unapproved word only when it is a technical noun",                DYes,  Mechanical),
    rule!("1.7",  "Do not use technical nouns as verbs",                                DNo,   Heuristic),
    rule!("1.8",  "Use technical nouns that your organisation accepts",                 DYes,  Mechanical),
    rule!("1.9",  "Select short, clear technical nouns",                                DNo,   Judgment),
    rule!("1.10", "No slang, regional or jargon technical nouns",                       DNo,   Judgment),
    rule!("1.11", "One technical noun for each item",                                   DNo,   Judgment),
    rule!("1.12", "Technical verbs are approved by category",                           DYes,  Mechanical),
    rule!("1.13", "Do not use technical verbs as nouns",                                DNo,   Heuristic),
    rule!("1.14", "American spelling",                                                  DNo,   Mechanical),
    // 2 Multi-word nouns
    rule!("2.1",  "Multi-word nouns of a maximum of 3 words",                           DNo,   Heuristic),
    rule!("2.2",  "Write longer technical nouns in full",                               DNo,   Judgment),
    // 3 Verbs
    rule!("3.1",  "Only the verb forms in the dictionary",                              DYes,  Mechanical),
    rule!("3.2",  "Only the infinitive, imperative, simple present, simple past, simple future and past participle as adjective", DNo, Heuristic),
    rule!("3.3",  "Past participle only as an adjective",                               DNo,   Heuristic),
    rule!("3.4",  "No complex auxiliary constructions",                                 DNo,   Heuristic),
    rule!("3.5",  "-ing only in technical nouns",                                       DYes,  Mechanical),
    rule!("3.6",  "Active voice. Passive only when the agent is unknown",               DNo,   Heuristic),
    rule!("3.7",  "Give an action with a verb, not with a noun",                        DNo,   Heuristic),
    // 4 Sentences
    rule!("4.1",  "Short, clear sentences",                                             DPart, Heuristic),
    rule!("4.2",  "No words removed, no contractions",                                  DPart, Heuristic),
    rule!("4.3",  "Vertical lists for complex text",                                    DNo,   Judgment),
    rule!("4.4",  "Connections between related sentences",                              DNo,   Judgment),
    rule!("4.5",  "Use articles and demonstratives before nouns",                       DNo,   Heuristic),
    // 5 Procedural writing
    rule!("5.1",  "Procedural sentences of at most 20 words",                           DNo,   Mechanical),
    rule!("5.2",  "One instruction in each sentence",                                   DNo,   Heuristic),
    rule!("5.3",  "Instructions in the imperative",                                     DNo,   Heuristic),
    rule!("5.4",  "Condition before the instruction",                                   DNo,   Heuristic),
    rule!("5.5",  "Notes give information, not instructions",                           DNo,   Judgment),
    // 6 Descriptive writing
    rule!("6.1",  "Give information gradually",                                         DNo,   Judgment),
    rule!("6.2",  "Key words give the text a structure",                                DNo,   Judgment),
    rule!("6.3",  "Descriptive sentences of at most 25 words",                          DYes,  Mechanical),
    rule!("6.4",  "Paragraphs group related information",                               DNo,   Judgment),
    rule!("6.5",  "One topic for each paragraph",                                       DNo,   Judgment),
    rule!("6.6",  "A maximum of 6 sentences in each paragraph",                         DNo,   Mechanical),
    // 7 Safety instructions
    rule!("7.1",  "Identify the level of risk with warning or caution",                 DNo,   NotApplicable),
    rule!("7.2",  "Start a safety instruction with a command or condition",             DNo,   NotApplicable),
    rule!("7.3",  "Tell the risk",                                                      DNo,   NotApplicable),
    // 8 Punctuation and word count
    rule!("8.1",  "Standard punctuation, but no semicolons",                            DNo,   Mechanical),
    rule!("8.2",  "Hyphenate directly related words",                                   DNo,   Judgment),
    rule!("8.3",  "Parentheses only for the uses in the list",                          DNo,   Judgment),
    rule!("8.4",  "In a vertical list a colon ends the word count",                     DNo,   Mechanical),
    rule!("8.5",  "Text in parentheses counts as one word",                             DNo,   Mechanical),
    rule!("8.6",  "Numbers, units, abbreviations, identifiers, quoted text, titles and proper nouns count as one word", DNo, Mechanical),
    rule!("8.7",  "Hyphenated words count as one word",                                 DYes,  Mechanical),
    // 9 Writing practices
    rule!("9.1",  "Write the sentence again when word-for-word substitution is not possible", DNo, Judgment),
    rule!("9.2",  "Use each approved word correctly",                                   DNo,   Judgment),
    rule!("9.3",  "No phrasal verbs",                                                   DNo,   Mechanical),
    rule!("9.4",  "Consistent terminology and style",                                   DNo,   Judgment),
];

pub fn find(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.id == id)
}

/// The ids of the rules that give findings, for `[rules]` in `memelint.toml`.
pub const CHECKED: &[&str] = &["1.1", "1.2", "1.6", "3.5", "4.2", "6.3"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifty_three_rules() {
        assert_eq!(RULES.len(), 53);
        let mut ids: Vec<&str> = RULES.iter().map(|r| r.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), 53, "duplicate rule id");
        for section in 1..=9 {
            let n = RULES
                .iter()
                .filter(|r| r.id.starts_with(&format!("{section}.")))
                .count();
            assert_eq!(
                n,
                [14, 2, 7, 5, 5, 6, 3, 7, 4][section - 1],
                "section {section}"
            );
        }
    }

    #[test]
    fn columns_are_consistent() {
        for r in RULES {
            // A rule that a program cannot check is not done.
            if matches!(r.feasibility, Judgment | NotApplicable) {
                assert_eq!(r.does, DNo, "rule {} is {:?} yet done", r.id, r.feasibility);
            }
        }
        // Each rule in CHECKED is done, fully or in part.
        for id in CHECKED {
            assert_ne!(
                find(id).unwrap().does,
                DNo,
                "rule {id} gives findings but is not done"
            );
        }
    }
}
