//! Parse the Part 2 dictionary table out of page text with positions.
//!
//! Each dictionary page is a table with 4 columns: word (part of speech),
//! approved meaning or alternatives, STE example, non-STE example. The
//! header row gives the x-coordinates of the columns. The words of each row
//! go to the columns by x. In column 1, upper-case text is an approved word
//! or its inflected forms, and lower-case text is an unapproved word. In
//! column 2, upper-case text is an alternative, and mixed-case text is a
//! meaning.

use std::sync::LazyLock;

use regex::Regex;
use memelint_common::{Alternative, Entry, Meaning};

use super::pdf::{Page, Word, join};

const POS: &str = "v|n|adj|adv|prep|pron|conj|art|prefix|TN|TV";

/// `FLURB (v),`  `flurb up (adv)`  `SO ... SO (conj)`  `zorch (away) (v)`
/// `zorch (in the zorch of ) (prep)`  `GLIM (or GLIMM) (adj)`
static RE_HEADWORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^(?P<word>[A-Za-z][A-Za-z0-9'’.\-/]*(?: [A-Za-z0-9'’.\-/]+)*?)(?P<qual> ?\([^()]*?\))? ?\((?P<pos>{POS})\),?$"
    ))
    .unwrap()
});
/// `GLIM (adj)`  `TWICE (TN) EACH DAY`
static RE_ALT_POS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^(?P<word>[A-Z][A-Z0-9'’ .\-/]*?)\s*\((?P<pos>{POS})\)(?:\s|$)"
    ))
    .unwrap()
});
static RE_MEANING_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<n>\d+)\.\s*(?P<text>.*)$").unwrap());
static RE_OTHER_MEANINGS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(for other|meanings, use:|for other meanings, use:)$").unwrap()
});
static RE_DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap());
static RE_ISSUE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bIssue (\d+)\b").unwrap());
static RE_PAGE_LABEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bPage 2-1-[A-Z]\d+\b").unwrap());
static RE_OR_QUAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^or ([A-Za-z\-]+)$").unwrap());
/// Part-of-speech and technical-name tags. They are lower case, also in
/// upper-case text: `PUT (v)`, `A CLAMP [TN]`.
static RE_TAGS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"\(({POS})\)|\[(?i:TN|TV)\]")).unwrap());

fn approved(e: &Entry) -> bool {
    e.status == "approved"
}

fn add_note(e: &mut Entry, text: &str) {
    match e.notes.last_mut() {
        Some(last) if !(last.ends_with('.') || last.ends_with(':')) => append(last, text),
        _ => e.notes.push(squash(text)),
    }
}

fn add_meaning_text(e: &mut Entry, text: &str) {
    let meanings = e.meanings.get_or_insert_with(Vec::new);
    if meanings.is_empty() {
        meanings.push(Meaning {
            number: 1,
            ..Default::default()
        });
    }
    append(&mut meanings.last_mut().unwrap().text, text);
}

/// Where the example text of this row goes. `in_other` is set after
/// `For other meanings, use:` in an approved entry.
fn examples(e: &mut Entry, in_other: bool) -> Option<(&mut String, &mut String)> {
    if approved(e) {
        if in_other && !e.alternatives.is_empty() {
            let a = e.alternatives.last_mut().unwrap();
            return Some((&mut a.ste_example, &mut a.non_ste_example));
        }
        let meanings = e.meanings.get_or_insert_with(Vec::new);
        if meanings.is_empty() {
            meanings.push(Meaning {
                number: 1,
                ..Default::default()
            });
        }
        let m = meanings.last_mut().unwrap();
        return Some((&mut m.ste_example, &mut m.non_ste_example));
    }
    e.alternatives
        .last_mut()
        .map(|a| (&mut a.ste_example, &mut a.non_ste_example))
}

pub struct Parsed {
    pub entries: Vec<Entry>,
    pub issue: Option<u32>,
    pub issue_date: Option<String>,
    pub dictionary_pages: usize,
}

// ---------------------------------------------------------------------------

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn append(target: &mut String, more: &str) {
    let more = squash(more);
    if more.is_empty() {
        return;
    }
    if !target.is_empty() {
        target.push(' ');
    }
    target.push_str(&more);
}

pub fn is_caps(s: &str) -> bool {
    let mut seen = false;
    for c in s.chars().filter(|c| c.is_alphabetic()) {
        seen = true;
        if !c.is_uppercase() {
            return false;
        }
    }
    seen
}

