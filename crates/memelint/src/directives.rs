//! Lines that tell memelint not to give findings: `memelint: ignore`.

use crate::extract::Comment;

/// The target of a directive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The comment that contains this line.
    This,
    /// The next comment after this line.
    Next,
    /// All comments in the file.
    File,
}

/// One `memelint:` line in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive {
    /// 1-based line of the directive.
    pub line: usize,
    pub kind: Kind,
    /// Rule ids, for example `4.2`. Empty means all rules.
    pub rules: Vec<String>,
}

/// The directives in `source`, in line sequence.
pub fn parse(source: &str) -> Vec<Directive> {
    source
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            parse_line(line).map(|(kind, rules)| Directive {
                line: i + 1,
                kind,
                rules,
            })
        })
        .collect()
}

/// Does a directive tell memelint not to give this finding?
pub fn hides(
    directives: &[Directive],
    comment: &Comment,
    comments: &[Comment],
    rule: &str,
) -> bool {
    directives
        .iter()
        .any(|d| applies(d, comment, comments, rule))
}

fn applies(d: &Directive, comment: &Comment, comments: &[Comment], rule: &str) -> bool {
    if !d.rules.is_empty() && !d.rules.iter().any(|r| r == rule) {
        return false;
    }
    match d.kind {
        Kind::File => true,
        Kind::This => {
            if comments.iter().any(|c| c.contains_line(d.line)) {
                comment.contains_line(d.line)
            } else {
                next_comment(comments, d.line).is_some_and(|n| same_span(comment, n))
            }
        }
        Kind::Next => next_comment(comments, d.line).is_some_and(|n| same_span(comment, n)),
    }
}

fn next_comment(comments: &[Comment], line: usize) -> Option<&Comment> {
    comments.iter().find(|c| c.start_line > line)
}

fn same_span(a: &Comment, b: &Comment) -> bool {
    a.start_line == b.start_line && a.end_line == b.end_line
}

fn parse_line(line: &str) -> Option<(Kind, Vec<String>)> {
    parse_payload(&payload(line)?)
}

/// The text after `memelint:`, if this line is a directive.
fn payload(line: &str) -> Option<String> {
    let mut t = line.trim();
    if let Some(rest) = t.strip_prefix("<!--") {
        t = rest.trim();
        t = t.strip_suffix("-->").unwrap_or(t).trim();
    }
    t = strip_opener(t).trim();
    t = t.strip_suffix("*/").unwrap_or(t).trim();
    let lower = t.to_ascii_lowercase();
    let rest = lower.strip_prefix("memelint:")?;
    let orig = &t[t.len() - rest.len()..];
    Some(orig.to_string())
}

fn strip_opener(t: &str) -> &str {
    let t = t.trim_start();
    for prefix in ["///", "//!", "//", "/*"] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return rest.trim_start_matches('*');
        }
    }
    if let Some(rest) = t.strip_prefix('#') {
        return rest.trim_start_matches('#');
    }
    t
}

fn parse_payload(payload: &str) -> Option<(Kind, Vec<String>)> {
    let t = payload.trim();
    let lower = t.to_ascii_lowercase();
    let (kind, rest) = if let Some(r) = lower.strip_prefix("ignore-file") {
        (Kind::File, r)
    } else if let Some(r) = lower.strip_prefix("ignore-next") {
        (Kind::Next, r)
    } else {
        let r = lower.strip_prefix("ignore")?;
        (Kind::This, r)
    };
    let orig_rest = &t[t.len() - rest.len()..];
    Some((kind, parse_rules(orig_rest)))
}

