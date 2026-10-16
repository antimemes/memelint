//! Get the prose out of source code: the comments and docstrings that
//! tree-sitter finds. The comment markers, the markup and the lines that
//! are not prose are removed.

use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::{Node, Parser};

use crate::grammars::Grammar;

/// A cleaned line of comment text with its position in the source file.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// 1-based.
    pub line: usize,
    /// 1-based column of the first character of `text`.
    pub col: usize,
    /// Byte offset in the source of the first character of `text`. The
    /// markup that becomes spaces keeps its byte length. Thus `byte + i` is
    /// the source offset of each character `text[i..]` that is not a space.
    pub byte: usize,
    pub text: String,
}

/// One comment (or run of adjacent line comments) as prose.
#[derive(Debug, Clone, Default)]
pub struct Comment {
    pub lines: Vec<Line>,
}

impl Comment {
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Find the (line, column) of a byte offset into `text()`. Line and column
    /// are 1-based.
    pub fn position(&self, offset: usize) -> (usize, usize) {
        let mut start = 0;
        for l in &self.lines {
            let end = start + l.text.len();
            if offset <= end {
                return (l.line, l.col + l.text[..offset - start].chars().count());
            }
            start = end + 1;
        }
        let last = self.lines.last().expect("comment has lines");
        (last.line, last.col + last.text.chars().count())
    }

    /// Map a byte range of `text()` to a byte range of the source. The range
    /// must be on one line; if it is not, None.
    pub fn source_range(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        let mut line_start = 0;
        for l in &self.lines {
            let line_end = line_start + l.text.len();
            if start <= line_end {
                if end < start || end > line_end {
                    return None;
                }
                return Some((l.byte + (start - line_start), l.byte + (end - line_start)));
            }
            line_start = line_end + 1;
        }
        None
    }
}

/// Get the prose from `source`: the comments of source code, or the
/// paragraphs of a prose document. Parse it with `grammar`.
pub fn extract(grammar: &Grammar, source: &str) -> Vec<Comment> {
    let mut parser = Parser::new();
    parser
        .set_language(&grammar.language)
        .expect("grammar built against this tree-sitter version");
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut raw: Vec<RawComment> = Vec::new();
    if grammar.prose {
        collect_prose(grammar, tree.root_node(), source, &mut raw);
    } else {
        collect(grammar, tree.root_node(), source, &mut raw);
    }
    merge_adjacent(&mut raw);
    raw.iter().filter_map(clean).collect()
}

/// Is the text of this syntax part prose? Comments and docstrings in code,
/// and paragraphs, headings and table cells in prose documents are prose.
pub fn is_prose(grammar: &Grammar, node: Node) -> bool {
    if grammar.prose {
        matches!(
            node.kind(),
            "paragraph" | "atx_heading" | "setext_heading" | "pipe_table_cell"
        )
    } else {
        node.kind().contains("comment") || is_docstring(node)
    }
}

/// The paragraphs, headings and table cells of a Markdown document. Code
/// blocks and HTML blocks are not prose.
fn collect_prose(grammar: &Grammar, node: Node, source: &str, out: &mut Vec<RawComment>) {
    if is_prose(grammar, node) {
        out.push(RawComment::new(node, source, false));
        return;
    }
    if matches!(
        node.kind(),
        "fenced_code_block" | "indented_code_block" | "html_block"
    ) {
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_prose(grammar, child, source, out);
    }
}

struct RawComment {
    line: usize,
    col: usize,
    end_line: usize,
    text: String,
    /// The source byte offset of the start of each line of `text`.
    starts: Vec<usize>,
    /// A line comment (`//`, `#`), not a block comment or a docstring.
    is_line: bool,
}

impl RawComment {
    fn new(node: Node, source: &str, is_line: bool) -> Self {
        // Some grammars include the line break in a line comment.
        let text = source[node.byte_range()].trim_end_matches(['\n', '\r']);
        let line = node.start_position().row + 1;
        let base = node.start_byte();
        let starts = std::iter::once(base)
            .chain(text.match_indices('\n').map(|(i, _)| base + i + 1))
            .collect();
        RawComment {
            line,
            col: node.start_position().column + 1,
            end_line: line + text.matches('\n').count(),
            text: text.to_string(),
            starts,
            is_line,
        }
    }
}

fn collect(grammar: &Grammar, node: Node, source: &str, out: &mut Vec<RawComment>) {
    if is_prose(grammar, node) {
        let is_line =
            node.kind().contains("comment") && !source[node.byte_range()].starts_with("/*");
        out.push(RawComment::new(node, source, is_line));
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(grammar, child, source, out);
    }
}