/// Upper-case text, when the part-of-speech tags are ignored: an alternative
/// or an STE example, not a meaning or a note.
fn caps_line(s: &str) -> bool {
    is_caps(&RE_TAGS.replace_all(s, ""))
}

/// A token that can be part of an upper-case headword or of a forms list.
fn caps_token(t: &str) -> bool {
    t == "(also" || t == "..." || is_caps(t)
}

struct Headword {
    word: String,
    qual: Option<String>,
    pos: String,
}

fn headword(text: &str) -> Option<Headword> {
    let m = RE_HEADWORD.captures(text)?;
    let word = m["word"].to_string();
    // Reject a mixed-case word: that is a cell that overflows into an
    // upper-case alternative.
    if !(is_caps(&word) || word == word.to_lowercase()) {
        return None;
    }
    Some(Headword {
        word,
        qual: m.name("qual").map(|q| q.as_str().to_string()),
        pos: m["pos"].to_string(),
    })
}

fn parse_forms(text: &str) -> Vec<String> {
    text.replace("(also", " ")
        .replace(['(', ')'], " ")
        .split(',')
        .map(|f| f.trim().to_lowercase())
        .filter(|f| !f.is_empty())
        .collect()
}

fn alternative(text: &str) -> Alternative {
    if let Some(m) = RE_ALT_POS.captures(text) {
        return Alternative {
            text: text.to_string(),
            word: squash(&m["word"]).to_lowercase(),
            pos: m["pos"].to_string(),
            ..Default::default()
        };
    }
    let word = text
        .split('(')
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches(['.', ','])
        .to_lowercase();
    Alternative {
        text: text.to_string(),
        word,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Columns {
    c1: f64,
    c2: f64,
    c3: f64,
    c4: f64,
}

struct Row {
    y: f64,
    col1: Vec<Word>,
    indented: bool,
    col2: String,
    col3: String,
    col4: String,
}

/// Find the table columns from the header words on a page.
fn find_columns(page: &Page, prev: Option<Columns>) -> Option<Columns> {
    let (mut c1, mut c2, mut c3, mut c4) = (None, None, None, None);
    for line in page.lines.iter().take(12) {
        for (i, w) in line.words.iter().enumerate() {
            match w.text.as_str() {
                "Word" if i == 0 => c1 = Some(w.x0),
                "ALTERNATIVES" => c2 = Some(w.x0),
                "STE" if line.words.get(i + 1).is_some_and(|n| n.text == "EXAMPLE") => {
                    c3 = Some(w.x0)
                }
                "Non-STE" => c4 = Some(w.x0),
                _ => {}
            }
        }
    }
    match (c1.or(prev.map(|p| p.c1)), c2, c3, c4) {
        (Some(c1), Some(c2), Some(c3), Some(c4)) => {
            Some(calibrate(page, Columns { c1, c2, c3, c4 }))
        }
        _ => prev,
    }
}

/// On some pages the table body is a maximum of 9pt to the left of the header
/// row. Cells of different row types have different padding. Move each
/// column start to the smallest x at which body text starts more than one
/// time near it.
fn calibrate(page: &Page, header: Columns) -> Columns {
    let body_start = |hx: f64| -> f64 {
        let mut counts: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
        for line in &page.lines {
            for w in &line.words {
                if (hx - 12.0..=hx + 3.0).contains(&w.x0) {
                    *counts.entry((w.x0 * 2.0).round() as i64).or_default() += 1;
                }
            }
        }
        counts
            .into_iter()
            .filter(|&(_, n)| n >= 3)
            .map(|(k, _)| k as f64 / 2.0)
            .fold(hx, f64::min)
    };
    Columns {
        c1: header.c1,
        c2: body_start(header.c2),
        c3: body_start(header.c3),
        c4: body_start(header.c4),
    }
}

fn is_header_line(text: &str) -> bool {
    text.contains("Part 2 - Dictionary")
        || text == "ASD-STE100 Simplified Technical English"
        || text == "Blank Page"
        || RE_DATE.is_match(text)
        || text.starts_with("Word Approved meaning/")
        || text.starts_with("(part of speech) ALTERNATIVES")
}

/// Give the words of a line to the columns, and make the column-1 / column-2
/// edge correct.
/// The lines in one table cell are 12pt apart. A new table row has a larger
/// space before it.
const ROW_GAP: f64 = 15.0;

fn make_row(words: &[Word], y: f64, cols: Columns) -> Row {
    // Column-1 text stays far from column 2 unless it overflows, which is
    // repaired below. Column-2 text can start a maximum of 5pt to the left of
    // the header word, which changes with the row type. Thus the first
    // tolerance is wider.
    const TOL1: f64 = 8.0;
    const TOL: f64 = 2.5;
    let (mut col1, mut col2, mut col3, mut col4) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for w in words {
        if w.x0 < cols.c2 - TOL1 {
            col1.push(w.clone());
        } else if w.x0 < cols.c3 - TOL {
            col2.push(w.clone());
        } else if w.x0 < cols.c4 - TOL {
            col3.push(w.clone());
        } else {
            col4.push(w.clone());
        }
    }

    if !col1.is_empty() {
        // A long cell overflows into the space of column 2 with one space
        // between, for example `ADJUSTABLE (adj) That you can adjust` or
        // `INTERCHANGED, the other`. Column-1 text is all upper-case or all
        // lower-case. Thus the tokens at the end that have the other case go
        // to column 2.
        let caps_mode = caps_token(&col1[0].text);
        let mut depth = 0i32;
        let mut keep = 0usize;
        for (i, w) in col1.iter().enumerate() {
            let t = w.text.as_str();
            let inside = depth > 0 || t.starts_with('(');
            depth += t.matches('(').count() as i32 - t.matches(')').count() as i32;
            let violates = !inside
                && if caps_mode {
                    !caps_token(t)
                } else {
                    is_caps(t) && t.len() > 1
                };
            if !violates {
                keep = i + 1;
            }
        }
        if keep < col1.len() {
            let moved = col1.split_off(keep);
            col2.splice(0..0, moved);
        }
        // The opposite: a `(pos)` or a `(qualifier)` at the edge of column 2
        // is part of the headword.
        if !col1.is_empty() && headword(&join(&col1)).is_none() {
            while col2.first().is_some_and(|w| w.text.starts_with('(')) {
                let mut take = 1;
                while take <= col2.len() && !col2[take - 1].text.ends_with(')') {
                    take += 1;
                }
                if take > col2.len() {
                    break;
                }
                let moved: Vec<Word> = col2.drain(0..take).collect();
                col1.extend(moved);
                if headword(&join(&col1)).is_some() {
                    break;
                }
            }
        }
    }

    let indented = col1.first().is_some_and(|w| w.x0 > cols.c1 + 6.0);
    Row {
        y,
        col1,
        indented,
        col2: join(&col2),
        col3: join(&col3),
        col4: join(&col4),
    }
}

// ---------------------------------------------------------------------------

pub fn parse(pages: &[Page], warn: &mut dyn FnMut(String)) -> Parsed {
    let mut entries: Vec<Entry> = Vec::new();
    let mut issue = None;
    let mut issue_date = None;
    let mut cols: Option<Columns> = None;
    let mut dictionary_pages = 0;

    for page in pages {
        let texts: Vec<String> = page.lines.iter().map(|l| l.text()).collect();
        let Some(header) = texts
            .iter()
            .find(|t| t.contains("Part 2 - Dictionary") && RE_PAGE_LABEL.is_match(t))
        else {
            continue;
        };
        dictionary_pages += 1;
        if issue.is_none() {
            issue = RE_ISSUE.captures(header).and_then(|c| c[1].parse().ok());
        }
        if issue_date.is_none() {
            issue_date = texts.iter().find(|t| RE_DATE.is_match(t)).cloned();
        }
        cols = find_columns(page, cols);
        let Some(c) = cols else {
            warn(format!(
                "page {}: no column header found; skipped",
                page.number
            ));
            continue;
        };

        let rows: Vec<Row> = page
            .lines
            .iter()
            .zip(&texts)
            .filter(|(_, t)| !is_header_line(t))
            .map(|(l, _)| make_row(&l.words, l.y, c))
            .collect();

        let mut i = 0;
        let mut prev_y: Option<f64> = None;
        let mut in_other = false;
        while i < rows.len() {
            let row = &rows[i];
            i += 1;
            let new_row = prev_y.is_none_or(|p| row.y - p > ROW_GAP);
            prev_y = Some(row.y);
            let col1 = join(&row.col1);
            let mut extra: Vec<&Row> = Vec::new();

            if !col1.is_empty() && !row.indented {
                let mut hw = headword(&col1);
                if hw.is_none()
                    && i < rows.len()
                    && !rows[i].col1.is_empty()
                    && !rows[i].indented
                    // A forms line that comes before the next headword is not a wrap.
                    && (col1.ends_with('-') || headword(&join(&rows[i].col1)).is_none())
                {
                    // A headword that wraps to 2 lines: `ACCIDENTAL` / `(adj)`,
                    // `UNSATISFAC-` / `TORILY (adv)`, `chance` / `(by chance) (n)`.
                    let next = join(&rows[i].col1);
                    let joined = match col1.strip_suffix('-') {
                        Some(stem) => format!("{stem}{next}"),
                        None => format!("{col1} {next}"),
                    };
                    if let Some(h) = headword(&joined) {
                        hw = Some(h);
                        extra.push(&rows[i]);
                        prev_y = Some(rows[i].y);
                        i += 1;
                    }
                }

                if let Some(h) = hw {
                    in_other = false;
                    let lemma = squash(&h.word).to_lowercase();
                    let approved = is_caps(&h.word);
                    let mut forms = vec![lemma.clone()];
                    let headword = match &h.qual {
                        Some(q) => {
                            let q = squash(q.trim().trim_matches(['(', ')']));
                            if let Some(m) = RE_OR_QUAL.captures(&q) {
                                forms.push(m[1].to_lowercase());
                            }
                            format!("{lemma} ({q})")
                        }
                        None => lemma.clone(),
                    };
                    entries.push(Entry {
                        headword,
                        lemma,
                        pos: h.pos,
                        status: if approved { "approved" } else { "unapproved" }.to_string(),
                        forms: approved.then_some(forms),
                        meanings: approved.then_some(Vec::new()),
                        alternatives: Vec::new(),
                        notes: Vec::new(),
                        page: page.number,
                    });
                } else if let Some(e) = entries
                    .last_mut()
                    .filter(|e| approved(e) && caps_token(&row.col1[0].text))
                {
                    let forms = e.forms.as_mut().unwrap();
                    for f in parse_forms(&col1) {
                        if !forms.contains(&f) {
                            forms.push(f);
                        }
                    }
                } else if !is_caps(&col1) && col1.starts_with(|c: char| c.is_ascii_lowercase()) {
                    in_other = false;
                    // An unapproved multi-word entry with no part of speech, for example `such as`.
                    entries.push(Entry {
                        headword: col1.to_lowercase(),
                        lemma: col1.to_lowercase(),
                        pos: String::new(),
                        status: "unapproved".to_string(),
                        forms: None,
                        meanings: None,
                        alternatives: Vec::new(),
                        notes: Vec::new(),
                        page: page.number,
                    });
                } else {
                    warn(format!(
                        "page {}: unrecognised column-1 text: {col1:?}",
                        page.number
                    ));
                }
            } else if !col1.is_empty()
                && let Some(e) = entries.last_mut()
            {
                add_note(e, &col1);
            }

            let Some(entry) = entries.last_mut() else {
                continue;
            };
            consume_row(
                entry,
                &row.col2,
                &row.col3,
                &row.col4,
                new_row,
                &mut in_other,
            );
            for r in extra {
                consume_row(entry, &r.col2, &r.col3, &r.col4, false, &mut in_other);
            }
        }
    }

    Parsed {
        entries,
        issue,
        issue_date,
        dictionary_pages,
    }
}

fn consume_row(
    entry: &mut Entry,
    col2: &str,
    col3: &str,
    col4: &str,
    new_row: bool,
    in_other: &mut bool,
) {
    let c2 = col2.trim();
    if !c2.is_empty() {
        if RE_OTHER_MEANINGS.is_match(c2) {
            *in_other = true;
        } else if let Some(m) = RE_MEANING_NUMBER.captures(c2).filter(|_| approved(entry)) {
            entry.meanings.get_or_insert_with(Vec::new).push(Meaning {
                number: m["n"].parse().unwrap_or(0),
                text: squash(&m["text"]),
                ..Default::default()
            });
            *in_other = false;
        } else if caps_line(c2)
            || RE_TAGS.is_match(c2) && RE_TAGS.replace_all(c2, "").trim().is_empty()
        {
            // Each alternative has a table row. Table rows are more than
            // 12pt apart, and the lines in one cell are 12pt apart. A bare
            // `(adv)` is the tag of a long alternative that continues on the
            // next line. A qualifier in parentheses, for example
            // `(WITH A CLAMP [TN])`, also continues the alternative above it.
            let bare_tag = RE_TAGS.replace_all(c2, "").trim().is_empty();
            let new_alt = new_row && !bare_tag && !c2.starts_with('(');
            if new_alt || entry.alternatives.is_empty() {
                entry.alternatives.push(alternative(c2));
            } else {
                let last = entry.alternatives.last_mut().unwrap();
                let mut text = last.text.clone();
                append(&mut text, c2);
                let merged = alternative(&text);
                last.text = merged.text;
                last.word = merged.word;
                last.pos = merged.pos;
            }
        } else {
            if approved(entry) && !*in_other {
                add_meaning_text(entry, c2);
            } else {
                add_note(entry, c2);
            }
        }
    }

    if let Some((ste, non_ste)) = examples(entry, *in_other) {
        if !col3.trim().is_empty() {
            append(ste, col3);
        }
        if !col4.trim().is_empty() {
            append(non_ste, col4);
        }
    }
}

#[cfg(test)]
mod tests {
    // The fixtures use words that are not English, in the layout of the
    // dictionary. No fixture is an entry of the standard.
    use super::*;

    fn hw(s: &str) -> (String, Option<String>, String) {
        let h = headword(s).unwrap_or_else(|| panic!("no headword in {s:?}"));
        (h.word, h.qual, h.pos)
    }

    #[test]
    fn headwords() {
        assert_eq!(hw("FLURB (v),"), ("FLURB".into(), None, "v".into()));
        assert_eq!(
            hw("flurb up (adv)"),
            ("flurb up".into(), None, "adv".into())
        );
        assert_eq!(
            hw("SO ... SO (conj)"),
            ("SO ... SO".into(), None, "conj".into())
        );
        assert_eq!(
            hw("zorch (away) (v)"),
            ("zorch".into(), Some(" (away)".into()), "v".into())
        );
        assert_eq!(
            hw("zorch (in the zorch of ) (prep)"),
            (
                "zorch".into(),
                Some(" (in the zorch of )".into()),
                "prep".into()
            )
        );
        assert_eq!(
            hw("GLIM (or GLIMM) (adj)"),
            ("GLIM".into(), Some(" (or GLIMM)".into()), "adj".into())
        );
        assert_eq!(hw("UN- (prefix)"), ("UN-".into(), None, "prefix".into()));
        assert!(headword("FLURBS,").is_none());
        assert!(headword("No other verb forms.").is_none());
        // A word that overflows into an upper-case alternative is not a headword.
        assert!(headword("flurbwise FLURBWARD (adj)").is_none());
    }

    #[test]
    fn forms() {
        assert_eq!(parse_forms("FLURBS,"), vec!["flurbs"]);
        assert_eq!(parse_forms("ZIG, ZAG,"), vec!["zig", "zag"]);
        assert_eq!(parse_forms("(also ZIGS, ZAGS)"), vec!["zigs", "zags"]);
        assert_eq!(parse_forms("FLURBS UP,"), vec!["flurbs up"]);
    }

    #[test]
    fn alternatives() {
        let a = alternative("GLIM (adj)");
        assert_eq!((a.word.as_str(), a.pos.as_str()), ("glim", "adj"));
        let a = alternative("TWICE (TN) EACH DAY");
        assert_eq!((a.word.as_str(), a.pos.as_str()), ("twice", "TN"));
        let a = alternative("LESS OFTEN");
        assert_eq!((a.word.as_str(), a.pos.as_str()), ("less often", ""));
        let a = alternative("FASTEN (v) (WITH A ZIP [TN])");
        assert_eq!((a.word.as_str(), a.pos.as_str()), ("fasten", "v"));
    }

    #[test]
    fn case_classes() {
        assert!(caps_line("FLURB (v)"));
        assert!(caps_line("(WITH A ZIP [TN])"));
        assert!(caps_line("POUR ROUGHLY 3 CUPS."));
        assert!(!caps_line("That you can wiggle"));
        assert!(!caps_line("(adv)"));
        assert!(caps_token("(also"));
        assert!(!caps_token("the"));
    }
}
