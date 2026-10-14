//! Where tree-sitter grammars come from.
//!
//! A grammar is a shared object with one exported function,
//! `tree_sitter_<name>`, that gives the language. Neovim, Helix and the
//! tree-sitter CLI all load grammars with this procedure, and nixpkgs puts
//! them in a directory as `<name>.so`. memelint looks for them in the
//! directories that `--grammars`, `MEMELINT_GRAMMARS` and
//! `$XDG_DATA_HOME/memelint/grammars` give, in that sequence. With the
//! `bundled` feature, some grammars are also compiled into the binary.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use tree_sitter::Language;

/// The grammar name for a file extension. The name is also the file stem of
/// the shared object and the suffix of its exported function. Thus the
/// grammar name of an unknown extension is the extension.
/// Files that are not source code. When they have no grammar, there is no
/// note.
const DATA_EXTENSIONS: &[&str] = &[
    "txt", "lock", "json", "yml", "yaml", "xml", "csv", "log", "cfg", "ini", "html", "svg", "png",
    "jpg", "jpeg", "gif", "pdf", "zip", "gz", "tar",
];

/// A grammar, and how to read the files it parses.
#[derive(Clone)]
pub struct Grammar {
    /// The grammar name, for example `rust`.
    pub name: String,
    pub language: Language,
    /// The document is prose (Markdown): lint its paragraphs, not its comments.
    pub prose: bool,
}

pub fn grammar_name(ext: &str) -> &str {
    match ext {
        "rs" => "rust",
        "py" | "pyi" => "python",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "h" => "c",
        "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => "cpp",
        "sh" | "zsh" => "bash",
        "rb" => "ruby",
        "kt" | "kts" => "kotlin",
        "ex" | "exs" => "elixir",
        "hs" => "haskell",
        "ml" | "mli" => "ocaml",
        "yml" => "yaml",
        "cs" => "c_sharp",
        "jl" => "julia",
        "pl" | "pm" => "perl",
        "el" => "elisp",
        "erl" => "erlang",
        "m" => "objc",
        "fs" | "fsx" => "fsharp",
        "md" => "markdown",
        other => other,
    }
}

pub struct Grammars {
    dirs: Vec<PathBuf>,
    loaded: HashMap<String, Option<Grammar>>,
    /// Grammar names that were wanted and not found.
    pub missing: BTreeSet<String>,
}

impl Grammars {
    pub fn new(explicit: Option<&Path>) -> Self {
        let mut dirs = Vec::new();
        if let Some(d) = explicit {
            dirs.push(d.to_path_buf());
        }
        if let Some(d) = std::env::var_os("MEMELINT_GRAMMARS") {
            dirs.extend(std::env::split_paths(&d));
        }
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
        if let Some(d) = data {
            dirs.push(d.join("memelint/grammars"));
        }
        Self::with_dirs(dirs)
    }

    /// Only these directories, and no directory from the environment.
    pub fn with_dirs(dirs: Vec<PathBuf>) -> Self {
        Grammars {
            dirs,
            loaded: HashMap::new(),
            missing: BTreeSet::new(),
        }
    }

    pub fn dirs(&self) -> &[PathBuf] {
        &self.dirs
    }

    /// The grammar for a file, if one is bundled or can be loaded.
    pub fn for_path(&mut self, path: &Path) -> Option<Grammar> {
        let ext = path.extension()?.to_str()?;
        if DATA_EXTENSIONS.contains(&ext) {
            return None;
        }
        let name = grammar_name(ext).to_string();
        if let Some(cached) = self.loaded.get(&name) {
            return cached.clone();
        }
        let grammar = bundled(&name)
            .or_else(|| self.load(&name))
            .map(|language| Grammar {
                name: name.clone(),
                language,
                prose: name == "markdown",
            });
        if grammar.is_none() {
            self.missing.insert(name.clone());
        }
        self.loaded.insert(name, grammar.clone());
        grammar
    }