/// A Python docstring: a string that is the first statement of a module,
/// class or function body.
fn is_docstring(node: Node) -> bool {
    if node.kind() != "string" {
        return false;
    }
    let Some(stmt) = node.parent().filter(|p| p.kind() == "expression_statement") else {
        return false;
    };
    let Some(body) = stmt.parent() else {
        return false;
    };
    matches!(body.kind(), "module" | "block") && stmt.prev_named_sibling().is_none()
}

/// Line comments that follow each other at the same indentation are one
/// paragraph.
fn merge_adjacent(raw: &mut Vec<RawComment>) {
    let mut merged: Vec<RawComment> = Vec::new();
    for c in raw.drain(..) {
        match merged.last_mut() {
            Some(prev)
                if prev.is_line
                    && c.is_line
                    && c.line == prev.end_line + 1
                    && c.col == prev.col =>
            {
                prev.text.push('\n');
                prev.text.push_str(&c.text);
                prev.end_line = c.end_line;
                prev.starts.extend(c.starts);
            }
            _ => merged.push(c),
        }
    }
    *raw = merged;
}

static RE_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^(\s*)(///|//!|//|/\*\*|/\*|\*/|\*|--|;+|%+|#!|#+|"""|''')?(\s*)"#).unwrap()
});
static RE_TRAILER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\s*(\*/|"""|''')\s*$"#).unwrap());
static RE_DIRECTIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(noqa|type:|pylint|pyright|mypy|flake8|ruff:|eslint|prettier|@ts-|biome-|fmt:|rustfmt|clippy|#\[|nolint|go:|lint:|spdx|pragma|coverage|istanbul|-\*-|vim:|emacs:)",
    )
    .unwrap()
});
static RE_BLANKABLE: LazyLock<Regex> = LazyLock::new(|| {
    // These become spaces, and the columns do not change: inline code,
    // URLs, link targets, HTML tags, JSDoc types and tags, and task tags.
    Regex::new(
        r"`[^`]*`|https?://\S+|\]\([^)]*\)|<[^>]*>|\{[^}]*\}|@\w+|\b(TODO|FIXME|XXX|HACK|NOTE)\b:?",
    )
    .unwrap()
});
static RE_CODE_LIKE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[;{}]$|\(\)|^(import |from |#include|return |let |const |var |if \(|for \(|while \(|fn |def |class |pub |use )").unwrap()
});
static RE_LICENSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)copyright|licen[sc]e|all rights reserved").unwrap());

/// Strip markers and markup from a raw comment. Returns `None` when the
/// comment is not prose: a license header, a directive, commented-out code.
fn clean(raw: &RawComment) -> Option<Comment> {
    let mut lines = Vec::new();
    let mut in_fence = false;
    for (i, src_line) in raw.text.split('\n').enumerate() {
        let line_no = raw.line + i;
        // All the lines of merged line comments start at the comment column.
        // The subsequent lines of a block comment or a docstring start at
        // column 1.
        let base_col = if i == 0 || raw.is_line { raw.col } else { 1 };

        let m = RE_MARKER.captures(src_line).unwrap();
        let stripped = m.get(0).unwrap().end();
        let mut text = &src_line[stripped..];
        if let Some(t) = RE_TRAILER.find(text) {
            text = &text[..t.start()];
        }
        if text.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence || RE_DIRECTIVE.is_match(text.trim_start()) {
            continue;
        }
        let blanked = RE_BLANKABLE.replace_all(text, |c: &regex::Captures| {
            // A link target keeps its `]`. Thus the link text stays a word.
            let keep = if c[0].starts_with(']') { "]" } else { "" };
            format!("{keep}{}", " ".repeat(c[0].len() - keep.len()))
        });
        let col = base_col + src_line[..stripped].chars().count();
        lines.push(Line {
            line: line_no,
            col,
            byte: raw.starts[i] + stripped,
            text: blanked.into_owned(),
        });
    }

    let prose: Vec<&Line> = lines.iter().filter(|l| !l.text.trim().is_empty()).collect();
    if prose.is_empty() {
        return None;
    }
    if prose.iter().take(3).any(|l| RE_LICENSE.is_match(&l.text)) {
        return None;
    }
    let code_like = prose
        .iter()
        .filter(|l| RE_CODE_LIKE.is_match(l.text.trim()))
        .count();
    if code_like * 2 >= prose.len() {
        return None;
    }
    Some(Comment { lines })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn texts(ext: &str, source: &str) -> Vec<String> {
        let lang = crate::grammars::Grammars::new(None)
            .for_path(Path::new(&format!("x.{ext}")))
            .unwrap();
        extract(&lang, source).iter().map(|c| c.text()).collect()
    }

    #[test]
    fn rust_comments_are_merged_and_stripped() {
        let src =
            "// Turn the dial.\n// Then wait.\nfn f() {}\n/// Returns the count.\nfn g() {}\n";
        assert_eq!(
            texts("rs", src),
            vec!["Turn the dial.\nThen wait.", "Returns the count."]
        );
    }

    #[test]
    fn python_docstring_and_hash_comments() {
        let src = "def f():\n    \"\"\"Open the hatch.\n\n    Wait for the light.\n    \"\"\"\n    x = 1  # noqa: E501\n    return x  # the count\n";
        assert_eq!(
            texts("py", src),
            vec!["Open the hatch.\n\nWait for the light.\n", "the count"]
        );
    }

    #[test]
    fn block_comments_lose_decoration_and_markup() {
        let src = "/**\n * Start the pump {@link Pump}.\n * @param speed the `rpm` value\n */\nfunction f() {}\n";
        let got = texts("js", src);
        assert_eq!(got.len(), 1);
        let words: Vec<&str> = got[0].split_whitespace().collect();
        assert_eq!(
            words,
            ["Start", "the", "pump", ".", "speed", "the", "value"]
        );
    }

    #[test]
    fn non_prose_is_dropped() {
        let src = "// Copyright 2030 Nobody. All rights reserved.\nint a;\n// foo(bar);\n// baz();\nint b;\n#include <x.h>\n// eslint-disable-next-line\nint c;\n";
        assert!(texts("c", src).is_empty());
    }

    #[test]
    fn markdown_paragraphs_headings_and_cells() {
        let src = "# Frob guide\n\nLift the [gantry](https://x.example) *now*.\n\n    frob --all\n\n```\ncode here\n```\n\n| Part | Note |\n|---|---|\n| gantry | Lift it. |\n";
        let got = texts("md", src);
        let words: Vec<String> = got
            .iter()
            .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect();
        assert_eq!(
            words,
            [
                "Frob guide",
                "Lift the [gantry] *now*.",
                "Part",
                "Note",
                "gantry",
                "Lift it."
            ]
        );
    }

    #[test]
    fn positions_map_back_to_source() {
        let src = "x = 1\n\n    # Lift the arm.\n    # Then stop.\n";
        let lang = crate::grammars::Grammars::new(None)
            .for_path(Path::new("a.py"))
            .unwrap();
        let comments = extract(&lang, src);
        let c = &comments[0];
        assert_eq!(c.position(0), (3, 7));
        let then = c.text().find("Then").unwrap();
        assert_eq!(c.position(then), (4, 7));
    }

    fn grammar(ext: &str) -> Grammar {
        crate::grammars::Grammars::new(None)
            .for_path(Path::new(&format!("x.{ext}")))
            .unwrap()
    }

    #[test]
    fn source_ranges_map_back_to_source() {
        // Merged line comments.
        let src = "x = 1\n\n    # Lift the arm.\n    # Then stop.\n";
        let c = &extract(&grammar("py"), src)[0];
        let text = c.text();
        let then = text.find("Then").unwrap();
        let (a, b) = c.source_range(then, then + 4).unwrap();
        assert_eq!(&src[a..b], "Then");

        // A block comment. Its subsequent lines start with ` * `.
        let src = "/**\n * Start the pump.\n * Then wait.\n */\nfunction f() {}\n";
        let c = &extract(&grammar("js"), src)[0];
        let text = c.text();
        let wait = text.find("wait").unwrap();
        let (a, b) = c.source_range(wait, wait + 4).unwrap();
        assert_eq!(&src[a..b], "wait");

        // A range across a line break has no source range.
        let pump = text.find("pump.").unwrap();
        assert_eq!(c.source_range(pump, wait), None);
    }

    #[test]
    fn is_prose_picks_comments_and_paragraphs() {
        fn walk(grammar: &Grammar, node: Node, out: &mut Vec<(String, bool)>) {
            out.push((node.kind().to_string(), is_prose(grammar, node)));
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk(grammar, child, out);
            }
        }
        fn kinds(grammar: &Grammar, src: &str) -> Vec<(String, bool)> {
            let mut parser = Parser::new();
            parser.set_language(&grammar.language).unwrap();
            let tree = parser.parse(src, None).unwrap();
            let mut out = Vec::new();
            walk(grammar, tree.root_node(), &mut out);
            out
        }
        fn prose(kinds: &[(String, bool)], kind: &str) -> bool {
            kinds.iter().find(|(k, _)| k == kind).expect(kind).1
        }

        let got = kinds(&grammar("rs"), "// Turn the dial.\nfn f() {}\n");
        assert!(prose(&got, "line_comment"));
        assert!(!prose(&got, "function_item"));

        let got = kinds(&grammar("md"), "Turn the dial.\n");
        assert!(prose(&got, "paragraph"));
    }
}
