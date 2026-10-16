//! Apply fixes to a source file, and the test that `--fix` makes before it
//! writes a file: the code must not change, only the comments.

use tree_sitter::{Node, Parser};

use crate::check::Fix;
use crate::extract::is_prose;
use crate::grammars::Grammar;

/// Apply the fixes to the source. None when a fix overlaps a different fix.
pub fn apply(source: &str, fixes: &[&Fix]) -> Option<String> {
    let mut fixes = fixes.to_vec();
    fixes.sort_by_key(|f| f.start);
    let mut out = String::with_capacity(source.len());
    let mut pos = 0;
    for f in fixes {
        // `is_char_boundary` rejects an offset after the end.
        if f.start < pos
            || f.end < f.start
            || !source.is_char_boundary(f.start)
            || !source.is_char_boundary(f.end)
        {
            return None;
        }
        out.push_str(&source[pos..f.start]);
        out.push_str(&f.replacement);
        pos = f.end;
    }
    out.push_str(&source[pos..]);
    Some(out)
}

/// Do `before` and `after` parse to the same syntax, with the prose not
/// compared? Then the change is only in the comments. A code formatter makes
/// the same test before it writes a file.
pub fn same_code(grammar: &Grammar, before: &str, after: &str) -> bool {
    match (skeleton(grammar, before), skeleton(grammar, after)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// One part of the syntax: its type, and its text when it is a token.
type Bone = (String, Option<String>);

/// The syntax without its prose: the type of each part, and the text of
/// each token. A part that is prose is one entry, and its text is not in
/// the list.
fn skeleton(grammar: &Grammar, source: &str) -> Option<Vec<Bone>> {
    let mut parser = Parser::new();
    parser
        .set_language(&grammar.language)
        .expect("grammar built against this tree-sitter version");
    let tree = parser.parse(source, None)?;
    let mut out = Vec::new();
    walk(grammar, tree.root_node(), source, &mut out);
    Some(out)
}

fn walk(grammar: &Grammar, node: Node, source: &str, out: &mut Vec<Bone>) {
    let kind = node.kind().to_string();
    if is_prose(grammar, node) {
        out.push((kind, None));
        return;
    }
    if node.child_count() == 0 {
        out.push((kind, Some(source[node.byte_range()].to_string())));
        return;
    }
    out.push((kind, None));
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(grammar, child, source, out);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn grammar(ext: &str) -> Grammar {
        crate::grammars::Grammars::new(None)
            .for_path(Path::new(&format!("x.{ext}")))
            .unwrap()
    }

    fn fix(start: usize, end: usize, replacement: &str) -> Fix {
        Fix {
            start,
            end,
            replacement: replacement.to_string(),
            safe: true,
        }
    }

    #[test]
    fn fixes_apply_in_any_sequence() {
        let src = "# frob the gantry\n";
        let first = fix(2, 6, "lift");
        let second = fix(11, 17, "hoist");
        assert_eq!(
            apply(src, &[&second, &first]).unwrap(),
            "# lift the hoist\n"
        );
        assert_eq!(apply(src, &[]).unwrap(), src);
    }

    #[test]
    fn bad_fixes_are_refused() {
        let src = "# frob the gantry\n";
        // One fix overlaps the other.
        assert!(apply(src, &[&fix(2, 10, "lift"), &fix(6, 17, "hoist")]).is_none());
        // After the end of the source.
        assert!(apply(src, &[&fix(2, 100, "lift")]).is_none());
        // In the middle of a character.
        assert!(apply("# é\n", &[&fix(3, 4, "e")]).is_none());
    }

    #[test]
    fn python_comment_and_docstring_edits_keep_the_code() {
        let g = grammar("py");
        let before =
            "def f():\n    \"\"\"Frob the gantry.\"\"\"\n    x = 1  # frob it\n    return x\n";
        let after =
            "def f():\n    \"\"\"Lift the gantry now.\"\"\"\n    x = 1  # lift it\n    return x\n";
        assert!(same_code(&g, before, after));
        assert!(!same_code(&g, before, &before.replace("x = 1", "x = 2")));
    }

    #[test]
    fn comment_that_becomes_code_is_a_change() {
        let g = grammar("py");
        assert!(!same_code(&g, "x = 1\n# y = 2\n", "x = 1\ny = 2\n"));
    }

    #[test]
    fn rust_doc_comment_edit_keeps_the_code() {
        let g = grammar("rs");
        assert!(same_code(
            &g,
            "/// Frob the gantry.\nfn f() {}\n",
            "/// Lift the gantry.\nfn f() {}\n"
        ));
    }

    #[test]
    fn markdown_paragraphs_are_prose_and_code_blocks_are_not() {
        let g = grammar("md");
        assert!(same_code(
            &g,
            "# Frob\n\nLift the gantry.\n\n```\nfrob --all\n```\n",
            "# Frob\n\nHoist the gantry.\n\n```\nfrob --all\n```\n"
        ));
        assert!(!same_code(
            &g,
            "Lift the gantry.\n",
            "```\nLift the gantry.\n```\n"
        ));
    }
}