    fn load(&mut self, name: &str) -> Option<Language> {
        let stems = [
            format!("{name}.so"),
            format!("libtree-sitter-{name}.so"),
            format!("{name}.dylib"),
        ];
        let symbol = format!("tree_sitter_{name}\0");
        for dir in &self.dirs {
            for stem in &stems {
                let file = dir.join(stem);
                if !file.is_file() {
                    continue;
                }
                // SAFETY: when a grammar is loaded, its constructors run. For
                // tree-sitter grammars they only set up static tables. The
                // symbol has the signature that each grammar exports.
                let library = unsafe { libloading::Library::new(&file) }.ok()?;
                let language = unsafe {
                    let f: libloading::Symbol<unsafe extern "C" fn() -> *const ()> =
                        library.get(symbol.as_bytes()).ok()?;
                    Language::new(tree_sitter_language::LanguageFn::from_raw(*f))
                };
                if language.abi_version() < tree_sitter::MIN_COMPATIBLE_LANGUAGE_VERSION
                    || language.abi_version() > tree_sitter::LANGUAGE_VERSION
                {
                    eprintln!(
                        "warning: {} has grammar ABI {} but this memelint accepts {} to {}; skipped",
                        file.display(),
                        language.abi_version(),
                        tree_sitter::MIN_COMPATIBLE_LANGUAGE_VERSION,
                        tree_sitter::LANGUAGE_VERSION
                    );
                    return None;
                }
                // A `Language` is a pointer into the library. Thus the library
                // must stay in memory until the process stops. Grammars are
                // small and a lint run is short. Do not unload them.
                std::mem::forget(library);
                return Some(language);
            }
        }
        None
    }
}

#[cfg(feature = "bundled")]
fn bundled(name: &str) -> Option<Language> {
    let lang = match name {
        "rust" => tree_sitter_rust::LANGUAGE,
        "python" => tree_sitter_python::LANGUAGE,
        "javascript" => tree_sitter_javascript::LANGUAGE,
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX,
        "go" => tree_sitter_go::LANGUAGE,
        "c" => tree_sitter_c::LANGUAGE,
        "java" => tree_sitter_java::LANGUAGE,
        "nix" => tree_sitter_nix::LANGUAGE,
        "toml" => tree_sitter_toml_ng::LANGUAGE,
        "markdown" => tree_sitter_md::LANGUAGE,
        _ => return None,
    };
    Some(lang.into())
}

#[cfg(not(feature = "bundled"))]
fn bundled(_name: &str) -> Option<Language> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_map_to_grammar_names() {
        assert_eq!(grammar_name("rs"), "rust");
        assert_eq!(grammar_name("hpp"), "cpp");
        assert_eq!(grammar_name("zig"), "zig");
    }

    #[test]
    fn missing_grammars_are_reported_not_fatal() {
        let empty = std::env::temp_dir().join("memelint-no-grammars-here");
        let mut g = Grammars::with_dirs(vec![empty]);
        assert!(g.for_path(Path::new("x.zig")).is_none());
        assert!(g.missing.contains("zig"));
        // The next lookup does not examine the directories again.
        assert!(g.for_path(Path::new("y.zig")).is_none());
    }

    #[cfg(feature = "bundled")]
    #[test]
    fn bundled_grammars_load() {
        let mut g = Grammars::new(None);
        assert!(g.for_path(Path::new("a.py")).is_some());
        assert!(g.for_path(Path::new("b.tsx")).is_some());
        assert!(g.for_path(Path::new("c.nix")).is_some());
        assert!(g.for_path(Path::new("d.toml")).is_some());
        assert!(g.for_path(Path::new("e.md")).is_some_and(|g| g.prose));
        assert!(g.for_path(Path::new("a.py")).is_some_and(|g| !g.prose));
    }

    /// Runs only where a grammar directory is available, for example in `nix develop`.
    #[test]
    fn loads_a_shared_object_grammar() {
        if std::env::var_os("MEMELINT_GRAMMARS").is_none() {
            return;
        }
        let mut g = Grammars::new(None);
        let grammar = g
            .for_path(Path::new("a.cpp"))
            .expect("cpp grammar in MEMELINT_GRAMMARS");
        // The language must operate after the loader that made it is removed.
        drop(g);
        let comments = crate::extract::extract(&grammar, "// Lift the arm.\nint x;\n");
        assert_eq!(comments[0].text(), "Lift the arm.");
    }
}
