# memelint

[![documentation](https://img.shields.io/badge/docs-blue)](https://memelint.antimemetics.institute)

Simplified Technical English lint for code comments and prose. memelint reads the comments and docstrings in your source files and tells you which words and sentences do not follow ASD-STE100. Markdown files are read as prose.

```console
$ memelint check
foo.py:2:29: error: 'ensure' is not approved; use 'make sure' [STE 1.1]
foo.py:3:7: error: 'don't' is a contraction; write the words in full [STE 4.2]
foo.py:3:13: error: 'delve' is not in the STE dictionary; if it is a technical name, add it to memelint.toml [STE 1.6]
foo.py:4:7: error: sentence has 28 words; the STE limit for descriptive text is 25 [STE 6.3]
foo.py:4:7: error: 'closing' is the -ing form of 'close'; STE allows only the infinitive, imperative, past tense and past participle [STE 3.5]
```

## Usage

[Nix](https://nixos.org/download/) is necessary. This builds the checker with the ASD-STE100 vocabulary and with each tree-sitter grammar that nixpkgs has, and runs it:

```bash
$(nix-build --no-out-link https://github.com/antimemes/memelint/archive/main.tar.gz)/bin/memelint check
```

With flakes:

```bash
nix run github:antimemes/memelint -- check
```

## Rules

ASD-STE100 Issue 9 has 53 rules. A program can check a part of them. memelint checks these, and each finding gives the rule id:

| Rule | Finding |
|---|---|
| 1.1 | a word that is not approved, with the approved alternatives |
| 1.2 | an approved word in a part of speech in which it is not approved |
| 1.6 | a word that is not in the dictionary, which can be a technical name for `memelint.toml` |
| 3.5 | the `-ing` form of an approved verb |
| 4.2 | a contraction |
| 6.3 | a sentence of more than 25 words |

Each rule has a severity: `error`, `warning` or `off`. The exit code is 1 when there is an error. The profile sets the severity of all rules, and `[rules]` changes one rule:

```toml
profile = "basic"       # "strict" (the default) makes each finding an error.
                        # "basic" makes 1.6 and 6.3 warnings.
[rules]
"4.2" = "off"

[languages.markdown.rules]
"1.6" = "off"           # no unknown-word findings in Markdown files
```

The other rules are in `crates/memelint/src/rules.rs`, with a note on how much of each one a program can check.

## Languages

memelint lints all languages that have a [tree-sitter grammar](https://tree-sitter.github.io/tree-sitter/#parsers). Rust, Python, JavaScript, TypeScript, Go, C, Java, Nix, TOML and Markdown are compiled in. For each other language, memelint loads `<name>.so` from a grammar directory: `--grammars`, `MEMELINT_GRAMMARS` or `~/.local/share/memelint/grammars`. These are the tree-sitter grammar files that Neovim and nixpkgs use.

## Your words

Technical names from your domain are not in the dictionary. memelint has lists of the technical names of software and of each language, and uses them unless you set `default-word-lists = false`. Put the names of your project in `memelint.toml` at the project root, and examine that file as you examine code:

```toml
words = ["gantry", "sprocket"]
word-lists = ["words.txt", "https://example.com/team-words.txt"]

[languages.rust]              # for the files of one language
words = ["monomorphize"]
```

A word list has one word on each line. `memelint fetch` downloads the URLs into the cache. [The word lists](crates/memelint/words/) are not complete. If you add words, send a pull request.

Paths in `ignore = ["vendor/"]` are not examined, with the syntax of `.gitignore`. The [documentation](https://memelint.antimemetics.institute/) has each key.

## The dictionary

ASD-STE100 is free to download, but it has copyright. Thus the memelint binary does not contain the dictionary. The Nix package makes the vocabulary from the PDF on your machine. With a different installation, make it one time on each machine:

```bash
memelint-import asd-ste100 --pdf ASD-STE100_ISSUE9.pdf
```

This writes the vocabulary to `~/.cache/memelint/`, which is not in your projects. `--vocab` or `MEMELINT_VOCAB` point the checker to a different file.

## Development

`nix develop` gives you the toolchain, the PDF and all grammars. The workspace has 3 crates: `memelint` (the checker), `memelint-import` (the importer) and `memelint-common` (the vocabulary file and its location). The end-to-end tests in `crates/memelint/tests/cases/` compare the output of `memelint check` on fixture worktrees with `expected.txt`. `UPDATE_EXPECT=1 cargo test --test cases` writes the correct output again after a change. The documentation is an mdBook in `docs/`: `mdbook serve docs`.

## License

MIT or Apache-2.0, as you select. This applies to the code, the tests and this document, and not to the vocabulary file, which comes from a document of ASD.