fn parse_rules(s: &str) -> Vec<String> {
    let s = s
        .trim()
        .trim_end_matches("-->")
        .trim_end_matches("*/")
        .trim();
    if s.is_empty() {
        return Vec::new();
    }
    s.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .map(|p| {
            let p = p.trim_matches(|c: char| matches!(c, '[' | ']' | '(' | ')'));
            let p = p
                .strip_prefix("STE")
                .or_else(|| p.strip_prefix("ste"))
                .unwrap_or(p);
            p.trim().trim_start_matches('-').to_string()
        })
        .filter(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit() || c == '.'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::Line;

    fn comment(start: usize, end: usize) -> Comment {
        Comment {
            start_line: start,
            end_line: end,
            lines: vec![Line {
                line: start,
                col: 1,
                byte: 0,
                text: "Hoist the gantry.".into(),
            }],
        }
    }

    #[test]
    fn parses_comment_forms() {
        let src = concat!(
            "// memelint: ignore\n",
            "/// memelint: ignore 4.2\n",
            "# memelint: ignore-next\n",
            "/* memelint: ignore-file */\n",
            "<!-- memelint: ignore 4.2, 6.3 -->\n",
            "memelint: ignore\n",
            "// memelint: ignore STE 1.1\n",
            "// MEMELINT: IGNORE-NEXT 3.5\n",
            "// see memelint: ignore\n",
            "println!(\"memelint: ignore\");\n",
        );
        let d = parse(src);
        assert_eq!(
            d,
            [
                Directive {
                    line: 1,
                    kind: Kind::This,
                    rules: vec![]
                },
                Directive {
                    line: 2,
                    kind: Kind::This,
                    rules: vec!["4.2".into()]
                },
                Directive {
                    line: 3,
                    kind: Kind::Next,
                    rules: vec![]
                },
                Directive {
                    line: 4,
                    kind: Kind::File,
                    rules: vec![]
                },
                Directive {
                    line: 5,
                    kind: Kind::This,
                    rules: vec!["4.2".into(), "6.3".into()]
                },
                Directive {
                    line: 6,
                    kind: Kind::This,
                    rules: vec![]
                },
                Directive {
                    line: 7,
                    kind: Kind::This,
                    rules: vec!["1.1".into()]
                },
                Directive {
                    line: 8,
                    kind: Kind::Next,
                    rules: vec!["3.5".into()]
                },
            ]
        );
    }

    #[test]
    fn this_hides_the_comment_that_contains_the_line() {
        let comments = [comment(1, 2), comment(5, 6)];
        let d = [Directive {
            line: 2,
            kind: Kind::This,
            rules: vec![],
        }];
        assert!(hides(&d, &comments[0], &comments, "4.2"));
        assert!(!hides(&d, &comments[1], &comments, "4.2"));
    }

    #[test]
    fn this_without_a_comment_applies_to_the_next() {
        // An HTML comment is not prose, thus no comment contains line 1.
        let comments = [comment(3, 3), comment(5, 5)];
        let d = [Directive {
            line: 1,
            kind: Kind::This,
            rules: vec![],
        }];
        assert!(hides(&d, &comments[0], &comments, "1.1"));
        assert!(!hides(&d, &comments[1], &comments, "1.1"));
    }

    #[test]
    fn next_hides_the_subsequent_comment() {
        let comments = [comment(1, 1), comment(4, 4)];
        let d = [Directive {
            line: 2,
            kind: Kind::Next,
            rules: vec![],
        }];
        assert!(!hides(&d, &comments[0], &comments, "1.1"));
        assert!(hides(&d, &comments[1], &comments, "1.1"));
    }

    #[test]
    fn file_hides_all_comments() {
        let comments = [comment(1, 1), comment(8, 9)];
        let d = [Directive {
            line: 1,
            kind: Kind::File,
            rules: vec![],
        }];
        assert!(hides(&d, &comments[0], &comments, "1.1"));
        assert!(hides(&d, &comments[1], &comments, "6.3"));
    }

    #[test]
    fn named_rules_only() {
        let comments = [comment(1, 2)];
        let d = [Directive {
            line: 1,
            kind: Kind::This,
            rules: vec!["4.2".into()],
        }];
        assert!(hides(&d, &comments[0], &comments, "4.2"));
        assert!(!hides(&d, &comments[0], &comments, "1.1"));
    }
}
