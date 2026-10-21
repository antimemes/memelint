# Languages

memelint lints each language that has a [tree-sitter grammar](https://tree-sitter.github.io/tree-sitter/#parsers). The grammar name is the key of `[languages.<name>]`, for example `rust`, `python`, `cpp` or `markdown`.

Rust, Python, JavaScript, TypeScript, Go, C, Java, Nix, TOML and Markdown are compiled in. For each other language memelint loads `<name>.so` from a grammar directory, in this sequence:

1. `--grammars <dir>`
2. each directory in `MEMELINT_GRAMMARS`
3. `~/.local/share/memelint/grammars`

These are the grammar files that Neovim and nixpkgs make. The Nix package sets `MEMELINT_GRAMMARS` to all the grammars in nixpkgs. When a file has no grammar, memelint does not examine it, and gives a note at the end.

A `[languages.<name>]` table has the keys `check`, `words`, `word-lists`, `default-word-lists` and `rules`, for the files of that language.

Markdown files are prose. memelint reads their paragraphs, headings and table cells, and not their code blocks. When memelint walks a directory, it does not examine Markdown files, unless `[languages.markdown]`, a scope that names them or the top level has `check = true`. It examines a file that you name on the command line, for example `memelint check README.md`.
