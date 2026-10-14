//! The bundled word lists: the technical names and verbs of software and
//! of each language. The spec tells each field of work to keep a list of
//! its technical names (rules 1.5 and 1.8). These lists are that list for
//! software. `memelint.toml` selects the lists that apply, by name.

/// The names of the bundled lists.
#[cfg(test)]
pub const NAMES: &[&str] = &[
    "software",
    "rust",
    "python",
    "go",
    "javascript",
    "typescript",
    "tsx",
    "c",
    "cpp",
    "java",
    "nix",
    "toml",
    "markdown",
];

/// The bundled list with this name. `software` is for all files. The other
/// names are grammar names, for the files of that language.
pub fn list(name: &str) -> Option<&'static str> {
    Some(match name {
        "software" => include_str!("../words/software.txt"),
        "rust" => include_str!("../words/rust.txt"),
        "python" => include_str!("../words/python.txt"),
        "go" => include_str!("../words/go.txt"),
        "javascript" => include_str!("../words/javascript.txt"),
        "typescript" => include_str!("../words/typescript.txt"),
        "tsx" => include_str!("../words/tsx.txt"),
        "c" => include_str!("../words/c.txt"),
        "cpp" => include_str!("../words/cpp.txt"),
        "java" => include_str!("../words/java.txt"),
        "nix" => include_str!("../words/nix.txt"),
        "toml" => include_str!("../words/toml.txt"),
        "markdown" => include_str!("../words/markdown.txt"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::Allowlist;

    #[test]
    fn each_name_has_a_list_of_plain_words() {
        for name in NAMES {
            let text = list(name).unwrap_or_else(|| panic!("no list {name}"));
            for word in text.lines().filter(|l| !l.starts_with('#')) {
                assert!(
                    word.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                    "{word:?} in {name} is not a plain word"
                );
            }
        }
        assert!(Allowlist::parse(list("software").unwrap()).allows("bytes"));
    }

    #[test]
    fn unknown_name_has_no_list() {
        assert!(list("zig").is_none());
    }
}
